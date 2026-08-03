//! Remote parallel execution control.
//!
//! Two properties run through every test here:
//!
//! - a controller that has only `GET /api/v2/capabilities` and
//!   `GET /api/v2/state` can tell sequential from parallel, available from
//!   unavailable, and eligible from blocked — without running Git itself;
//! - `set_parallel_mode` and `set_all_execution_marks` reach the *same* shared
//!   application service a keypress reaches, so neither can grow its own
//!   lifecycle matrix, its own exclusion vocabulary, or a partial target set.
//!
//! Unit-scoped where the subject is classification or projection; the service
//! tests use the real reducer and the in-memory `DynamicQueue`, with no
//! repository, process, or network involved.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;

use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, ParallelRuntime,
};
use crate::orchestration::run_control::{
    testing::RecordingScheduler, ResolveReservations, RunControlService,
};
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::dto::{CommandSpec, ErrorCode};
use crate::web::remote_control_api::executor::{RemoteControlExecutor, SharedServiceExecutor};
use crate::web::state::WebState;

use super::{get, harness, post_json, send, status_and_json};

fn change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 2,
        last_modified: "1m ago".to_string(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

/// One workspace observation: which changes are committed and which are dirty.
fn changes_refreshed(
    changes: Vec<Change>,
    committed: &[&str],
    uncommitted: &[&str],
) -> ExecutionEvent {
    ExecutionEvent::ChangesRefreshed {
        changes,
        rejected_changes: Vec::new(),
        committed_change_ids: committed.iter().map(|s| s.to_string()).collect(),
        uncommitted_file_change_ids: uncommitted.iter().map(|s| s.to_string()).collect(),
        worktree_change_ids: HashSet::new(),
        worktree_paths: HashMap::new(),
        worktree_not_ahead_ids: HashSet::new(),
        merge_wait_ids: HashSet::new(),
    }
}

/// Everything the production runner binds, over in-memory doubles.
struct Wired {
    web_state: Arc<WebState>,
    reducer: Arc<tokio::sync::RwLock<OrchestratorState>>,
    marks: Arc<ExecutionMarkStore>,
    parallel: Arc<ParallelRuntime>,
    scheduler: Arc<RecordingScheduler>,
    executor: SharedServiceExecutor,
}

impl Wired {
    async fn new(change_ids: &[&str]) -> Self {
        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::with_mode(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
            crate::orchestration::state::ExecutionMode::Parallel,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        parallel.set_available(true);
        parallel.set_max_concurrent(4);
        parallel.set_vcs_backend("git");

        let web_state = Arc::new(WebState::new(&[]));
        web_state.set_shared_state(reducer.clone()).await;
        web_state.set_execution_marks(marks.clone()).await;
        web_state.set_parallel_runtime(parallel.clone()).await;
        web_state.set_repo_root(PathBuf::from("/repo")).await;

        let service = Arc::new(
            OperatorCommandService::new(
                reducer.clone(),
                Arc::new(crate::tui::queue::DynamicQueue::new()),
                Arc::new(NoopQueueHooks),
                marks.clone(),
            )
            .with_parallel(parallel.clone()),
        );
        let scheduler = Arc::new(RecordingScheduler::new());
        let run_control = Arc::new(RunControlService::new(
            reducer.clone(),
            service.clone(),
            scheduler.clone(),
            Arc::new(ResolveReservations::new()),
            parallel.clone(),
        ));
        let projection = web_state.remote_control().projection();
        let executor =
            SharedServiceExecutor::new(service, run_control, web_state.clone(), projection);

        Self {
            web_state,
            reducer,
            marks,
            parallel,
            scheduler,
            executor,
        }
    }

    /// Publish a change list and an operator mode into the projection.
    ///
    /// The eligibility set is derived from the same two Git observations the
    /// refresh loop supplies, so the shared start guard and the published
    /// per-change reason always describe one workspace state.
    async fn observe(
        &self,
        change_ids: &[&str],
        committed: &[&str],
        uncommitted: &[&str],
        app_mode: &str,
    ) {
        let changes: Vec<Change> = change_ids.iter().copied().map(change).collect();
        self.web_state
            .apply_execution_event(&changes_refreshed(changes.clone(), committed, uncommitted))
            .await;
        self.web_state.update_with_mode(&changes, app_mode).await;

        let mut ineligible = Vec::new();
        for id in change_ids {
            if !committed.contains(id) || uncommitted.contains(id) {
                ineligible.push((*id).to_string());
            }
        }
        self.parallel.set_parallel_ineligible(ineligible);
        self.web_state.sync_remote_control_projection().await;
    }

    fn snapshot(&self) -> crate::web::remote_control_api::dto::InstanceSnapshot {
        self.web_state.remote_control().projection().snapshot().0
    }

    async fn status(&self, change_id: &str) -> String {
        self.reducer
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }
}

// ============================================================================
// Discovery: the snapshot answers "what mode, and why is this row blocked"
// ============================================================================

#[tokio::test]
async fn the_snapshot_distinguishes_every_parallel_runtime_condition() {
    use crate::web::remote_control_api::dto::ParallelMode;

    let wired = Wired::new(&["c1"]).await;
    wired.observe(&["c1"], &["c1"], &[], "select").await;

    // Sequential but available: the operator may turn it on.
    let parallel = wired.snapshot().parallel;
    assert_eq!(parallel.mode, ParallelMode::Sequential);
    assert!(parallel.available);
    assert_eq!(parallel.max_concurrent, 4);
    assert_eq!(parallel.vcs_backend, "git");

    // Active parallel.
    wired.parallel.set_parallel_mode(true);
    wired.web_state.sync_remote_control_projection().await;
    assert_eq!(wired.snapshot().parallel.mode, ParallelMode::Parallel);

    // Unavailable parallel is a distinct third condition, not "off".
    wired.parallel.set_parallel_mode(false);
    wired.parallel.set_available(false);
    wired.web_state.sync_remote_control_projection().await;
    let parallel = wired.snapshot().parallel;
    assert_eq!(parallel.mode, ParallelMode::Sequential);
    assert!(
        !parallel.available,
        "'parallel is off' and 'parallel cannot be turned on' are different operator problems"
    );
}

#[tokio::test]
async fn per_change_eligibility_explains_itself_without_the_client_running_git() {
    use crate::web::remote_control_api::dto::ParallelBlockedReason;

    let wired = Wired::new(&["eligible", "uncommitted", "dirty", "active", "final"]).await;
    wired
        .observe(
            &["eligible", "uncommitted", "dirty", "active", "final"],
            // `uncommitted` is absent from the commit tree; `dirty` is present
            // but has uncommitted files under it.
            &["eligible", "dirty", "active", "final"],
            &["dirty"],
            "running",
        )
        .await;
    {
        let mut guard = wired.reducer.write().await;
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "active".to_string(),
            command: "apply".to_string(),
        });
        guard.apply_execution_event(&ExecutionEvent::ChangeRejected {
            change_id: "final".to_string(),
            reason: "acceptance refused the proposal".to_string(),
        });
    }
    wired.web_state.sync_remote_control_projection().await;

    let snapshot = wired.snapshot();
    let row = |id: &str| {
        snapshot
            .changes
            .iter()
            .find(|change| change.id == id)
            .unwrap_or_else(|| panic!("'{id}' must be projected"))
            .clone()
    };

    assert!(row("eligible").parallel.eligible);
    assert_eq!(row("eligible").parallel.blocked_reason, None);
    assert_eq!(
        row("uncommitted").parallel.blocked_reason,
        Some(ParallelBlockedReason::NotCommitted)
    );
    assert_eq!(
        row("dirty").parallel.blocked_reason,
        Some(ParallelBlockedReason::UncommittedChanges),
        "a committed change with dirty files is a different fix than an uncommitted one"
    );
    // Eligibility is a workspace fact, so it is reported for an in-flight and a
    // final row too rather than being suppressed by their lifecycle status.
    assert!(row("active").parallel.eligible);
    assert_eq!(row("active").display_status, "applying");
    assert!(row("final").parallel.eligible);
    assert_eq!(row("final").display_status, "rejected");
}

#[tokio::test]
async fn capabilities_and_state_never_disagree_about_parallel_execution() {
    let h = harness(None, &[]);
    let mut snapshot = crate::web::remote_control_api::dto::InstanceSnapshot::empty();
    snapshot.parallel = crate::web::remote_control_api::dto::ParallelRuntimeState {
        mode: crate::web::remote_control_api::dto::ParallelMode::Parallel,
        available: true,
        max_concurrent: 7,
        vcs_backend: "git".to_string(),
    };
    h.projection
        .apply_state("state_refreshed", None, json!({}), snapshot);

    let (status, capabilities) =
        status_and_json(send(&h.router, get("/api/v2/capabilities", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(capabilities["parallel"]["mode"], "parallel");
    assert_eq!(capabilities["parallel"]["available"], true);
    assert_eq!(capabilities["parallel"]["max_concurrent"], 7);
    assert_eq!(capabilities["parallel"]["vcs_backend"], "git");
    let reasons: Vec<String> =
        serde_json::from_value(capabilities["parallel"]["blocked_reasons"].clone()).unwrap();
    assert_eq!(reasons, vec!["not_committed", "uncommitted_changes"]);
    let toggle_modes: Vec<String> =
        serde_json::from_value(capabilities["parallel"]["toggle_modes"].clone()).unwrap();
    assert_eq!(toggle_modes, vec!["select", "stopped"]);

    let (_, state) = status_and_json(send(&h.router, get("/api/v2/state", None)).await).await;
    let published = &state["snapshot"]["parallel"];
    for field in ["mode", "available", "max_concurrent", "vcs_backend"] {
        assert_eq!(
            published[field], capabilities["parallel"][field],
            "'{field}' comes from one source, so the two resources cannot drift"
        );
    }

    let commands: Vec<String> = serde_json::from_value(capabilities["commands"].clone()).unwrap();
    assert!(commands.contains(&"set_parallel_mode".to_string()));
    assert!(commands.contains(&"set_all_execution_marks".to_string()));
}

// ============================================================================
// The route and schema surface
// ============================================================================

#[tokio::test]
async fn the_new_commands_are_admitted_delegated_and_replayed_like_every_other() {
    for (body, expected) in [
        (
            json!({"type": "set_parallel_mode", "enabled": true}),
            CommandSpec::SetParallelMode { enabled: true },
        ),
        (
            json!({"type": "set_all_execution_marks"}),
            CommandSpec::SetAllExecutionMarks,
        ),
    ] {
        let h = harness(None, &[]);
        let mut envelope = body.as_object().unwrap().clone();
        envelope.insert("expected_revision".to_string(), json!(0));
        envelope.insert("idempotency_key".to_string(), json!("k1"));
        let envelope = serde_json::Value::Object(envelope).to_string();

        let (status, record) =
            status_and_json(send(&h.router, post_json("/api/v2/commands", None, &envelope)).await)
                .await;
        assert_eq!(status, StatusCode::OK, "{envelope}");
        assert_eq!(record["state"], "succeeded");
        assert_eq!(h.executor.calls(), vec![expected.clone()]);

        // An exact replay resolves from the record without a second side effect.
        let (replay_status, replay) =
            status_and_json(send(&h.router, post_json("/api/v2/commands", None, &envelope)).await)
                .await;
        assert_eq!(replay_status, StatusCode::OK);
        assert_eq!(replay["command_id"], record["command_id"]);
        assert_eq!(
            h.executor.call_count(),
            1,
            "a replay must not execute the command twice"
        );
    }
}

#[tokio::test]
async fn a_smuggled_parameter_is_a_schema_failure_not_a_silently_ignored_field() {
    // `set_all_execution_marks` derives its target state server-side. A client
    // that tries to dictate it must be refused rather than quietly obeyed-ish.
    let error =
        serde_json::from_str::<CommandSpec>(r#"{"type":"set_all_execution_marks","marked":false}"#);
    assert!(
        error.is_err(),
        "the bulk mutation takes no client-supplied target state"
    );

    let error = serde_json::from_str::<CommandSpec>(r#"{"type":"set_parallel_mode"}"#);
    assert!(error.is_err(), "the toggle value is mandatory");
}

#[tokio::test]
async fn a_stale_revision_refuses_a_bulk_mutation_before_it_reaches_the_service() {
    let h = harness(None, &[]);
    h.projection.apply_state(
        "state_refreshed",
        None,
        json!({}),
        super::snapshot_with("c1", "not queued"),
    );

    let body = r#"{"type":"set_all_execution_marks","expected_revision":0,"idempotency_key":"k1"}"#;
    let (status, response) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, body)).await).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(response["error_code"], "stale_revision");
    assert_eq!(
        h.executor.call_count(),
        0,
        "a stale bulk mutation must produce no partial effect at all"
    );
}

// ============================================================================
// Shared-service behavior through the v2 executor
// ============================================================================

#[tokio::test]
async fn remote_parallel_toggle_clears_ineligible_intent_and_reports_it() {
    let wired = Wired::new(&["committed", "uncommitted"]).await;
    wired
        .observe(&["committed", "uncommitted"], &["committed"], &[], "select")
        .await;
    wired
        .marks
        .replace(["committed".to_string(), "uncommitted".to_string()]);
    wired.reducer.write().await.apply_command(
        crate::orchestration::state::ReducerCommand::AddToQueue("uncommitted".to_string()),
    );

    let summary = wired
        .executor
        .execute(&CommandSpec::SetParallelMode { enabled: true })
        .await
        .expect("Select mode accepts the toggle");

    assert!(summary.changed);
    let detail = summary.detail.unwrap_or_default();
    assert!(
        detail.contains("parallel") && detail.contains("uncommitted"),
        "the outcome must identify each excluded change: {detail}"
    );
    assert_eq!(wired.marks.marked_ids(), vec!["committed".to_string()]);
    assert_eq!(wired.status("uncommitted").await, "not queued");
    assert!(wired.parallel.parallel_mode());

    // The mutation is readable at the revision the command settled at.
    let snapshot = wired.snapshot();
    assert_eq!(
        snapshot.parallel.mode,
        crate::web::remote_control_api::dto::ParallelMode::Parallel
    );
}

#[tokio::test]
async fn remote_parallel_toggle_is_refused_in_a_running_or_stopping_lifecycle() {
    for app_mode in ["running", "stopping", "error"] {
        let wired = Wired::new(&["c1"]).await;
        wired.observe(&["c1"], &["c1"], &[], app_mode).await;

        let failure = wired
            .executor
            .execute(&CommandSpec::SetParallelMode { enabled: true })
            .await
            .expect_err("a live lifecycle owns scheduling decisions the toggle would invalidate");

        assert_eq!(
            failure.error_code,
            ErrorCode::LifecycleConflict,
            "{app_mode}"
        );
        assert!(
            !wired.parallel.parallel_mode(),
            "{app_mode}: a refused toggle must change nothing"
        );
    }
}

#[tokio::test]
async fn remote_parallel_toggle_without_git_is_target_ineligible_not_a_lifecycle_problem() {
    let wired = Wired::new(&["c1"]).await;
    wired.observe(&["c1"], &["c1"], &[], "select").await;
    wired.parallel.set_available(false);

    let failure = wired
        .executor
        .execute(&CommandSpec::SetParallelMode { enabled: true })
        .await
        .expect_err("parallel execution requires git");

    assert_eq!(failure.error_code, ErrorCode::TargetIneligible);
    assert!(!wired.parallel.parallel_mode());
}

#[tokio::test]
async fn remote_bulk_mark_is_atomic_over_one_revision_and_names_its_exclusions() {
    let wired = Wired::new(&["a", "b", "uncommitted", "active"]).await;
    wired
        .observe(
            &["a", "b", "uncommitted", "active"],
            &["a", "b", "active"],
            &[],
            "running",
        )
        .await;
    wired.parallel.set_parallel_mode(true);
    {
        let mut guard = wired.reducer.write().await;
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "active".to_string(),
            command: "apply".to_string(),
        });
    }

    let summary = wired
        .executor
        .execute(&CommandSpec::SetAllExecutionMarks)
        .await
        .expect("Running mode accepts a bulk mutation");

    assert!(summary.changed);
    let detail = summary.detail.unwrap_or_default();
    assert!(
        detail.contains("2 change(s) marked")
            && detail.contains("2 excluded")
            && detail.contains("change_active")
            && detail.contains("parallel_ineligible"),
        "the outcome must report changed IDs and stable exclusion reasons: {detail}"
    );

    assert_eq!(
        wired.marks.marked_ids(),
        vec!["a".to_string(), "b".to_string()]
    );
    assert_eq!(wired.status("a").await, "queued");
    assert_eq!(wired.status("b").await, "queued");
    assert_eq!(
        wired.status("uncommitted").await,
        "not queued",
        "an excluded row keeps coherent intent"
    );
    assert_eq!(wired.status("active").await, "applying");
    assert!(
        wired.scheduler.calls().is_empty(),
        "a bulk mark expresses intent; it never dispatches a run by itself"
    );

    // Clearing NEW attention is a consequence of the mark, not a second step.
    let snapshot = wired.snapshot();
    for id in ["a", "b"] {
        let row = snapshot
            .changes
            .iter()
            .find(|change| change.id == id)
            .expect("marked change is projected");
        assert!(row.execution_marked);
        assert_eq!(
            row.attention,
            crate::web::remote_control_api::dto::AttentionState::None
        );
    }
}

#[tokio::test]
async fn remote_bulk_mark_with_no_eligible_row_settles_as_a_no_op() {
    let wired = Wired::new(&["rejected"]).await;
    wired
        .observe(&["rejected"], &["rejected"], &[], "select")
        .await;
    wired
        .reducer
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ChangeRejected {
            change_id: "rejected".to_string(),
            reason: "acceptance refused the proposal".to_string(),
        });

    let summary = wired
        .executor
        .execute(&CommandSpec::SetAllExecutionMarks)
        .await
        .expect("a zero-eligible bulk mutation is valid, not an error");

    assert!(
        !summary.changed,
        "nothing was eligible, so nothing may be claimed as changed"
    );
    assert!(wired.marks.marked_ids().is_empty());
}

#[tokio::test]
async fn remote_bulk_mark_is_refused_in_error_mode_without_touching_any_row() {
    let wired = Wired::new(&["c1"]).await;
    wired.observe(&["c1"], &["c1"], &[], "error").await;

    let failure = wired
        .executor
        .execute(&CommandSpec::SetAllExecutionMarks)
        .await
        .expect_err("recovery in Error mode is owned by the retry commands");

    assert_eq!(failure.error_code, ErrorCode::LifecycleConflict);
    assert!(wired.marks.marked_ids().is_empty());
}

// ============================================================================
// Atomic parallel start
// ============================================================================

#[tokio::test]
async fn one_ineligible_marked_target_rejects_a_remote_parallel_start_entirely() {
    let wired = Wired::new(&["committed", "uncommitted"]).await;
    wired
        .observe(&["committed", "uncommitted"], &["committed"], &[], "select")
        .await;
    wired.parallel.set_parallel_mode(true);
    wired
        .marks
        .replace(["committed".to_string(), "uncommitted".to_string()]);

    let failure = wired
        .executor
        .execute(&CommandSpec::Start)
        .await
        .expect_err("parallel start is all-or-nothing");

    assert_eq!(failure.error_code, ErrorCode::TargetIneligible);
    assert!(
        failure.message.contains("uncommitted"),
        "the response must identify the ineligible target and the reason: {}",
        failure.message
    );
    assert!(
        wired.scheduler.calls().is_empty(),
        "neither change may start"
    );
    assert_eq!(wired.status("committed").await, "not queued");
    assert_eq!(wired.status("uncommitted").await, "not queued");
    assert_eq!(
        wired.marks.marked_ids(),
        vec!["committed".to_string(), "uncommitted".to_string()],
        "marks and queue intent stay coherent after the refusal"
    );
}

#[tokio::test]
async fn a_fully_eligible_marked_set_starts_in_parallel_mode() {
    let wired = Wired::new(&["a", "b"]).await;
    wired.observe(&["a", "b"], &["a", "b"], &[], "select").await;
    wired.parallel.set_parallel_mode(true);
    wired.marks.replace(["a".to_string(), "b".to_string()]);

    let summary = wired
        .executor
        .execute(&CommandSpec::Start)
        .await
        .expect("every marked target is parallel-eligible");

    assert!(summary.changed);
    assert_eq!(wired.scheduler.calls().len(), 1);
    assert_eq!(wired.status("a").await, "queued");
    assert_eq!(wired.status("b").await, "queued");
}
