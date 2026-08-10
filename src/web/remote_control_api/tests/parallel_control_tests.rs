//! Remote worktree execution control.
//!
//! Two properties run through every test here:
//!
//! - a controller that has only `GET /api/v2/capabilities` and
//!   `GET /api/v2/state` can tell eligible from blocked — without running Git
//!   itself, and without any execution-mode dimension to read;
//! - `set_all_execution_marks` reaches the *same* shared application service a
//!   keypress reaches, so it cannot grow its own lifecycle matrix, its own
//!   exclusion vocabulary, or a partial target set.
//!
//! Unit-scoped where the subject is classification or projection; the service
//! tests use the real reducer and the in-memory `DynamicQueue`, with no
//! repository, process, or network involved.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use serde_json::json;

use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, OperatorOutcome,
    ParallelEligibility, ParallelRuntime, QueuePort,
};
use crate::orchestration::run_control::{
    testing::{RecordingScheduler, SchedulerCall},
    ResolveReservations, RunControlService,
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
    service: Arc<OperatorCommandService>,
    /// The same run-lifecycle service the TUI command handlers drive.
    run_control: Arc<RunControlService>,
    /// The admission authority the shared transaction validates against.
    core_mode: Arc<crate::orchestration::operator_coordinator::CoreMode>,
}

impl Wired {
    async fn new(change_ids: &[&str]) -> Self {
        let queue: Arc<dyn QueuePort> = Arc::new(crate::tui::queue::DynamicQueue::new());
        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        parallel.set_max_concurrent(4);
        parallel.set_vcs_backend("git");

        let web_state = Arc::new(WebState::new(&[]));
        web_state.set_shared_state(reducer.clone()).await;
        web_state.set_execution_marks(marks.clone()).await;
        web_state.set_parallel_runtime(parallel.clone()).await;
        web_state.set_repo_root(PathBuf::from("/repo")).await;

        // One scheduler handle, three readers: dispatch, command admission, and
        // the published action eligibility. Two handles would be two answers to
        // "is the owning boundary still alive".
        let scheduler = Arc::new(RecordingScheduler::new());
        web_state.set_run_boundary(scheduler.clone()).await;

        let service = Arc::new(
            OperatorCommandService::new(
                reducer.clone(),
                queue,
                Arc::new(NoopQueueHooks),
                marks.clone(),
            )
            .with_parallel(parallel.clone())
            .with_run_boundary(scheduler.clone()),
        );
        let run_control = Arc::new(RunControlService::new(
            reducer.clone(),
            service.clone(),
            scheduler.clone(),
            Arc::new(ResolveReservations::new()),
            parallel.clone(),
        ));
        let core_mode = Arc::new(crate::orchestration::operator_coordinator::CoreMode::new());
        let (executor, _application) = crate::web::remote_control_api::executor::wired_for_test(
            reducer.clone(),
            run_control.clone(),
            web_state.clone(),
            core_mode.clone(),
        );

        Self {
            web_state,
            reducer,
            marks,
            parallel,
            scheduler,
            executor,
            service,
            run_control,
            core_mode,
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
        // Command admission validates against Core, not against the published
        // snapshot, so an arrangement that moved only the snapshot would be
        // describing a process that cannot exist.
        self.core_mode
            .set(crate::orchestration::operator_command::OperatorMode::from_app_mode(app_mode));

        // The same shared classification the refresh loop uses, so the store's
        // reason and the projected per-change reason cannot disagree.
        let committed_ids: HashSet<String> = committed.iter().map(|id| (*id).to_string()).collect();
        let uncommitted_ids: HashSet<String> =
            uncommitted.iter().map(|id| (*id).to_string()).collect();
        let ineligible: Vec<(String, ParallelEligibility)> = change_ids
            .iter()
            .map(|id| {
                (
                    (*id).to_string(),
                    ParallelEligibility::observe(id, &committed_ids, &uncommitted_ids),
                )
            })
            .collect();
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

/// The snapshot publishes worktree runtime facts and no execution-mode
/// dimension at all.
#[tokio::test]
async fn the_snapshot_publishes_worktree_facts_without_a_mode_dimension() {
    let wired = Wired::new(&["c1"]).await;
    wired.observe(&["c1"], &["c1"], &[], "select").await;

    let parallel = wired.snapshot().parallel;
    assert_eq!(parallel.max_concurrent, 4);
    assert_eq!(parallel.vcs_backend, "git");

    let serialized = serde_json::to_value(&parallel).expect("the runtime state serializes");
    let object = serialized.as_object().expect("an object on the wire");
    assert_eq!(
        object.keys().cloned().collect::<Vec<_>>(),
        vec!["max_concurrent".to_string(), "vcs_backend".to_string()],
        "no execution-mode or availability dimension may be published"
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
        max_concurrent: 7,
        vcs_backend: "git".to_string(),
    };
    h.projection
        .apply_state("state_refreshed", None, json!({}), snapshot);

    let (status, capabilities) =
        status_and_json(send(&h.router, get("/api/v2/capabilities", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        capabilities["parallel"].get("mode").is_none(),
        "capabilities must not expose an execution-mode dimension"
    );
    assert_eq!(capabilities["parallel"]["max_concurrent"], 7);
    assert_eq!(capabilities["parallel"]["vcs_backend"], "git");
    let reasons: Vec<String> =
        serde_json::from_value(capabilities["parallel"]["blocked_reasons"].clone()).unwrap();
    assert_eq!(reasons, vec!["not_committed", "uncommitted_changes"]);
    assert!(
        capabilities["parallel"].get("toggle_modes").is_none(),
        "there is no mode to toggle, so no toggle modes may be advertised"
    );

    let (_, state) = status_and_json(send(&h.router, get("/api/v2/state", None)).await).await;
    let published = &state["snapshot"]["parallel"];
    for field in ["max_concurrent", "vcs_backend"] {
        assert_eq!(
            published[field], capabilities["parallel"][field],
            "'{field}' comes from one source, so the two resources cannot drift"
        );
    }

    let commands: Vec<String> = serde_json::from_value(capabilities["commands"].clone()).unwrap();
    assert!(
        !commands.contains(&"set_parallel_mode".to_string()),
        "the retired command must not be advertised"
    );
    assert!(commands.contains(&"set_all_execution_marks".to_string()));
}

// ============================================================================
// The route and schema surface
// ============================================================================

#[tokio::test]
async fn the_new_commands_are_admitted_delegated_and_replayed_like_every_other() {
    for (body, expected) in [(
        json!({"type": "set_all_execution_marks"}),
        CommandSpec::SetAllExecutionMarks {},
    )] {
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

    // The retired execution-mode command is not part of the closed enum any
    // more, so it fails schema validation instead of reaching a service.
    for body in [
        r#"{"type":"set_parallel_mode","enabled":true}"#,
        r#"{"type":"set_parallel_mode"}"#,
    ] {
        assert!(
            serde_json::from_str::<CommandSpec>(body).is_err(),
            "the retired command must be a schema failure: {body}"
        );
    }
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

/// A remote bulk mark covers every visible non-terminal row and writes marks only.
///
/// `dirty` has uncommitted proposal files, `absent` is missing from HEAD, and
/// `active` is mid-apply. None of those is a reason to refuse *next-run* intent:
/// they are current-run facts that start admission evaluates against the state
/// that holds when a run is actually requested. What the command must not do is
/// carry queue intent, dispatch a scheduler, or disturb the live row.
#[tokio::test]
async fn remote_bulk_mark_covers_every_non_terminal_row_without_run_effects() {
    let wired = Wired::new(&["a", "b", "dirty", "absent", "active"]).await;
    wired
        .observe(
            &["a", "b", "dirty", "absent", "active"],
            &["a", "b", "dirty", "active"],
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
    }

    let summary = wired
        .executor
        .execute(&CommandSpec::SetAllExecutionMarks {})
        .await
        .expect("Running mode accepts a bulk mutation");

    assert!(summary.changed);
    let detail = summary.detail.unwrap_or_default();
    assert!(
        detail.contains("5 change(s) marked") && !detail.contains("excluded"),
        "every visible non-terminal row is a target, so nothing is excluded: {detail}"
    );

    let mut marked = wired.marks.marked_ids();
    marked.sort();
    assert_eq!(
        marked,
        vec![
            "a".to_string(),
            "absent".to_string(),
            "active".to_string(),
            "b".to_string(),
            "dirty".to_string()
        ]
    );
    // Not one row moved in the queue: a mark is intent, not membership.
    for id in ["a", "b", "dirty", "absent"] {
        assert_eq!(
            wired.status(id).await,
            "not queued",
            "{id}: a mark must not write queue intent"
        );
    }
    assert_eq!(
        wired.status("active").await,
        "applying",
        "and the live row keeps running, unmarked or marked"
    );
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
        .execute(&CommandSpec::SetAllExecutionMarks {})
        .await
        .expect("a zero-eligible bulk mutation is valid, not an error");

    assert!(
        !summary.changed,
        "nothing was eligible, so nothing may be claimed as changed"
    );
    assert!(wired.marks.marked_ids().is_empty());
}

/// The single-row remote mark obeys the same contract as the bulk one.
///
/// Two commands, one rule: every visible non-terminal row accepts the mutation
/// whatever its status or mode, a terminal row settles as a reasoned no-op, and
/// neither writes queue intent, dispatches a scheduler, or disturbs an in-flight
/// run. The single command is checked against a *live* row on purpose — that is
/// the case the old matrix refused, and the one where a queue alias would show
/// up.
///
/// `gone` is driven all the way through archive *and* merge on purpose: archive
/// alone leaves the row in the post-archive resolve lane, which is still a
/// legitimate next-run target. Only the merge makes it terminal.
#[tokio::test]
async fn run_mark_intent_single_remote_mark_is_lifecycle_independent_and_effect_free() {
    let wired = Wired::new(&["active", "idle", "gone"]).await;
    wired
        .observe(
            &["active", "idle", "gone"],
            &["active", "idle", "gone"],
            &[],
            "running",
        )
        .await;
    {
        let mut guard = wired.reducer.write().await;
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "active".to_string(),
            command: "apply".to_string(),
        });
        guard.apply_execution_event(&ExecutionEvent::ChangeArchived("gone".to_string()));
        guard.apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "gone".to_string(),
            revision: "deadbeef".to_string(),
        });
    }
    assert_eq!(wired.status("gone").await, "merged");

    for change_id in ["active", "idle"] {
        let summary = wired
            .executor
            .execute(&CommandSpec::SetExecutionMark {
                change_id: change_id.to_string(),
                marked: true,
            })
            .await
            .unwrap_or_else(|err| panic!("`{change_id}` must accept a mark: {err:?}"));
        assert!(summary.changed, "`{change_id}` was unmarked before");
        assert!(wired.marks.is_marked(change_id));
    }

    // The live run is untouched: still applying, still not queued by the mark.
    assert_eq!(wired.status("active").await, "applying");
    assert_eq!(wired.status("idle").await, "not queued");
    assert!(
        wired.scheduler.calls().is_empty(),
        "a mark never wakes a scheduler"
    );

    // Unmarking the live row changes intent and nothing else.
    let summary = wired
        .executor
        .execute(&CommandSpec::SetExecutionMark {
            change_id: "active".to_string(),
            marked: false,
        })
        .await
        .expect("unmarking a live row is allowed");
    assert!(summary.changed);
    assert!(!wired.marks.is_marked("active"));
    assert_eq!(
        wired.status("active").await,
        "applying",
        "unmarking must not cancel or dequeue admitted work"
    );
    assert!(wired.scheduler.calls().is_empty());

    // The terminal row is the one exclusion, and the store keeps its shape.
    let summary = wired
        .executor
        .execute(&CommandSpec::SetExecutionMark {
            change_id: "gone".to_string(),
            marked: true,
        })
        .await
        .expect("a terminal target is a reasoned no-op, not a transport error");
    assert!(
        !summary.changed,
        "a terminal row carries no next-run intent: {summary:?}"
    );
    assert!(summary
        .detail
        .unwrap_or_default()
        .contains("terminal and carries no next-run intent"));
    assert!(!wired.marks.is_marked("gone"));
    assert!(wired.marks.is_marked("idle"), "unrelated marks survive");
}

/// Error mode marks like any other mode.
///
/// Recovery is still owned by the retry commands — this writes no retry intent
/// and starts nothing. It records which change the operator wants the next run
/// to consider, which is exactly the decision an operator makes while looking at
/// a failed run.
#[tokio::test]
async fn remote_bulk_mark_is_accepted_in_error_mode_without_starting_anything() {
    let wired = Wired::new(&["c1"]).await;
    wired.observe(&["c1"], &["c1"], &[], "error").await;

    let summary = wired
        .executor
        .execute(&CommandSpec::SetAllExecutionMarks {})
        .await
        .expect("Error mode does not gate next-run intent");

    assert!(summary.changed);
    assert_eq!(wired.marks.marked_ids(), vec!["c1".to_string()]);
    assert_eq!(wired.status("c1").await, "not queued");
    assert!(
        wired.scheduler.calls().is_empty(),
        "recovery stays owned by the retry commands"
    );
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

// ============================================================================
// Concurrency: the toggle and a bulk mark are one mutation each
// ============================================================================

/// The second mutation's outcome if it finished while the first one held the
/// guard, `None` if it was still waiting — which is what serialization looks
/// like.
///
/// The result is *carried* rather than asserted on immediately so that an
/// interleaving is judged by the state it leaves behind: the settled-state
/// assertion runs first, and the ordering assertion only explains it.
async fn settled_early(
    handle: &mut tokio::task::JoinHandle<
        crate::orchestration::operator_command::OperatorResult<OperatorOutcome>,
    >,
    accepted: &str,
) -> Option<OperatorOutcome> {
    tokio::time::timeout(Duration::from_millis(500), handle)
        .await
        .ok()
        .map(|joined| {
            joined
                .unwrap()
                .unwrap_or_else(|error| panic!("{accepted}: {error}"))
        })
}

/// A bulk mark must serialize against the shared mutation guard.
///
/// A bulk mark classifies against one observation and then writes the derived
/// state to every eligible row. If it could run while another mutation — an
/// operator command, or the event reconciliation that revokes marks — held the
/// guard, it would classify against a half-applied target set and the two would
/// settle on different answers.
///
/// The guard is held directly here rather than through a queue effect: a mark
/// is pure next-run intent and no longer touches `DynamicQueue` at all, so
/// there is no queue mutation left inside it to park in.
#[tokio::test(start_paused = true)]
async fn a_bulk_mark_waits_for_the_shared_mutation_guard() {
    let wired = Wired::new(&["a_eligible", "z_blocked"]).await;
    wired
        .observe(
            &["a_eligible", "z_blocked"],
            &["a_eligible"],
            &[],
            "running",
        )
        .await;

    // Another mutation is in flight: the guard is held and not yet released.
    let parallel = wired.service.parallel();
    let in_flight = parallel.lock_mutations().await;

    let mut bulk = tokio::spawn({
        let service = wired.service.clone();
        async move { service.set_all_execution_marks().await }
    });
    let raced = settled_early(&mut bulk, "Running mode accepts a bulk mark").await;
    assert!(
        raced.is_none(),
        "the bulk mark must wait for the in-flight mutation"
    );
    assert!(
        wired.marks.marked_ids().is_empty(),
        "a waiting bulk mark must not have written anything yet"
    );

    drop(in_flight);

    let settled = bulk
        .await
        .unwrap()
        .expect("Running mode accepts a bulk mark");

    match settled {
        OperatorOutcome::BulkMarks {
            marked, excluded, ..
        } => {
            assert!(marked, "the observation had no marks, so the bulk marks");
            assert!(
                excluded.is_empty(),
                "worktree eligibility no longer excludes a mark target: {excluded:?}"
            );
        }
        other => panic!("the bulk mark must complete in full: {other:?}"),
    }
    assert_eq!(
        {
            let mut ids = wired.marks.marked_ids();
            ids.sort();
            ids
        },
        vec!["a_eligible".to_string(), "z_blocked".to_string()],
        "every visible non-terminal row carries the derived mark"
    );
    // The mark wrote no queue intent: an ineligible row is still not queued.
    assert_eq!(wired.status("z_blocked").await, "not queued");
    assert_eq!(wired.status("a_eligible").await, "not queued");
}

#[tokio::test]
async fn a_fully_eligible_marked_set_starts() {
    let wired = Wired::new(&["a", "b"]).await;
    wired.observe(&["a", "b"], &["a", "b"], &[], "select").await;
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

// ============================================================================
// Workspace preparation projects one shared token
// ============================================================================

/// One internal preparation event must produce the same `preparing` token in the
/// reducer and in the `/api/v2` snapshot, with the action set an active row
/// gets. A surface that inferred preparation from logs or filesystem timing
/// instead would be free to disagree with the reducer; this is what stops that.
#[tokio::test]
async fn preparing_projection_is_one_reducer_token_across_every_surface() {
    use crate::events::{dispatch_event, EventSink};
    use crate::web::remote_control_api::dto::ActionBlockedReason;
    use crate::web::remote_control_api::projection::change_actions_for_test;
    use crate::web::state::WebEventSink;

    let wired = Wired::new(&["prep", "waiting"]).await;
    wired
        .observe(&["prep", "waiting"], &["prep", "waiting"], &[], "running")
        .await;

    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(wired.web_state.clone()))];
    dispatch_event(
        wired.reducer.as_ref(),
        &sinks,
        ExecutionEvent::WorkspacePreparationStarted {
            change_id: "prep".to_string(),
        },
    )
    .await;
    wired.web_state.sync_remote_control_projection().await;

    // The reducer is the single source.
    assert_eq!(wired.status("prep").await, "preparing");
    // A change still waiting behind an occupied slot is untouched.
    assert_eq!(wired.status("waiting").await, "not queued");

    let snapshot = wired.snapshot();
    let row = |id: &str| {
        snapshot
            .changes
            .iter()
            .find(|change| change.id == id)
            .unwrap_or_else(|| panic!("'{id}' must be projected"))
            .clone()
    };
    assert_eq!(row("prep").display_status, "preparing");
    assert_eq!(row("waiting").display_status, "not queued");

    // Preparation is active work, not a blocker and not a final outcome.
    assert!(
        row("prep").blocker.is_none(),
        "an active row must not grow a blocker badge"
    );
    let actions = row("prep").actions;
    assert!(
        actions.stop_and_dequeue.allowed,
        "stop remains expressible; the refusal is the queue's to make"
    );
    assert!(
        !actions.set_queue_intent.allowed,
        "an admitted change mutating its worktree is not queue-mutable"
    );
    assert!(
        actions.set_execution_mark.allowed,
        "but its next-run intent is still the operator's to state: {:?}",
        actions.set_execution_mark
    );
    assert_eq!(
        actions.resolve_merge.blocked_reason,
        Some(ActionBlockedReason::ChangeActive)
    );
    // The point is not any particular reason token: it is that a remote client
    // is offered exactly what an in-flight operation is offered, so preparation
    // cannot become a hole in the active-row action contract.
    assert_eq!(
        actions,
        change_actions_for_test("running", "applying", None),
        "preparing must advertise the same action set as a running operation"
    );

    // The legacy monitoring snapshot counts it as in-progress work too, so the
    // two web payloads cannot disagree about whether anything is running.
    let legacy = wired.web_state.get_state().await;
    assert_eq!(
        legacy
            .changes
            .iter()
            .find(|change| change.id == "prep")
            .and_then(|change| change.queue_status.as_deref()),
        Some("preparing")
    );
    assert_eq!(legacy.in_progress_changes, 1);

    // Preparation yields to the repository-derived phase, from the same reducer.
    dispatch_event(
        wired.reducer.as_ref(),
        &sinks,
        ExecutionEvent::ApplyStarted {
            change_id: "prep".to_string(),
            command: "apply".to_string(),
        },
    )
    .await;
    wired.web_state.sync_remote_control_projection().await;
    assert_eq!(wired.status("prep").await, "applying");
    assert_eq!(
        wired
            .snapshot()
            .changes
            .iter()
            .find(|change| change.id == "prep")
            .map(|change| change.display_status.clone()),
        Some("applying".to_string())
    );
}

/// A dispatch that ends before any operation-started event must leave the row
/// visibly not-preparing rather than stranding an active-looking status.
#[tokio::test]
async fn preparing_projection_clears_on_a_pre_operation_exit() {
    use crate::events::{dispatch_event, EventSink};
    use crate::web::state::WebEventSink;

    let wired = Wired::new(&["prep"]).await;
    wired.observe(&["prep"], &["prep"], &[], "running").await;
    {
        let mut guard = wired.reducer.write().await;
        guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
            "prep".to_string(),
        ));
    }

    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(wired.web_state.clone()))];
    for event in [
        ExecutionEvent::WorkspacePreparationStarted {
            change_id: "prep".to_string(),
        },
        ExecutionEvent::WorkspacePreparationEnded {
            change_id: "prep".to_string(),
        },
    ] {
        dispatch_event(wired.reducer.as_ref(), &sinks, event).await;
    }
    wired.web_state.sync_remote_control_projection().await;

    assert_eq!(wired.status("prep").await, "queued");
    assert_eq!(
        wired
            .snapshot()
            .changes
            .iter()
            .find(|change| change.id == "prep")
            .map(|change| change.display_status.clone()),
        Some("queued".to_string())
    );
    assert_eq!(wired.web_state.get_state().await.in_progress_changes, 0);
}

// ============================================================================
// Active-run Apply iteration limit: cross-adapter parity (integration)
// ============================================================================
//
// One wiring, two adapters. The TUI adapter reaches the shared services through
// `RunControlService` exactly as `handle_start_processing_command` does, and the
// v2 adapter reaches them through `SharedServiceExecutor`. Both are asserted
// against the same recorded scheduler, so "the adapters agree" is a comparison
// of one instrumented runtime rather than two differently stubbed ones.

impl Wired {
    /// Record a terminal error plus its typed Apply-ceiling evidence.
    async fn to_iteration_limit(&self, change_id: &str, attempts: u32, max: u32) {
        {
            let mut guard = self.reducer.write().await;
            guard.apply_execution_event(&ExecutionEvent::ProcessingError {
                id: change_id.to_string(),
                error: "max iterations reached".to_string(),
            });
            guard.record_apply_iteration_limit(change_id, attempts, max);
        }
        self.web_state.sync_remote_control_projection().await;
    }

    fn blocked_reason(
        &self,
        change_id: &str,
    ) -> Option<crate::web::remote_control_api::dto::ActionBlockedReason> {
        self.snapshot()
            .changes
            .iter()
            .find(|change| change.id == change_id)
            .and_then(|change| change.actions.retry_change.blocked_reason)
    }
}

#[tokio::test]
async fn active_iteration_limit_rejects_the_same_target_through_both_adapters() {
    use crate::orchestration::operator_command::OperatorCommandError;
    use crate::orchestration::run_control::RunControlError;
    use crate::web::remote_control_api::dto::ActionBlockedReason;

    let wired = Wired::new(&["limited"]).await;
    wired
        .observe(&["limited"], &["limited"], &[], "running")
        .await;
    wired.scheduler.set_running(true);
    wired.to_iteration_limit("limited", 50, 50).await;

    assert_eq!(
        wired.blocked_reason("limited"),
        Some(ActionBlockedReason::ApplyIterationLimitActive),
        "the authoritative snapshot advertises the refusal before either adapter acts"
    );

    // TUI adapter: the same shared service a keypress reaches.
    let tui_error = wired
        .run_control
        .retry_change("limited")
        .await
        .expect_err("the TUI adapter is refused");
    assert!(
        matches!(
            tui_error,
            RunControlError::Operator(OperatorCommandError::ApplyIterationLimitActive { .. })
        ),
        "unexpected TUI error: {tui_error:?}"
    );

    // v2 adapter: the same services behind the remote command executor.
    let v2_failure = wired
        .executor
        .execute(&CommandSpec::RetryChange {
            change_id: "limited".to_string(),
        })
        .await
        .expect_err("the v2 adapter is refused identically");
    assert_eq!(v2_failure.error_code, ErrorCode::TargetIneligible);

    assert_eq!(wired.status("limited").await, "error");
    assert!(
        wired.marks.marked_ids().is_empty(),
        "neither adapter may mark a refused target"
    );
    assert!(
        !wired.scheduler.calls().iter().any(|call| matches!(
            call,
            SchedulerCall::Notified | SchedulerCall::Started { .. }
        )),
        "neither adapter may notify or start a scheduler for a refused target: {:?}",
        wired.scheduler.calls()
    );
    assert_eq!(
        wired.blocked_reason("limited"),
        Some(ActionBlockedReason::ApplyIterationLimitActive),
        "the reason is still readable in the snapshot at the result revision"
    );
}

/// The queue alias is refused through the remote surface too, so a client cannot
/// route around `retry_change` by submitting `set_queue_intent=true`.
#[tokio::test]
async fn active_iteration_limit_refuses_the_remote_queue_intent_alias() {
    let wired = Wired::new(&["limited"]).await;
    wired
        .observe(&["limited"], &["limited"], &[], "running")
        .await;
    wired.scheduler.set_running(true);
    wired.to_iteration_limit("limited", 50, 50).await;

    let failure = wired
        .executor
        .execute(&CommandSpec::SetQueueIntent {
            change_id: "limited".to_string(),
            queued: true,
        })
        .await
        .expect_err("the alias is refused by the same guard");
    assert_eq!(failure.error_code, ErrorCode::TargetIneligible);
    assert_eq!(wired.status("limited").await, "error");
}

/// Bulk retry stays partial across the remote surface, and the limited target's
/// reason stays readable at the result revision.
#[tokio::test]
async fn active_iteration_limit_bulk_retry_stays_partial_through_the_remote_surface() {
    use crate::web::remote_control_api::dto::ActionBlockedReason;

    let wired = Wired::new(&["limited", "ordinary"]).await;
    wired
        .observe(
            &["limited", "ordinary"],
            &["limited", "ordinary"],
            &[],
            "running",
        )
        .await;
    wired.scheduler.set_running(true);
    wired.to_iteration_limit("limited", 50, 50).await;
    {
        let mut guard = wired.reducer.write().await;
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "ordinary".to_string(),
            error: "boom".to_string(),
        });
    }
    wired.web_state.sync_remote_control_projection().await;

    let summary = wired
        .executor
        .execute(&CommandSpec::RetryErrors {
            change_ids: vec!["limited".to_string(), "ordinary".to_string()],
        })
        .await
        .expect("an unrelated retryable target keeps the request useful");

    assert!(summary.changed);
    let detail = summary.detail.clone().unwrap_or_default();
    assert!(
        detail.contains("ordinary") && !detail.contains("limited"),
        "the result must not claim the limited target was accepted: {detail}"
    );
    assert_eq!(
        wired.blocked_reason("limited"),
        Some(ActionBlockedReason::ApplyIterationLimitActive)
    );
    assert_eq!(wired.status("limited").await, "error");
}

/// Scheduler-task exit retires the gate for both adapters at once, and the later
/// admission starts a boundary instead of waking the one that exited.
#[tokio::test]
async fn active_iteration_limit_run_boundary_exit_admits_a_later_run_for_both_adapters() {
    let wired = Wired::new(&["limited"]).await;
    wired
        .observe(&["limited"], &["limited"], &[], "running")
        .await;
    wired.scheduler.set_running(true);
    wired.to_iteration_limit("limited", 50, 50).await;
    assert!(wired.run_control.retry_change("limited").await.is_err());

    wired.scheduler.set_running(false);
    wired.web_state.sync_remote_control_projection().await;

    assert_eq!(
        wired.blocked_reason("limited"),
        None,
        "task exit removes the action block even though the record remains"
    );
    assert!(
        wired
            .reducer
            .read()
            .await
            .apply_iteration_limit("limited")
            .is_some(),
        "the typed record survives its boundary"
    );

    let summary = wired
        .executor
        .execute(&CommandSpec::RetryChange {
            change_id: "limited".to_string(),
        })
        .await
        .expect("a closed boundary admits the ordinary retry route");
    assert!(summary.changed);
    assert_eq!(
        wired.scheduler.started_targets(),
        vec![vec!["limited".to_string()]],
        "a later boundary is started, never a wake-up of the exited scheduler"
    );
    assert!(
        !wired
            .scheduler
            .calls()
            .iter()
            .any(|call| matches!(call, SchedulerCall::Notified)),
        "the exited scheduler is never notified: {:?}",
        wired.scheduler.calls()
    );
}
