//! The authoritative operator snapshot.
//!
//! One property runs through every test here: a controller that has *only* the
//! current `GET /api/v2/state` response must be able to render a change and
//! decide which commands are offered, without replaying events, parsing logs, or
//! re-deriving anything from a display string.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::events::{ExecutionEvent, LogEntry, StalledBlocker};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::operator_command::ExecutionMarkStore;
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::dto::{
    ActionBlockedReason, AttentionState, BlockerKind, ChangeBlocker, ParallelBlockedReason,
    QueueIntent,
};
use crate::web::remote_control_api::projection::{
    change_actions_for_test, project_snapshot, EventsSince,
};
use crate::web::state::{ChangeStatus, OrchestratorStateSnapshot, WebState};

use super::{get, harness_with_projection, send, status_and_json};

/// Every canonical display status the reducer can produce.
const CANONICAL_STATUSES: [&str; 18] = [
    "not queued",
    "queued",
    "blocked",
    "stalled",
    "applying",
    "accepting",
    "rejecting",
    "archiving",
    "resolving",
    "merge wait",
    "resolve pending",
    "reject pending",
    "archived",
    "merged",
    "pushed",
    "rejected",
    "error",
    "stopped",
];

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

fn source_with(changes: Vec<ChangeStatus>) -> OrchestratorStateSnapshot {
    let mut source = OrchestratorStateSnapshot::from_changes(&[]);
    source.app_mode = "running".to_string();
    source.changes = changes;
    source
}

/// A refresh event carrying the workspace observations the snapshot publishes.
fn changes_refreshed(
    changes: Vec<Change>,
    committed: &[&str],
    uncommitted: &[&str],
    worktree_paths: HashMap<String, PathBuf>,
) -> ExecutionEvent {
    ExecutionEvent::ChangesRefreshed {
        changes,
        rejected_changes: Vec::new(),
        committed_change_ids: committed.iter().map(|s| s.to_string()).collect(),
        uncommitted_file_change_ids: uncommitted.iter().map(|s| s.to_string()).collect(),
        worktree_change_ids: HashSet::new(),
        worktree_paths,
        worktree_not_ahead_ids: HashSet::new(),
        merge_wait_ids: HashSet::new(),
    }
}

/// A `WebState` wired to a reducer, an execution-mark store, and a repo root —
/// the same three authorities the runner binds in production.
async fn wired_web_state(
    change_ids: &[&str],
) -> (
    WebState,
    Arc<tokio::sync::RwLock<OrchestratorState>>,
    Arc<ExecutionMarkStore>,
) {
    let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        change_ids.iter().map(|id| id.to_string()).collect(),
        0,
    )));
    let marks = Arc::new(ExecutionMarkStore::new());
    let web_state = WebState::new(&[]);
    web_state.set_shared_state(reducer.clone()).await;
    web_state.set_execution_marks(marks.clone()).await;
    web_state.set_repo_root(PathBuf::from("/repo")).await;
    (web_state, reducer, marks)
}

/// Drive one event into both the reducer and the monitoring state, the way the
/// orchestrator does.
async fn observe(
    web_state: &WebState,
    reducer: &Arc<tokio::sync::RwLock<OrchestratorState>>,
    event: ExecutionEvent,
) {
    reducer.write().await.apply_execution_event(&event);
    web_state.apply_execution_event(&event).await;
}

fn projected_change(snapshot: &Value, change_id: &str) -> Value {
    snapshot["changes"]
        .as_array()
        .expect("changes array")
        .iter()
        .find(|change| change["id"] == change_id)
        .unwrap_or_else(|| panic!("change '{change_id}' must be projected"))
        .clone()
}

// ============================================================================
// Task 1 — the fields exist, with explicit empty values
// ============================================================================

#[test]
fn a_change_publishes_every_operator_decision_field_with_an_explicit_empty_value() {
    let source = source_with(vec![ChangeStatus {
        id: "c1".to_string(),
        ..Default::default()
    }]);

    let projected = project_snapshot(&source);
    let json = serde_json::to_value(&projected.changes[0]).unwrap();
    let object = json.as_object().unwrap();

    for field in [
        "id",
        "display_status",
        "progress_status",
        "completed_tasks",
        "total_tasks",
        "progress_percent",
        "dependencies",
        "execution_marked",
        "queue_intent",
        "attention",
        "blocker",
        "error_detail",
        "actions",
        "parallel",
        "timing",
        "latest_activity",
        "worktree",
    ] {
        assert!(
            object.contains_key(field),
            "'{field}' must be present on every change, not conditional"
        );
    }

    // An absent value is an explicit null, never a missing key: a client that
    // replaces its local data from this snapshot has to be able to *clear* a
    // field, which an omitted key cannot express.
    for empty in ["blocker", "error_detail", "latest_activity", "worktree"] {
        assert!(
            object[empty].is_null(),
            "'{empty}' must be an explicit null when there is nothing to report"
        );
    }
    assert_eq!(object["queue_intent"], "not_queued");
    assert_eq!(object["attention"], "none");
    assert_eq!(object["execution_marked"], false);
    assert_eq!(object["parallel"]["eligible"], true);
    assert!(object["parallel"]["blocked_reason"].is_null());
    assert!(object["timing"]["started_at"].is_null());
    assert!(object["timing"]["completed_at"].is_null());
    assert!(object["timing"]["elapsed_ms"].is_null());
}

#[test]
fn execution_mark_and_queue_intent_are_independent_fields() {
    // A marked change that the reducer has not queued, and a queued change that
    // carries no mark: neither combination is expressible if the two axes are
    // collapsed into one status word.
    let source = source_with(vec![
        ChangeStatus {
            id: "marked-only".to_string(),
            execution_marked: true,
            queue_intent: QueueIntent::NotQueued,
            queue_status: None,
            ..Default::default()
        },
        ChangeStatus {
            id: "queued-only".to_string(),
            execution_marked: false,
            queue_intent: QueueIntent::Queued,
            queue_status: Some("queued".to_string()),
            ..Default::default()
        },
    ]);

    let projected = project_snapshot(&source);

    assert!(projected.changes[0].execution_marked);
    assert_eq!(projected.changes[0].queue_intent, QueueIntent::NotQueued);
    assert_eq!(projected.changes[0].display_status, "not queued");

    assert!(!projected.changes[1].execution_marked);
    assert_eq!(projected.changes[1].queue_intent, QueueIntent::Queued);
    assert_eq!(projected.changes[1].display_status, "queued");
}

#[test]
fn the_snapshot_carries_no_live_clock_even_with_timing_published() {
    // Timing is boundary instants only. A continuously advancing field would
    // make every projection differ and advance `state_revision` forever.
    let json = serde_json::to_value(crate::web::remote_control_api::dto::InstanceSnapshot::empty())
        .unwrap();
    assert!(json.get("last_updated").is_none());

    let source = source_with(vec![ChangeStatus {
        id: "c1".to_string(),
        ..Default::default()
    }]);
    let change = serde_json::to_value(&project_snapshot(&source).changes[0]).unwrap();
    let timing = change["timing"].as_object().unwrap();
    assert_eq!(timing.len(), 3, "started_at, completed_at, elapsed_ms only");
    assert!(!timing.contains_key("elapsed_seconds_now"));
}

// ============================================================================
// Task 2 — one coherent snapshot from reducer, intent store, and observations
// ============================================================================

#[test]
fn action_eligibility_is_stated_for_every_canonical_status() {
    // Running mode, no blocker payload: the table below is the contract a client
    // branches on instead of re-implementing the lifecycle matrix.
    let expected: HashMap<&str, [Option<ActionBlockedReason>; 5]> = HashMap::from([
        // (set_execution_mark, set_queue_intent, retry_change, stop_and_dequeue, resolve_merge)
        (
            "not queued",
            [
                None,
                None,
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::NotMergeWaiting),
            ],
        ),
        (
            "queued",
            [
                None,
                None,
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::NotMergeWaiting),
            ],
        ),
        (
            "blocked",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::NotMergeWaiting),
            ],
        ),
        (
            "stalled",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                None,
                None,
                Some(ActionBlockedReason::NotMergeWaiting),
            ],
        ),
        (
            "applying",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::ChangeActive),
            ],
        ),
        (
            "accepting",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::ChangeActive),
            ],
        ),
        (
            "rejecting",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::ChangeActive),
            ],
        ),
        (
            "archiving",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::ChangeActive),
            ],
        ),
        (
            "resolving",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::ChangeActive),
            ],
        ),
        (
            "merge wait",
            [
                None,
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                None,
            ],
        ),
        (
            "resolve pending",
            [
                None,
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                None,
            ],
        ),
        (
            "reject pending",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::NotMergeWaiting),
            ],
        ),
        (
            "archived",
            [
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                None,
            ],
        ),
        (
            "merged",
            [
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
            ],
        ),
        (
            "pushed",
            [
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
            ],
        ),
        (
            "rejected",
            [
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
                Some(ActionBlockedReason::FinalStatus),
            ],
        ),
        (
            "error",
            [
                None,
                None,
                None,
                None,
                Some(ActionBlockedReason::NotMergeWaiting),
            ],
        ),
        (
            "stopped",
            [
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::StatusImmutable),
                Some(ActionBlockedReason::NoRetryableEvidence),
                None,
                Some(ActionBlockedReason::NotMergeWaiting),
            ],
        ),
    ]);

    for status in CANONICAL_STATUSES {
        let actions = change_actions_for_test("running", status, None);
        let expected = expected
            .get(status)
            .unwrap_or_else(|| panic!("'{status}' must have a declared expectation"));
        let observed = [
            actions.set_execution_mark,
            actions.set_queue_intent,
            actions.retry_change,
            actions.stop_and_dequeue,
            actions.resolve_merge,
        ];
        for (observed, expected) in observed.iter().zip(expected.iter()) {
            assert_eq!(
                observed.allowed,
                expected.is_none(),
                "'{status}': expected allowed={} got {observed:?}",
                expected.is_none()
            );
            assert_eq!(
                observed.blocked_reason, *expected,
                "'{status}': blocked reason mismatch"
            );
        }
    }
}

#[test]
fn mode_changes_the_offered_actions_without_changing_the_status() {
    // Error mode routes recovery through retry; a pending graceful stop refuses
    // intent changes outright. Both are mode facts, not status facts.
    let error_mode = change_actions_for_test("error", "not queued", None);
    assert_eq!(
        error_mode.set_execution_mark.blocked_reason,
        Some(ActionBlockedReason::RetryRequired)
    );

    let stopping = change_actions_for_test("stopping", "queued", None);
    assert_eq!(
        stopping.set_execution_mark.blocked_reason,
        Some(ActionBlockedReason::StopPending)
    );

    // Select mode has no runtime queue, so a mark is pure intent.
    let select = change_actions_for_test("select", "not queued", None);
    assert!(select.set_execution_mark.allowed);
    assert_eq!(
        select.set_queue_intent.blocked_reason,
        Some(ActionBlockedReason::ModeHasNoQueue)
    );
}

#[test]
fn a_non_resumable_hold_is_never_advertised_as_retryable() {
    let resumable = ChangeBlocker {
        status: "stalled".to_string(),
        kind: BlockerKind::None,
        category: Some("acceptance_finding".to_string()),
        detail: None,
        unblock_condition: None,
        prerequisite_owner: None,
        origin: None,
        resumable: true,
    };
    let stuck = ChangeBlocker {
        resumable: false,
        ..resumable.clone()
    };

    assert!(
        change_actions_for_test("running", "stalled", Some(&resumable))
            .retry_change
            .allowed
    );
    let refused = change_actions_for_test("running", "stalled", Some(&stuck)).retry_change;
    assert!(!refused.allowed);
    assert_eq!(
        refused.blocked_reason,
        Some(ActionBlockedReason::HoldNotResumable),
        "a hold that keeps its blocker evidence must not look retryable"
    );

    // An externally blocked change is retryable; a dependency wait is not.
    let external = ChangeBlocker {
        status: "blocked".to_string(),
        kind: BlockerKind::External,
        ..resumable.clone()
    };
    let dependency = ChangeBlocker {
        status: "blocked".to_string(),
        kind: BlockerKind::Dependency,
        ..resumable
    };
    assert!(
        change_actions_for_test("running", "blocked", Some(&external))
            .retry_change
            .allowed
    );
    assert_eq!(
        change_actions_for_test("running", "blocked", Some(&dependency))
            .retry_change
            .blocked_reason,
        Some(ActionBlockedReason::NoRetryableEvidence)
    );
}

#[tokio::test]
async fn dependency_external_and_stalled_holds_stay_distinguishable() {
    let (web_state, reducer, _) = wired_web_state(&["dep-wait", "ext-wait", "held"]).await;
    let changes = vec![change("dep-wait"), change("ext-wait"), change("held")];
    web_state.update(&changes).await;

    observe(
        &web_state,
        &reducer,
        ExecutionEvent::DependencyBlocked {
            change_id: "dep-wait".to_string(),
            dependency_ids: vec!["other".to_string()],
        },
    )
    .await;
    observe(
        &web_state,
        &reducer,
        ExecutionEvent::AcceptanceGated {
            change_id: "ext-wait".to_string(),
            blocker: StalledBlocker::acceptance_external("credential", "STAGING_API_KEY is unset"),
        },
    )
    .await;
    observe(
        &web_state,
        &reducer,
        ExecutionEvent::AcceptanceGated {
            change_id: "held".to_string(),
            blocker: StalledBlocker {
                category: "acceptance_finding".to_string(),
                phase: "acceptance".to_string(),
                gate: "acceptance".to_string(),
                error_summary: "unresolved finding".to_string(),
                evidence: vec!["src/lib.rs:1 missing coverage".to_string()],
                unblock_condition: None,
                prerequisite_owner: None,
                next_action: "resolve the finding and retry".to_string(),
                resumable: true,
                worktree_preserved: true,
            },
        },
    )
    .await;

    let (snapshot, _, _) = web_state.remote_control().projection().snapshot();
    let by_id: HashMap<&str, _> = snapshot
        .changes
        .iter()
        .map(|change| (change.id.as_str(), change))
        .collect();

    let dependency = by_id["dep-wait"]
        .blocker
        .as_ref()
        .expect("dependency blocker");
    assert_eq!(by_id["dep-wait"].display_status, "blocked");
    assert_eq!(dependency.kind, BlockerKind::Dependency);

    let external = by_id["ext-wait"]
        .blocker
        .as_ref()
        .expect("external blocker");
    assert_eq!(by_id["ext-wait"].display_status, "blocked");
    assert_eq!(external.kind, BlockerKind::External);
    assert!(
        external.unblock_condition.is_some(),
        "an external wait states what clears it"
    );
    assert!(external.resumable);

    let stalled = by_id["held"].blocker.as_ref().expect("stalled hold");
    assert_eq!(by_id["held"].display_status, "stalled");
    assert_eq!(
        stalled.kind,
        BlockerKind::None,
        "an execution hold is not a wait on a named prerequisite"
    );
    assert!(stalled.detail.is_some());

    // `blocked` alone cannot separate the first two, and the kind alone cannot
    // separate the second from the third. Both axes are required, and both are
    // published.
    assert_eq!(
        by_id["dep-wait"].display_status,
        by_id["ext-wait"].display_status
    );
    assert_ne!(dependency.kind, external.kind);
}

#[tokio::test]
async fn a_change_local_error_is_distinct_from_a_fatal_process_error() {
    let (web_state, reducer, _) = wired_web_state(&["c1"]).await;
    web_state.update(&[change("c1")]).await;

    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ProcessingError {
            id: "c1".to_string(),
            error: "apply exited 1".to_string(),
        },
    )
    .await;
    web_state
        .apply_execution_event(&ExecutionEvent::Error {
            message: "the orchestrator could not start".to_string(),
        })
        .await;

    let (snapshot, _, _) = web_state.remote_control().projection().snapshot();
    assert_eq!(snapshot.changes[0].display_status, "error");
    assert_eq!(
        snapshot.changes[0].error_detail.as_deref(),
        Some("apply exited 1")
    );
    assert_eq!(
        snapshot.process_error.as_deref(),
        Some("the orchestrator could not start"),
        "a dead run and a failed change are different facts"
    );
}

/// Bounded post-archive resolve exhaustion is change-local across the remote
/// surface: one ordered `resolve_failed` carrying the change ID, presentation
/// telemetry that owns no state, and `process_error` left unset.
///
/// The regression it guards is the run where the same failure was also emitted
/// as a global Error, which made every frontend report a dead run while the
/// scheduler was still alive.
#[tokio::test]
async fn an_exhausted_post_archive_resolve_stays_change_scoped_on_the_remote_surface() {
    let (web_state, reducer, _) = wired_web_state(&["alpha"]).await;
    web_state.update(&[change("alpha")]).await;
    let projection = web_state.remote_control().projection();
    let before = projection.revision();

    let detail = crate::parallel::resolve_failure_detail(
        3,
        crate::parallel::ResolveFailureClassification::UnresolvedConflict,
        "conflicts still present after merge resolution attempt: a.rs",
    );

    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ConflictResolutionFailed {
            error: detail.clone(),
        },
    )
    .await;
    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ResolveFailed {
            change_id: "alpha".to_string(),
            error: detail,
        },
    )
    .await;

    let published = match projection.events_after(before) {
        EventsSince::Replay(events) => events,
        EventsSince::Gap => {
            panic!("the cursor was just taken; the stream must be replayable")
        }
    };
    let resolve_failed: Vec<_> = published
        .iter()
        .filter(|event| event.event_type == "resolve_failed")
        .collect();
    assert_eq!(
        resolve_failed.len(),
        1,
        "exactly one change-scoped projection for the failure"
    );
    assert_eq!(
        resolve_failed[0].change_id.as_deref(),
        Some("alpha"),
        "the ordered event must keep the structured change ID"
    );
    assert!(
        published
            .iter()
            .any(|event| event.event_type == "conflict_resolution_failed"),
        "presentation telemetry is still ordered on the stream"
    );
    assert!(
        published
            .iter()
            .all(|event| event.event_type != "process_error"),
        "a change-local failure must not project as a process error"
    );

    let (snapshot, _, _) = projection.snapshot();
    assert_eq!(
        snapshot.process_error, None,
        "the run is not dead; only one change is waiting for explicit retry"
    );
}

/// A run-fatal outcome keeps its existing process-fatal projection. Change-local
/// suppression must not downgrade the one event that means the run stopped.
#[tokio::test]
async fn a_run_fatal_outcome_remains_process_fatal_on_the_remote_surface() {
    let (web_state, reducer, _) = wired_web_state(&["alpha"]).await;
    web_state.update(&[change("alpha")]).await;
    let projection = web_state.remote_control().projection();
    let before = projection.revision();

    observe(
        &web_state,
        &reducer,
        ExecutionEvent::Error {
            message: "Background merge failed for 'alpha' (workspace 'ws-alpha'): base branch could not be identified".to_string(),
        },
    )
    .await;

    let published = match projection.events_after(before) {
        EventsSince::Replay(events) => events,
        EventsSince::Gap => {
            panic!("the cursor was just taken; the stream must be replayable")
        }
    };
    assert!(
        published
            .iter()
            .any(|event| event.event_type == "process_error"),
        "a run-fatal global Error still projects as process-scoped"
    );
    let (snapshot, _, _) = projection.snapshot();
    assert!(
        snapshot
            .process_error
            .as_deref()
            .is_some_and(|detail| detail.contains("base branch could not be identified")),
        "the fatal detail must reach the operator"
    );
}

#[tokio::test]
async fn parallel_eligibility_and_worktree_relation_come_from_the_workspace_observation() {
    let (web_state, _, _) = wired_web_state(&["ready", "dirty", "uncommitted"]).await;
    let changes = vec![change("ready"), change("dirty"), change("uncommitted")];
    web_state.update(&changes).await;

    let mut worktree_paths = HashMap::new();
    worktree_paths.insert(
        "ready".to_string(),
        PathBuf::from("/repo/../worktrees/ws-ready"),
    );
    web_state
        .apply_execution_event(&ExecutionEvent::WorktreesRefreshed {
            worktrees: vec![crate::tui::types::WorktreeInfo {
                path: PathBuf::from("/repo/../worktrees/ws-ready"),
                head: "abc1234".to_string(),
                branch: "ws-ready".to_string(),
                is_detached: false,
                is_main: false,
                merge_conflict: None,
                has_commits_ahead: true,
                is_merging: false,
            }],
        })
        .await;
    web_state
        .apply_execution_event(&changes_refreshed(
            changes.clone(),
            &["ready", "dirty"],
            &["dirty"],
            worktree_paths,
        ))
        .await;

    let (snapshot, _, _) = web_state.remote_control().projection().snapshot();
    let by_id: HashMap<&str, _> = snapshot
        .changes
        .iter()
        .map(|change| (change.id.as_str(), change))
        .collect();

    assert!(by_id["ready"].parallel.eligible);
    assert!(by_id["ready"].parallel.blocked_reason.is_none());
    assert_eq!(
        by_id["dirty"].parallel.blocked_reason,
        Some(ParallelBlockedReason::UncommittedChanges)
    );
    assert_eq!(
        by_id["uncommitted"].parallel.blocked_reason,
        Some(ParallelBlockedReason::NotCommitted)
    );

    let worktree = by_id["ready"].worktree.as_ref().expect("worktree relation");
    assert_eq!(worktree.branch.as_deref(), Some("ws-ready"));
    assert_eq!(
        worktree.path, "../worktrees/ws-ready",
        "the relation is published repository-relative"
    );
    assert!(
        by_id["dirty"].worktree.is_none(),
        "a change with no worktree says so explicitly"
    );
}

#[tokio::test]
async fn a_published_worktree_path_is_never_absolute() {
    let (web_state, _, _) = wired_web_state(&["c1"]).await;
    web_state.update(&[change("c1")]).await;

    let mut worktree_paths = HashMap::new();
    worktree_paths.insert(
        "c1".to_string(),
        PathBuf::from("/repo/../secret-root/worktrees/ws-c1"),
    );
    web_state
        .apply_execution_event(&changes_refreshed(
            vec![change("c1")],
            &["c1"],
            &[],
            worktree_paths,
        ))
        .await;

    let (snapshot, _, _) = web_state.remote_control().projection().snapshot();
    let path = &snapshot.changes[0].worktree.as_ref().unwrap().path;
    assert!(
        !path.starts_with('/'),
        "an absolute worktree root must never cross the boundary: {path}"
    );
    let body = serde_json::to_string(&snapshot).unwrap();
    assert!(!body.contains("\"/repo"), "no absolute root in the payload");
}

#[tokio::test]
async fn timing_and_latest_activity_track_the_run_without_churning_the_revision() {
    let (web_state, reducer, _) = wired_web_state(&["c1"]).await;
    web_state.update(&[change("c1")]).await;
    let projection = web_state.remote_control().projection();

    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ProcessingStarted("c1".to_string()),
    )
    .await;

    let (snapshot, revision_after_start, _) = projection.snapshot();
    assert!(snapshot.changes[0].timing.started_at.is_some());
    assert!(snapshot.changes[0].timing.completed_at.is_none());
    assert_eq!(
        snapshot.changes[0]
            .latest_activity
            .as_ref()
            .map(|activity| activity.event_type.as_str()),
        Some("processing_started")
    );

    // Streaming output and logs are observational. If they reached
    // `latest_activity`, every chunk would advance the revision and leave every
    // client's optimistic token permanently stale.
    web_state
        .apply_execution_event(&ExecutionEvent::ApplyOutput {
            change_id: "c1".to_string(),
            output: "compiling".to_string(),
            iteration: None,
        })
        .await;
    web_state
        .apply_execution_event(&ExecutionEvent::Log(LogEntry::info("noise")))
        .await;
    let (snapshot, revision_after_noise, _) = projection.snapshot();
    assert_eq!(
        revision_after_noise, revision_after_start,
        "output and logs must not advance the revision"
    );
    assert_eq!(
        snapshot.changes[0]
            .latest_activity
            .as_ref()
            .map(|activity| activity.event_type.as_str()),
        Some("processing_started")
    );

    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ChangeArchived("c1".to_string()),
    )
    .await;
    let (snapshot, revision_after_end, _) = projection.snapshot();
    assert!(revision_after_end > revision_after_start);
    let timing = &snapshot.changes[0].timing;
    assert!(timing.completed_at.is_some());
    assert!(
        timing.elapsed_ms.is_some(),
        "a finished run reports how long it took"
    );
    assert_eq!(
        snapshot.changes[0]
            .latest_activity
            .as_ref()
            .map(|activity| activity.event_type.as_str()),
        Some("change_archived")
    );
}

#[tokio::test]
async fn operator_detail_is_sanitized_before_it_is_published() {
    let (web_state, reducer, _) = wired_web_state(&["c1"]).await;
    web_state.update(&[change("c1")]).await;

    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ProcessingError {
            id: "c1".to_string(),
            error: "\u{1b}[31mfailed\u{1b}[0m\nsecond line\u{7}".to_string(),
        },
    )
    .await;

    let (snapshot, _, _) = web_state.remote_control().projection().snapshot();
    let detail = snapshot.changes[0].error_detail.as_deref().unwrap();
    assert!(!detail.contains('\u{1b}'), "ANSI escapes are stripped");
    assert!(!detail.contains('\n'), "raw newlines are escaped");
    assert!(!detail.contains('\u{7}'), "control characters are dropped");
    assert!(detail.contains("failed"));

    let activity = snapshot.changes[0].latest_activity.as_ref().unwrap();
    let activity_detail = activity.detail.as_deref().unwrap();
    assert!(!activity_detail.contains('\u{1b}'));
}

// ============================================================================
// Task 3 — ordered publication, replay gaps, and process incarnation
// ============================================================================

#[tokio::test]
async fn a_command_that_changes_a_decision_field_publishes_it_before_settling() {
    use crate::orchestration::operator_command::{NoopQueueHooks, OperatorCommandService};
    use crate::web::remote_control_api::dto::CommandSpec;
    use crate::web::remote_control_api::executor::{RemoteControlExecutor, SharedServiceExecutor};

    let (web_state, reducer, marks) = wired_web_state(&["c1"]).await;
    let web_state = Arc::new(web_state);
    web_state.update(&[change("c1")]).await;

    // The real shared service, over an in-memory queue: an execution mark emits
    // no execution event, so nothing else would ever publish it.
    let service = Arc::new(OperatorCommandService::new(
        reducer.clone(),
        Arc::new(crate::tui::queue::DynamicQueue::new()),
        Arc::new(NoopQueueHooks),
        marks.clone(),
    ));
    let run_control = Arc::new(crate::orchestration::run_control::RunControlService::new(
        reducer.clone(),
        service.clone(),
        Arc::new(crate::orchestration::run_control::testing::RecordingScheduler::new()),
        Arc::new(crate::orchestration::run_control::ResolveReservations::new()),
        Arc::new(crate::orchestration::run_control::StartEligibility::new()),
    ));
    let projection = web_state.remote_control().projection();
    let executor =
        SharedServiceExecutor::new(service, run_control, web_state.clone(), projection.clone());

    let revision_before = projection.revision();
    let summary = executor
        .execute(&CommandSpec::SetExecutionMark {
            change_id: "c1".to_string(),
            marked: true,
        })
        .await
        .expect("a select-mode mark is accepted");
    assert!(summary.changed);

    let (snapshot, revision_after, _) = projection.snapshot();
    assert_eq!(
        revision_after,
        revision_before + 1,
        "the mutation is published at once, not left for an unrelated event"
    );
    assert!(snapshot.changes[0].execution_marked, "the mark reads back");
    assert_eq!(
        snapshot.changes[0].queue_intent,
        QueueIntent::NotQueued,
        "and the unchanged queue intent is reported alongside it"
    );

    // A repeated identical command changes nothing and publishes nothing.
    let repeat = executor
        .execute(&CommandSpec::SetExecutionMark {
            change_id: "c1".to_string(),
            marked: true,
        })
        .await
        .expect("a repeated mark is still accepted");
    assert!(!repeat.changed);
    assert_eq!(projection.revision(), revision_after);
}

#[tokio::test]
async fn a_replay_gap_is_fully_recovered_from_the_state_resource() {
    let (web_state, reducer, marks) = wired_web_state(&["c1"]).await;
    web_state.update(&[change("c1")]).await;
    let projection = web_state.remote_control().projection();

    let mut worktree_paths = HashMap::new();
    worktree_paths.insert("c1".to_string(), PathBuf::from("/repo/../worktrees/ws-c1"));
    web_state
        .apply_execution_event(&changes_refreshed(
            vec![change("c1")],
            &["c1"],
            &[],
            worktree_paths,
        ))
        .await;
    // Processing start records queue intent; apply start is what makes the
    // change active. Both axes have to survive the gap independently.
    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ProcessingStarted("c1".to_string()),
    )
    .await;
    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ApplyStarted {
            change_id: "c1".to_string(),
            command: "apply".to_string(),
        },
    )
    .await;
    marks.set("c1", true);
    web_state.sync_remote_control_projection().await;

    // Overflow the retained ring so the cursor genuinely cannot be replayed.
    for i in 0..(crate::web::remote_control_api::dto::MAX_EVENTS + 5) {
        projection.apply_log(LogEntry::info(format!("line {i}")));
    }
    assert_eq!(
        projection.events_after(1),
        EventsSince::Gap,
        "the test must actually exercise a gap"
    );

    let harness = harness_with_projection(projection.clone(), None, &[]);
    let (status, body) =
        status_and_json(send(&harness.router, get("/api/v2/state", None)).await).await;
    assert_eq!(status, StatusCode::OK);

    // Everything an operator decision needs comes back from this one response.
    let change = projected_change(&body["snapshot"], "c1");
    assert_eq!(change["display_status"], "applying");
    assert_eq!(change["execution_marked"], true);
    assert_eq!(
        change["queue_intent"], "queued",
        "queue intent survives the gap as its own field, not folded into the status"
    );
    assert!(change["timing"]["started_at"].is_string());
    assert_eq!(change["latest_activity"]["event_type"], "apply_started");
    assert_eq!(change["worktree"]["path"], "../worktrees/ws-c1");
    assert_eq!(change["parallel"]["eligible"], true);
    assert!(change["actions"]["stop_and_dequeue"]["allowed"].is_boolean());
    assert_eq!(
        body["snapshot"]["changes"].as_array().unwrap().len(),
        1,
        "the snapshot is a replacement, not a delta"
    );
}

#[tokio::test]
async fn a_new_process_incarnation_clears_ephemeral_intent_and_recomputes_from_the_workspace() {
    let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["c1".to_string()],
        0,
    )));
    let first_marks = Arc::new(ExecutionMarkStore::new());
    let first = WebState::new(&[]);
    first.set_shared_state(reducer.clone()).await;
    first.set_execution_marks(first_marks.clone()).await;
    first.update(&[change("c1")]).await;
    // A second observation is what makes an arrival "new"; the first is baseline.
    first.update(&[change("c1"), change("c2")]).await;
    first_marks.set("c1", true);
    first.sync_remote_control_projection().await;

    let (before, _, _) = first.remote_control().projection().snapshot();
    let marked = before.changes.iter().find(|c| c.id == "c1").unwrap();
    let arrived = before.changes.iter().find(|c| c.id == "c2").unwrap();
    assert!(marked.execution_marked);
    assert_eq!(arrived.attention, AttentionState::New);

    // Restart: the same workspace and the same reducer evidence, a brand new
    // process-local store. Nothing durable carried the mark or the attention.
    let second = WebState::new(&[]);
    second.set_shared_state(reducer.clone()).await;
    second
        .set_execution_marks(Arc::new(ExecutionMarkStore::new()))
        .await;
    second.update(&[change("c1"), change("c2")]).await;

    let (after, _, _) = second.remote_control().projection().snapshot();
    assert_ne!(
        first.remote_control().projection().instance_id(),
        second.remote_control().projection().instance_id(),
        "a restart is a new incarnation"
    );
    for change in &after.changes {
        assert!(
            !change.execution_marked,
            "a restart starts with every mark false by construction"
        );
        assert_eq!(
            change.attention,
            AttentionState::None,
            "attention is process-local and is not carried forward"
        );
        assert_eq!(
            change.display_status, "not queued",
            "routing still comes from the workspace and the reducer"
        );
    }
}

#[tokio::test]
async fn attention_clears_once_the_change_is_acted_on() {
    let (web_state, _, marks) = wired_web_state(&["c1"]).await;
    web_state.update(&[change("c1")]).await;
    web_state.update(&[change("c1"), change("c2")]).await;

    let (before, _, _) = web_state.remote_control().projection().snapshot();
    let arrived = before.changes.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(arrived.attention, AttentionState::New);

    marks.set("c2", true);
    web_state.sync_remote_control_projection().await;
    let (after, _, _) = web_state.remote_control().projection().snapshot();
    let acted_on = after.changes.iter().find(|c| c.id == "c2").unwrap();
    assert_eq!(
        acted_on.attention,
        AttentionState::None,
        "an operator who marked the change has already seen it"
    );
}

#[tokio::test]
async fn a_mark_mutation_is_readable_at_the_revision_it_produced() {
    let (web_state, reducer, marks) = wired_web_state(&["c1"]).await;
    web_state.update(&[change("c1")]).await;
    let _ = reducer.write().await.apply_command(
        crate::orchestration::state::ReducerCommand::AddToQueue("c1".to_string()),
    );
    web_state.sync_remote_control_projection().await;

    let projection = web_state.remote_control().projection();
    let (before, revision_before, _) = projection.snapshot();
    assert_eq!(before.changes[0].queue_intent, QueueIntent::Queued);
    assert!(!before.changes[0].execution_marked);

    marks.set("c1", true);
    web_state.sync_remote_control_projection().await;

    let (after, revision_after, _) = projection.snapshot();
    assert_eq!(
        revision_after,
        revision_before + 1,
        "a real mutation advances the revision exactly once"
    );
    assert!(after.changes[0].execution_marked, "the new mark reads back");
    assert_eq!(
        after.changes[0].queue_intent,
        QueueIntent::Queued,
        "queue intent is reported unchanged alongside it, with no inference"
    );

    // A repeated identical mark is a no-op all the way through.
    marks.set("c1", true);
    web_state.sync_remote_control_projection().await;
    assert_eq!(projection.revision(), revision_after);
}

// ============================================================================
// Task 4 — route and schema coverage
// ============================================================================

#[tokio::test]
async fn the_state_and_change_routes_publish_the_same_operator_fields() {
    let (web_state, reducer, marks) = wired_web_state(&["c1"]).await;
    web_state.update(&[change("c1")]).await;
    observe(
        &web_state,
        &reducer,
        ExecutionEvent::ProcessingStarted("c1".to_string()),
    )
    .await;
    marks.set("c1", true);
    web_state.sync_remote_control_projection().await;

    let harness =
        harness_with_projection(web_state.remote_control().projection(), Some("tok"), &[]);

    let (_, state_body) =
        status_and_json(send(&harness.router, get("/api/v2/state", Some("tok"))).await).await;
    let from_state = projected_change(&state_body["snapshot"], "c1");

    let (status, change_body) =
        status_and_json(send(&harness.router, get("/api/v2/changes/c1", Some("tok"))).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        change_body["change"], from_state,
        "one change resource, two routes, no divergence"
    );

    let (_, list_body) =
        status_and_json(send(&harness.router, get("/api/v2/changes", Some("tok"))).await).await;
    assert_eq!(list_body["changes"][0], from_state);
}

#[test]
fn the_openapi_document_describes_every_added_schema() {
    let document = serde_json::to_value(crate::web::openapi::document()).unwrap();
    let schemas = document["components"]["schemas"]
        .as_object()
        .expect("component schemas");

    for schema in [
        "ActionBlockedReason",
        "ActionEligibility",
        "AttentionState",
        "BlockerKind",
        "ChangeActions",
        "ChangeActivity",
        "ChangeBlocker",
        "ChangeTiming",
        "ChangeWorktree",
        "ParallelBlockedReason",
        "ParallelEligibility",
        "QueueIntent",
    ] {
        assert!(
            schemas.contains_key(schema),
            "'{schema}' must be discoverable from the published schema"
        );
    }

    let change = schemas["ChangeResource"]["properties"]
        .as_object()
        .expect("ChangeResource properties");
    for field in [
        "execution_marked",
        "queue_intent",
        "attention",
        "blocker",
        "error_detail",
        "actions",
        "parallel",
        "timing",
        "latest_activity",
        "worktree",
    ] {
        assert!(
            change.contains_key(field),
            "'{field}' must be described, not just serialized"
        );
    }
    assert!(schemas["InstanceSnapshot"]["properties"]
        .as_object()
        .unwrap()
        .contains_key("process_error"));
}

#[test]
fn every_stable_reason_token_round_trips_as_snake_case() {
    // A client branches on these tokens; renaming one silently is a breaking
    // change that no type check would catch.
    for (reason, token) in [
        (ActionBlockedReason::FinalStatus, "final_status"),
        (ActionBlockedReason::RetryRequired, "retry_required"),
        (ActionBlockedReason::StopPending, "stop_pending"),
        (ActionBlockedReason::StatusImmutable, "status_immutable"),
        (ActionBlockedReason::ModeHasNoQueue, "mode_has_no_queue"),
        (
            ActionBlockedReason::NoRetryableEvidence,
            "no_retryable_evidence",
        ),
        (ActionBlockedReason::HoldNotResumable, "hold_not_resumable"),
        (ActionBlockedReason::ChangeActive, "change_active"),
        (ActionBlockedReason::NotMergeWaiting, "not_merge_waiting"),
    ] {
        assert_eq!(serde_json::to_value(reason).unwrap(), json!(token));
    }
    for (reason, token) in [
        (ParallelBlockedReason::NotCommitted, "not_committed"),
        (
            ParallelBlockedReason::UncommittedChanges,
            "uncommitted_changes",
        ),
    ] {
        assert_eq!(serde_json::to_value(reason).unwrap(), json!(token));
    }
    assert_eq!(serde_json::to_value(QueueIntent::Queued).unwrap(), "queued");
    assert_eq!(
        serde_json::to_value(BlockerKind::External).unwrap(),
        "external"
    );
    assert_eq!(serde_json::to_value(AttentionState::New).unwrap(), "new");
}
