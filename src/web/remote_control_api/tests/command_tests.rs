//! Command-endpoint tests: admission ordering, idempotency, and delegation.

use std::sync::Arc;

use axum::http::StatusCode;
use chrono::Utc;
use serde_json::json;

use crate::orchestration::operator_command::{
    MarkRoute, OperatorCommandError, OperatorMode, OperatorOutcome, QueueMutation, QueueOutcome,
    RetryPlan,
};
use crate::web::remote_control_api::dto::{
    CommandIdentity, CommandRecord, CommandSpec, CommandState, ErrorCode,
};
use crate::web::remote_control_api::executor::{
    map_operator_error, operator_mode, summarize_outcome, CommandFailure, ExecutionSummary,
};
use crate::web::remote_control_api::projection::Projection;
use crate::web::remote_control_api::registry::CommandRegistry;

use super::{
    get, harness, harness_with_projection, post_json, send, snapshot_with, status_and_json,
};

fn envelope(command: serde_json::Value, revision: u64, key: &str) -> String {
    let mut object = command.as_object().unwrap().clone();
    object.insert("expected_revision".to_string(), json!(revision));
    object.insert("idempotency_key".to_string(), json!(key));
    serde_json::Value::Object(object).to_string()
}

// ── Validation ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn an_unknown_command_type_is_rejected_without_any_service_call() {
    let h = harness(None, &[]);
    let body = envelope(json!({"type": "delete_everything"}), 0, "k1");
    let (status, response) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response["error_code"], "validation_failed");
    assert_eq!(h.executor.call_count(), 0, "no service call may occur");
}

#[tokio::test]
async fn every_command_must_carry_a_revision_and_an_idempotency_key() {
    let h = harness(None, &[]);
    for body in [
        r#"{"type":"start","idempotency_key":"k1"}"#,
        r#"{"type":"start","expected_revision":0}"#,
        r#"{"type":"stop","expected_revision":0,"idempotency_key":""}"#,
    ] {
        let (status, response) =
            status_and_json(send(&h.router, post_json("/api/v2/commands", None, body)).await).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(response["error_code"], "validation_failed", "{body}");
    }
    assert_eq!(h.executor.call_count(), 0);
}

#[tokio::test]
async fn an_invalid_correlation_id_in_the_body_fails_validation() {
    let h = harness(None, &[]);
    let body = r#"{"type":"start","expected_revision":0,"idempotency_key":"k1","correlation_id":"bad\nvalue"}"#;
    let (status, response) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, body)).await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response["error_code"], "validation_failed");
    assert_eq!(h.executor.call_count(), 0);
}

// ── Revision control ─────────────────────────────────────────────────────────

#[tokio::test]
async fn a_stale_new_command_is_rejected_before_the_service_runs() {
    let h = harness(None, &[]);
    for i in 0..12u32 {
        h.projection
            .apply_state("a", None, json!({}), snapshot_with("c1", &format!("s{i}")));
    }
    assert_eq!(h.projection.revision(), 12);

    let body = envelope(json!({"type": "retry_change", "change_id": "c1"}), 11, "k1");
    let (status, response) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(response["error_code"], "stale_revision");
    assert_eq!(response["current_revision"], 12);
    assert_eq!(
        h.executor.call_count(),
        0,
        "no side effect for a stale command"
    );
}

#[tokio::test]
async fn a_current_command_is_accepted_and_delegated_once() {
    let h = harness(None, &[]);
    let body = envelope(
        json!({"type": "set_queue_intent", "change_id": "c1", "queued": true}),
        0,
        "k1",
    );
    let (status, response) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["state"], "succeeded");
    assert_eq!(response["type"], "set_queue_intent");
    assert_eq!(response["instance_id"], h.projection.instance_id());
    assert_eq!(response["expected_revision"], 0);
    assert_eq!(h.executor.call_count(), 1);
    assert_eq!(
        h.executor.calls()[0],
        CommandSpec::SetQueueIntent {
            change_id: "c1".to_string(),
            queued: true
        }
    );
}

// ── Idempotency ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn an_exact_replay_after_the_revision_advanced_returns_the_original_record() {
    let h = harness(None, &[]);
    let body = envelope(
        json!({"type": "retry_change", "change_id": "c1"}),
        0,
        "replay-key",
    );

    let (status, first) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;
    assert_eq!(status, StatusCode::OK);
    let original_id = first["command_id"].as_str().unwrap().to_string();

    // State moves on; the same intent must still resolve to the same record.
    h.projection
        .apply_state("a", None, json!({}), snapshot_with("c1", "applying"));
    assert_eq!(h.projection.revision(), 1);

    let (status, second) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["command_id"], original_id);
    assert_eq!(
        h.executor.call_count(),
        1,
        "the side effect must not be repeated"
    );
}

#[tokio::test]
async fn a_structurally_equivalent_replay_ignores_member_order_and_whitespace() {
    let h = harness(None, &[]);
    let compact = r#"{"type":"set_queue_intent","change_id":"c1","queued":true,"expected_revision":0,"idempotency_key":"k"}"#;
    let reordered = "{\n  \"idempotency_key\" : \"k\" ,\n \"expected_revision\":0,\n\t\"queued\":true,\n \"change_id\":\"c1\",\n \"type\":\"set_queue_intent\"\n}";

    let (_, first) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, compact)).await).await;
    let (status, second) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, reordered)).await)
            .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["command_id"], first["command_id"]);
    assert_eq!(h.executor.call_count(), 1);
}

#[tokio::test]
async fn correlation_id_is_not_part_of_command_identity() {
    let h = harness(None, &[]);
    let with_trace_a =
        r#"{"type":"stop","expected_revision":0,"idempotency_key":"k","correlation_id":"trace-a"}"#;
    let with_trace_b =
        r#"{"type":"stop","expected_revision":0,"idempotency_key":"k","correlation_id":"trace-b"}"#;

    let (_, first) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, with_trace_a)).await)
            .await;
    let (status, second) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, with_trace_b)).await)
            .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["command_id"], first["command_id"]);
    assert_eq!(
        second["correlation_id"], "trace-a",
        "the replayed record keeps its original trace label"
    );
    assert_eq!(h.executor.call_count(), 1);
}

#[tokio::test]
async fn reusing_a_key_with_a_different_identity_is_a_typed_conflict() {
    let h = harness(None, &[]);
    let first = envelope(json!({"type": "retry_change", "change_id": "c1"}), 0, "k1");
    send(&h.router, post_json("/api/v2/commands", None, &first)).await;

    for conflicting in [
        // different expected revision
        r#"{"type":"retry_change","change_id":"c1","expected_revision":1,"idempotency_key":"k1"}"#,
        // different target
        r#"{"type":"retry_change","change_id":"c2","expected_revision":0,"idempotency_key":"k1"}"#,
        // different type
        r#"{"type":"stop_and_dequeue","change_id":"c1","expected_revision":0,"idempotency_key":"k1"}"#,
    ] {
        let (status, response) = status_and_json(
            send(&h.router, post_json("/api/v2/commands", None, conflicting)).await,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{conflicting}");
        assert_eq!(
            response["error_code"], "idempotency_mismatch",
            "{conflicting}"
        );
    }
    assert_eq!(h.executor.call_count(), 1, "no conflicting request may run");
}

#[tokio::test]
async fn stale_revision_and_idempotency_mismatch_are_distinguishable_at_the_same_status() {
    let h = harness(None, &[]);
    send(
        &h.router,
        post_json(
            "/api/v2/commands",
            None,
            &envelope(
                json!({"type": "retry_change", "change_id": "c1"}),
                0,
                "bound",
            ),
        ),
    )
    .await;
    h.projection
        .apply_state("a", None, json!({}), snapshot_with("c1", "applying"));

    let (stale_status, stale) = status_and_json(
        send(
            &h.router,
            post_json(
                "/api/v2/commands",
                None,
                &envelope(
                    json!({"type": "retry_change", "change_id": "c9"}),
                    0,
                    "fresh-key",
                ),
            ),
        )
        .await,
    )
    .await;
    let (mismatch_status, mismatch) = status_and_json(
        send(
            &h.router,
            post_json(
                "/api/v2/commands",
                None,
                &envelope(
                    json!({"type": "retry_change", "change_id": "c1"}),
                    1,
                    "bound",
                ),
            ),
        )
        .await,
    )
    .await;

    assert_eq!(stale_status, StatusCode::CONFLICT);
    assert_eq!(mismatch_status, StatusCode::CONFLICT);
    assert_eq!(stale["error_code"], "stale_revision");
    assert_eq!(mismatch["error_code"], "idempotency_mismatch");
}

// ── In-progress behavior and capacity ────────────────────────────────────────

#[tokio::test]
async fn a_long_running_command_is_accepted_and_replays_as_in_progress() {
    let h = harness(None, &[]);
    let gate = h.executor.block_until_released();
    let body = envelope(
        json!({"type": "stop_and_dequeue", "change_id": "c1"}),
        0,
        "slow",
    );

    let (status, first) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(first["state"], "running");

    let (status, replay) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "an in-progress replay is still 202"
    );
    assert_eq!(replay["command_id"], first["command_id"]);
    assert_eq!(h.executor.call_count(), 1);

    // Status lookup reports the same in-progress record.
    let uri = format!("/api/v2/commands/{}", first["command_id"].as_str().unwrap());
    let (status, looked_up) = status_and_json(send(&h.router, get(&uri, None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(looked_up["state"], "running");

    gate.notify_waiters();
}

#[tokio::test]
async fn capacity_pressure_from_in_progress_work_fails_closed_without_executing() {
    // A two-slot registry makes the pressure reachable without 1000 requests.
    let projection = Arc::new(Projection::with_registry(CommandRegistry::new(2, 3600)));
    let h = harness_with_projection(projection, None, &[]);
    let gate = h.executor.block_until_released();

    for i in 0..2 {
        let body = envelope(
            json!({"type": "stop_and_dequeue", "change_id": format!("c{i}")}),
            0,
            &format!("k{i}"),
        );
        let response = send(&h.router, post_json("/api/v2/commands", None, &body)).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }
    assert_eq!(h.executor.call_count(), 2);

    let body = envelope(json!({"type": "retry_change", "change_id": "c9"}), 0, "k9");
    let (status, response) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response["error_code"], "registry_capacity");
    assert_eq!(
        h.executor.call_count(),
        2,
        "a refused admission must not execute anything"
    );

    let (commands, keys) = h.projection.registry_sizes();
    assert_eq!(commands, 2, "no in-progress record was evicted");
    assert_eq!(keys, 2, "the registries stayed paired");

    gate.notify_waiters();
}

#[tokio::test]
async fn a_command_and_its_idempotency_record_are_reserved_together() {
    let h = harness(None, &[]);
    let body = envelope(json!({"type": "start"}), 0, "paired");
    send(&h.router, post_json("/api/v2/commands", None, &body)).await;

    let (commands, keys) = h.projection.registry_sizes();
    assert_eq!(commands, 1);
    assert_eq!(keys, 1);
}

// ── Outcomes ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn an_explicit_no_op_settles_as_200_without_advancing_anything() {
    let h = harness(None, &[]);
    h.executor
        .set_outcome(Ok(ExecutionSummary::no_op("already queued")));

    let body = envelope(
        json!({"type": "set_queue_intent", "change_id": "c1", "queued": true}),
        0,
        "k1",
    );
    let (status, response) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["state"], "no_op");
    assert_eq!(response["detail"], "already queued");
    assert_eq!(h.projection.revision(), 0);
}

#[tokio::test]
async fn a_typed_service_failure_is_reported_with_its_error_code() {
    let h = harness(None, &[]);
    h.executor.set_outcome(Err(CommandFailure::new(
        ErrorCode::TargetIneligible,
        "retry is not supported for 'c1' with status 'archived'",
    )));

    let body = envelope(json!({"type": "retry_change", "change_id": "c1"}), 0, "k1");
    let (status, response) =
        status_and_json(send(&h.router, post_json("/api/v2/commands", None, &body)).await).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(response["state"], "failed");
    assert_eq!(response["error_code"], "target_ineligible");
    assert!(response["detail"].as_str().unwrap().contains("archived"));
}

#[tokio::test]
async fn a_command_status_lookup_for_an_unknown_id_is_typed_not_found() {
    let h = harness(None, &[]);
    let (status, response) =
        status_and_json(send(&h.router, get("/api/v2/commands/deadbeef", None)).await).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(response["error_code"], "not_found");
}

#[tokio::test]
async fn commands_are_refused_while_no_orchestration_runtime_is_bound() {
    // The real router binds `RemoteControlRuntime` as its executor; before a run
    // exists it must refuse rather than claim acceptance.
    let runtime = Arc::new(crate::web::remote_control_api::RemoteControlRuntime::new());
    assert!(!runtime.is_bound().await);
    let router = crate::web::remote_control_api::router(
        crate::web::remote_control_api::RemoteControlState::new(
            runtime.projection(),
            Arc::new(crate::web::remote_control_api::auth::RemoteControlAuth::default()),
            runtime.clone(),
        ),
    );

    let body = envelope(json!({"type": "start"}), 0, "k1");
    let (status, response) =
        status_and_json(send(&router, post_json("/api/v2/commands", None, &body)).await).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(response["error_code"], "lifecycle_conflict");
}

// ── Shared-service mapping ───────────────────────────────────────────────────

#[test]
fn shared_service_refusals_map_to_distinguishable_error_codes() {
    let mark_in_error_mode = OperatorCommandError::MarkNotAllowed {
        change_id: "c1".to_string(),
        mode: OperatorMode::Error,
        route: MarkRoute::RetryRequired,
        display_status: "error".to_string(),
    };
    assert_eq!(
        map_operator_error(&mark_in_error_mode).error_code,
        ErrorCode::LifecycleConflict,
        "Error mode is a run-level condition the operator resolves with retry"
    );

    let immutable_row = OperatorCommandError::MarkNotAllowed {
        change_id: "c1".to_string(),
        mode: OperatorMode::Running,
        route: MarkRoute::Immutable,
        display_status: "archived".to_string(),
    };
    assert_eq!(
        map_operator_error(&immutable_row).error_code,
        ErrorCode::TargetIneligible,
        "an immutable row is about the target, not the run"
    );

    assert_eq!(
        map_operator_error(&OperatorCommandError::MissingCancellationHandle {
            change_id: "c1".to_string()
        })
        .error_code,
        ErrorCode::TargetIneligible
    );
    assert_eq!(
        map_operator_error(&OperatorCommandError::TerminationTimeout {
            change_id: "c1".to_string(),
            waited: std::time::Duration::from_secs(30),
        })
        .error_code,
        ErrorCode::RootBusy
    );
    assert_eq!(
        map_operator_error(&OperatorCommandError::CancellationFailed {
            change_id: "c1".to_string(),
            message: "no runtime".to_string(),
        })
        .error_code,
        ErrorCode::InternalError
    );
    assert_eq!(
        map_operator_error(&OperatorCommandError::RetryUnsupported {
            change_id: "c1".to_string(),
            display_status: "archived".to_string(),
        })
        .error_code,
        ErrorCode::TargetIneligible
    );
}

#[test]
fn shared_service_outcomes_distinguish_real_effects_from_no_ops() {
    let real = summarize_outcome(&OperatorOutcome::Queue(QueueOutcome {
        change_id: "c1".to_string(),
        mutation: QueueMutation::Added,
        reducer_changed: true,
        dynamic_queue_mutated: true,
        display_status: "queued".to_string(),
    }));
    assert!(real.changed);

    let duplicate = summarize_outcome(&OperatorOutcome::Queue(QueueOutcome {
        change_id: "c1".to_string(),
        mutation: QueueMutation::Added,
        reducer_changed: false,
        dynamic_queue_mutated: false,
        display_status: "queued".to_string(),
    }));
    assert!(!duplicate.changed, "a duplicate request changed nothing");

    let empty_retry = summarize_outcome(&OperatorOutcome::Retry(RetryPlan {
        change_ids: Vec::new(),
        routes: Vec::new(),
        explicit_retry: false,
    }));
    assert!(!empty_retry.changed);

    assert!(
        summarize_outcome(&OperatorOutcome::Dequeued {
            change_id: "c1".to_string()
        })
        .changed
    );
}

#[test]
fn application_mode_strings_map_onto_the_shared_operator_mode() {
    assert_eq!(operator_mode("select"), OperatorMode::Select);
    assert_eq!(operator_mode("running"), OperatorMode::Running);
    assert_eq!(operator_mode("stopping"), OperatorMode::Stopping);
    assert_eq!(operator_mode("stopped"), OperatorMode::Stopped);
    assert_eq!(operator_mode("error"), OperatorMode::Error);
    assert_eq!(operator_mode("something-new"), OperatorMode::Select);
}

#[test]
fn admission_reserves_before_any_execution_can_be_attempted() {
    // Direct projection-level check: after admission the record exists and is
    // in progress, so a crash between reservation and execution can never leave
    // an unreserved side effect.
    let projection = Projection::new();
    let request: crate::web::remote_control_api::dto::CommandRequest =
        serde_json::from_str(r#"{"type":"start","expected_revision":0,"idempotency_key":"k1"}"#)
            .unwrap();

    let admission = projection.admit(&request, "trace", Utc::now());
    let crate::web::remote_control_api::projection::Admission::Admitted(record) = admission else {
        panic!("a current command must be admitted");
    };
    assert_eq!(record.state, CommandState::Running);
    assert_eq!(projection.registry_sizes(), (1, 1));

    let stored: CommandRecord = projection.command(&record.command_id).unwrap();
    assert_eq!(stored.idempotency_key, "k1");
    assert_eq!(
        CommandIdentity {
            command: CommandSpec::Start,
            expected_revision: 0
        },
        request.identity()
    );
}
