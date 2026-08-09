//! `/api/v2/execution-status`: what is actually happening, and what is private.
//!
//! These are integration-scoped over the real router, the real projection owner,
//! and the real facts store: the properties being proved are about the resource
//! a client actually reaches, not about a DTO's `serde` derive.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use serde_json::Value;

use crate::events::{ExecutionEvent, LogEntry, LogLevel};
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::dto::{
    InstanceSnapshot, ALL_CHANGE_EXECUTION_STATES, ALL_EXECUTION_PHASES,
};

use super::{
    change_resource, get, harness, json_body, send, snapshot_with, status_and_json, Harness,
};

const TOKEN: &str = "t0ken";

const STATUS_PATH: &str = "/api/v2/execution-status";

fn log_for(change_id: Option<&str>, message: &str, seconds: i64) -> LogEntry {
    LogEntry {
        timestamp: "12:00:00".to_string(),
        created_at: Utc
            .timestamp_opt(1_700_000_000 + seconds, 0)
            .single()
            .expect("fixed instant"),
        message: message.to_string(),
        color: ratatui::style::Color::White,
        level: LogLevel::Info,
        change_id: change_id.map(str::to_string),
        operation: Some("apply".to_string()),
        iteration: Some(2),
        workspace_path: Some("/private/host/workspaces/alpha".to_string()),
    }
}

/// Drive the reducer and the facts store the way the dispatch owner does.
fn observe(harness: &Harness, state: &mut OrchestratorState, event: ExecutionEvent, id: u64) {
    state.apply_execution_event(&event);
    harness
        .execution_facts
        .observe(id, &event, Some(state), Utc::now());
}

fn snapshot_of(changes: &[(&str, &str)]) -> InstanceSnapshot {
    let mut snapshot = InstanceSnapshot::empty();
    snapshot.app_mode = "running".to_string();
    snapshot.changes = changes
        .iter()
        .map(|(id, status)| change_resource(id, status))
        .collect();
    snapshot.totals.total = changes.len();
    snapshot
}

async fn read_status(harness: &Harness) -> Value {
    let response = send(&harness.router, get(STATUS_PATH, Some(TOKEN))).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    json_body(response).await
}

/// The headline case: Apply completed, acceptance is running, and the response
/// says so in typed values with absolute instants and no relative time.
#[tokio::test]
async fn agent_execution_observability_status_reports_phase_boundaries_absolutely() {
    let harness = harness(Some(TOKEN), &[]);
    let mut state = OrchestratorState::new(vec!["alpha".to_string()], 0);
    harness.boundary.set_running(true);

    observe(
        &harness,
        &mut state,
        ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "agent apply".to_string(),
        },
        1,
    );
    observe(
        &harness,
        &mut state,
        ExecutionEvent::ApplyCompleted {
            change_id: "alpha".to_string(),
            revision: "abc123".to_string(),
        },
        2,
    );
    observe(
        &harness,
        &mut state,
        ExecutionEvent::AcceptanceStarted {
            change_id: "alpha".to_string(),
            command: "agent accept".to_string(),
        },
        3,
    );
    harness.projection.apply_state(
        "acceptance_started",
        None,
        Value::Null,
        snapshot_of(&[("alpha", "accepting")]),
    );

    let body = read_status(&harness).await;

    let change = &body["changes"][0];
    assert_eq!(change["id"], "alpha");
    assert_eq!(change["current_phase"], "acceptance");
    assert_eq!(change["last_completed_phase"], "apply");
    assert_eq!(change["execution_state"], "active");
    assert_eq!(body["process"]["scheduler_running"], true);
    assert_eq!(body["process"]["has_active_work"], true);

    for field in ["phase_started_at", "last_completed_at"] {
        let value = change[field].as_str().unwrap_or_else(|| {
            panic!(
                "{field} must be an absolute instant, got {:?}",
                change[field]
            )
        });
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap_or_else(|_| panic!("{field} must be RFC 3339, got {value}"));
        assert!(
            value.ends_with("+00:00") || value.ends_with('Z'),
            "{field} must be UTC"
        );
    }
    chrono::DateTime::parse_from_rfc3339(body["observed_at"].as_str().expect("observed_at"))
        .expect("observed_at must be RFC 3339");

    // The whole point of `observed_at` is that no elapsed value is published.
    let serialized = serde_json::to_string(&body).expect("serializable");
    for forbidden in [
        "elapsed_seconds",
        "age_seconds",
        "elapsed_ms",
        "seconds_ago",
        "ago",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "the resource must publish no relative-time field, found '{forbidden}'"
        );
    }
}

/// `cflx run` shape: no `EventDispatcher` exists, so the only thing that reaches
/// the store is the orchestrator's forwarder calling
/// `WebState::apply_execution_event`.
///
/// Everything above drives the store the way the TUI's dispatch owner does, with
/// the reducer state travelling on the dispatch. Run mode has no such owner: it
/// moves the shared reducer inside the parallel executor and hands the same
/// typed event to `WebState` afterwards, which reads the reducer back. This is
/// integration-scoped over the real `WebState`, the real app router, and the two
/// bindings `main.rs` makes for a run — a resource that publishes
/// `has_active_work: false` while acceptance is running is exactly the
/// affirmative wrong answer this contract exists to prevent.
#[tokio::test]
async fn agent_execution_observability_status_follows_run_mode_forwarded_events() {
    /// One forwarded event: the executor moves the reducer, then the
    /// orchestrator's web channel delivers the same event. No dispatch owner.
    async fn forward(
        shared: &Arc<tokio::sync::RwLock<OrchestratorState>>,
        web_state: &Arc<crate::web::state::WebState>,
        event: ExecutionEvent,
    ) {
        shared.write().await.apply_execution_event(&event);
        web_state.apply_execution_event(&event).await;
    }

    let change = crate::openspec::Change {
        id: "alpha".to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: crate::openspec::ProposalMetadata::default(),
    };
    let web_state = Arc::new(crate::web::state::WebState::new(&[change]));
    // The two bindings a run makes: the process-local facts store (`main.rs`,
    // `Commands::Run`) and the shared reducer (`Orchestrator::set_web_state`).
    web_state
        .set_execution_facts(Arc::new(
            crate::orchestration::execution_facts::ExecutionFactsStore::new(),
        ))
        .await;
    let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["alpha".to_string()],
        0,
    )));
    web_state.set_shared_state(shared.clone()).await;

    let config = crate::web::WebConfig::enabled(0, "127.0.0.1".to_string());
    let router = crate::web::build_app_for_test(&config, web_state.clone());
    let read = || async {
        let response = send(&router, get(STATUS_PATH, None)).await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        json_body(response).await
    };

    forward(
        &shared,
        &web_state,
        ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "agent apply".to_string(),
        },
    )
    .await;
    forward(
        &shared,
        &web_state,
        ExecutionEvent::ApplyCompleted {
            change_id: "alpha".to_string(),
            revision: "abc123".to_string(),
        },
    )
    .await;
    forward(
        &shared,
        &web_state,
        ExecutionEvent::AcceptanceStarted {
            change_id: "alpha".to_string(),
            command: "agent accept".to_string(),
        },
    )
    .await;

    let body = read().await;
    assert_eq!(
        body["process"]["has_active_work"], true,
        "a run forwarding typed events must not report an idle process"
    );
    let change = body["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .find(|change| change["id"] == "alpha")
        .expect("alpha")
        .clone();
    assert_eq!(change["current_phase"], "acceptance");
    assert_eq!(change["last_completed_phase"], "apply");
    assert_eq!(change["execution_state"], "active");

    // The typed terminal for that phase, forwarded the same way, must close it
    // rather than leave the run looking busy forever.
    forward(
        &shared,
        &web_state,
        ExecutionEvent::AcceptanceCompleted {
            change_id: "alpha".to_string(),
        },
    )
    .await;

    let body = read().await;
    assert_eq!(body["process"]["has_active_work"], false);
    let change = body["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .find(|change| change["id"] == "alpha")
        .expect("alpha")
        .clone();
    assert_eq!(change["current_phase"], "none");
    assert_eq!(
        change["last_completed_phase"], "acceptance",
        "the completion the forwarder delivered is the one recorded"
    );
}

/// A live scheduler with nothing admitted is not active work. This is the exact
/// ambiguity the resource exists to remove.
#[tokio::test]
async fn agent_execution_observability_status_idle_scheduler_is_not_active_work() {
    let harness = harness(Some(TOKEN), &[]);
    harness.boundary.set_running(true);
    let mut state = OrchestratorState::new(vec!["alpha".to_string()], 0);
    observe(
        &harness,
        &mut state,
        ExecutionEvent::PersistentSchedulerIdle,
        1,
    );
    harness.projection.apply_state(
        "persistent_scheduler_idle",
        None,
        Value::Null,
        snapshot_of(&[("alpha", "queued")]),
    );

    let body = read_status(&harness).await;

    assert_eq!(body["process"]["scheduler_running"], true);
    assert_eq!(
        body["process"]["has_active_work"], false,
        "a parked scheduler has admitted nothing"
    );
    assert_eq!(body["changes"][0]["current_phase"], "none");
    assert_eq!(
        body["process"]["active_activities"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
}

/// A log arriving on its own changes the observation without invalidating a
/// client's optimistic concurrency token.
#[tokio::test]
async fn agent_execution_observability_status_log_only_preserves_state_revision() {
    let harness = harness(Some(TOKEN), &[]);
    harness.projection.apply_state(
        "state",
        None,
        Value::Null,
        snapshot_with("alpha", "applying"),
    );

    let before = read_status(&harness).await;
    let revision = before["state_revision"].as_u64().expect("revision");
    let sequence = before["event_sequence"].as_u64().expect("sequence");
    assert!(before["changes"][0]["latest_log"].is_null());

    harness
        .projection
        .apply_log(log_for(Some("alpha"), "apply iteration 2", 10));

    let after = read_status(&harness).await;
    assert_eq!(
        after["state_revision"].as_u64(),
        Some(revision),
        "a log must never advance the state revision"
    );
    assert!(
        after["event_sequence"].as_u64().expect("sequence") > sequence,
        "a log still advances the observation cursor"
    );
    assert_eq!(
        after["changes"][0]["latest_log"]["message"],
        "apply iteration 2"
    );
}

/// Association is structural. A line that merely mentions a change ID is not
/// that change's log.
#[tokio::test]
async fn agent_execution_observability_status_latest_log_requires_exact_association() {
    let harness = harness(Some(TOKEN), &[]);
    harness.projection.apply_state(
        "state",
        None,
        Value::Null,
        snapshot_of(&[("alpha", "applying"), ("alpha-two", "queued")]),
    );

    harness
        .projection
        .apply_log(log_for(Some("alpha"), "oldest for alpha", 1));
    harness
        .projection
        .apply_log(log_for(Some("alpha"), "newest for alpha", 2));
    // Structurally unassociated, and it names `alpha` in its text.
    harness
        .projection
        .apply_log(log_for(None, "process line mentioning alpha", 3));
    // A different change whose ID has `alpha` as a prefix.
    harness
        .projection
        .apply_log(log_for(Some("alpha-two"), "line for alpha-two", 4));

    let body = read_status(&harness).await;
    let changes = body["changes"].as_array().expect("changes");
    let alpha = changes.iter().find(|c| c["id"] == "alpha").expect("alpha");
    let alpha_two = changes
        .iter()
        .find(|c| c["id"] == "alpha-two")
        .expect("alpha-two");

    assert_eq!(
        alpha["latest_log"]["message"], "newest for alpha",
        "selection is by insertion order over exactly associated entries"
    );
    assert_eq!(alpha_two["latest_log"]["message"], "line for alpha-two");
    assert_eq!(
        body["process"]["latest_log"]["message"], "line for alpha-two",
        "the process line is simply the newest retained entry"
    );
}

/// A change with no retained line reports `null` rather than borrowing another's.
#[tokio::test]
async fn agent_execution_observability_status_absent_log_is_null() {
    let harness = harness(Some(TOKEN), &[]);
    harness.projection.apply_state(
        "state",
        None,
        Value::Null,
        snapshot_of(&[("alpha", "applying"), ("beta", "queued")]),
    );
    harness
        .projection
        .apply_log(log_for(Some("alpha"), "only alpha", 1));

    let body = read_status(&harness).await;
    let beta = body["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .find(|c| c["id"] == "beta")
        .expect("beta")
        .clone();
    assert!(beta["latest_log"].is_null());
}

/// The latest-log projection is a new closed shape, not the retained wire entry:
/// it carries no display timestamp and no workspace path.
#[tokio::test]
async fn agent_execution_observability_privacy_latest_log_omits_locators() {
    let harness = harness(Some(TOKEN), &[]);
    harness.projection.apply_state(
        "state",
        None,
        Value::Null,
        snapshot_with("alpha", "applying"),
    );
    harness
        .projection
        .apply_log(log_for(Some("alpha"), "apply iteration 2", 5));

    let body = read_status(&harness).await;
    let log = &body["changes"][0]["latest_log"];
    let object = log.as_object().expect("latest_log object");

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["created_at", "iteration", "level", "message", "operation"],
        "the projection is closed: adding a field here is a contract change"
    );
    assert!(!object.contains_key("timestamp"));
    assert!(!object.contains_key("workspace_path"));
    chrono::DateTime::parse_from_rfc3339(log["created_at"].as_str().expect("created_at"))
        .expect("created_at is RFC 3339, not epoch seconds");
}

/// No log locator reaches a client through any v2 read, on either transport.
///
/// The retained entry deliberately carries a workspace path. `/api/v2/logs`
/// keeps publishing it — that field predates this change and is explicitly out
/// of scope — but nothing may turn it, or anything else, into a *log locator*:
/// no persistent log path, no file URL, no filename to fetch.
#[tokio::test]
async fn agent_execution_observability_privacy_no_log_locator_is_disclosed() {
    let harness = harness(Some(TOKEN), &[]);
    harness.projection.apply_state(
        "state",
        None,
        Value::Null,
        snapshot_with("alpha", "applying"),
    );
    harness
        .projection
        .apply_log(log_for(Some("alpha"), "apply iteration 2", 5));

    for path in [
        STATUS_PATH,
        "/api/v2/logs",
        "/api/v2/capabilities",
        "/api/v2/state",
    ] {
        let response = send(&harness.router, get(path, Some(TOKEN))).await;
        let serialized = serde_json::to_string(&json_body(response).await).expect("serializable");
        for forbidden in ["file://", "log_path", "log_file", "logfile", ".log"] {
            assert!(
                !serialized.contains(forbidden),
                "{path} disclosed '{forbidden}'"
            );
        }
    }

    // The execution status itself carries no host path at all, not even the one
    // the retained entry it projects from holds.
    let body = read_status(&harness).await;
    let serialized = serde_json::to_string(&body).expect("serializable");
    assert!(
        !serialized.contains("/private/host/workspaces"),
        "the execution status must project no host path: {serialized}"
    );
}

/// `/api/v2/logs` keeps its existing schema: this change adds a compact
/// projection, it does not narrow or relocate the full retained ring.
#[tokio::test]
async fn agent_execution_observability_privacy_logs_resource_is_unchanged() {
    let harness = harness(Some(TOKEN), &[]);
    harness
        .projection
        .apply_log(log_for(Some("alpha"), "apply iteration 2", 5));

    let body = json_body(send(&harness.router, get("/api/v2/logs", Some(TOKEN))).await).await;
    let entry = body["logs"][0].as_object().expect("retained entry");
    assert!(entry.contains_key("message"));
    assert!(entry.contains_key("timestamp"));
    assert!(entry.contains_key("created_at"));
    assert!(
        !entry.contains_key("path") && !entry.contains_key("log_path"),
        "no log-file locator may be introduced"
    );
}

/// The new route follows the same bearer policy as every other v2 read.
#[tokio::test]
async fn agent_execution_observability_privacy_route_requires_authentication() {
    let harness = harness(Some(TOKEN), &[]);

    let (status, body) = status_and_json(send(&harness.router, get(STATUS_PATH, None)).await).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "unauthorized");

    let (status, _) =
        status_and_json(send(&harness.router, get(STATUS_PATH, Some("wrong"))).await).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);

    let response = send(&harness.router, get(STATUS_PATH, Some(TOKEN))).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
}

/// Process-level episodes are real work even though they address no change.
#[tokio::test]
async fn agent_execution_observability_status_process_activity_is_active_work() {
    let harness = harness(Some(TOKEN), &[]);
    harness.boundary.set_running(true);
    harness.execution_facts.observe(
        1,
        &ExecutionEvent::ConflictResolutionStarted,
        None,
        Utc::now(),
    );

    let body = read_status(&harness).await;
    assert_eq!(body["process"]["has_active_work"], true);
    assert_eq!(
        body["process"]["active_activities"],
        serde_json::json!(["conflict_resolution"])
    );

    harness.execution_facts.observe(
        2,
        &ExecutionEvent::ConflictResolutionCompleted,
        None,
        Utc::now(),
    );
    let body = read_status(&harness).await;
    assert_eq!(body["process"]["has_active_work"], false);
}

/// A change the store has never observed is explicitly unknown, not "idle".
#[tokio::test]
async fn agent_execution_observability_status_unobserved_change_is_unknown() {
    let harness = harness(Some(TOKEN), &[]);
    harness.projection.apply_state(
        "state",
        None,
        Value::Null,
        snapshot_of(&[("alpha", "queued")]),
    );

    let body = read_status(&harness).await;
    assert_eq!(body["changes"][0]["current_phase"], "unknown");
    assert_eq!(body["changes"][0]["execution_state"], "unknown");
    assert!(body["changes"][0]["last_completed_phase"].is_null());
    assert!(body["changes"][0]["phase_started_at"].is_null());
}

/// Every published phase and execution-state token comes from the closed
/// vocabularies the contract advertises.
#[tokio::test]
async fn agent_execution_observability_status_uses_closed_vocabularies() {
    let harness = harness(Some(TOKEN), &[]);
    let mut state = OrchestratorState::new(vec!["alpha".to_string()], 0);
    observe(
        &harness,
        &mut state,
        ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "agent apply".to_string(),
        },
        1,
    );
    harness.projection.apply_state(
        "state",
        None,
        Value::Null,
        snapshot_of(&[("alpha", "applying")]),
    );

    let body = read_status(&harness).await;
    let change = &body["changes"][0];
    assert!(ALL_EXECUTION_PHASES.contains(&change["current_phase"].as_str().expect("phase")));
    assert!(
        ALL_CHANGE_EXECUTION_STATES.contains(&change["execution_state"].as_str().expect("state"))
    );
}

/// The snapshot and the log ring are read together, so a client never sees a log
/// line attached to a snapshot from a different instant.
#[tokio::test]
async fn agent_execution_observability_projection_observation_is_coherent() {
    let projection = Arc::new(crate::web::remote_control_api::projection::Projection::new());
    projection.apply_state(
        "state",
        None,
        Value::Null,
        snapshot_of(&[("alpha", "applying"), ("beta", "queued")]),
    );
    projection.apply_log(log_for(Some("alpha"), "first", 1));
    projection.apply_log(log_for(Some("alpha"), "second", 2));
    projection.apply_log(log_for(Some("gamma"), "untracked change", 3));

    let observation = projection.execution_observation();
    let (snapshot, revision, sequence) = projection.snapshot();

    assert_eq!(observation.state_revision, revision);
    assert_eq!(observation.event_sequence, sequence);
    assert_eq!(observation.snapshot, snapshot);
    assert_eq!(
        observation.change_logs["alpha"].message, "second",
        "insertion order decides, not the second-precision timestamp"
    );
    assert!(
        !observation.change_logs.contains_key("gamma"),
        "a change the snapshot does not carry is not selected for"
    );
    assert!(!observation.change_logs.contains_key("beta"));
    assert_eq!(
        observation
            .process_log
            .as_ref()
            .map(|log| log.message.as_str()),
        Some("untracked change"),
        "the process line is the newest retained entry regardless of association"
    );
}
