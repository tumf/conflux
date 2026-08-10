//! The typed `stop_and_dequeue` command result on the wire.
//!
//! Integration-scoped over the real router, the real projection owner, and the
//! real registry: what matters is that a client reading a settled record — or
//! replaying it — gets settlement evidence that never changes, not that a struct
//! serializes.

use serde_json::json;

use crate::orchestration::execution_facts::ExecutionPhase as SharedPhase;
use crate::orchestration::operator_command::{OperatorOutcome, StopSettlement};
use crate::web::remote_control_api::executor::{summarize_outcome, ExecutionSummary};

use super::{harness, post_json, send, snapshot_with, status_and_json};

const OID: &str = "9f1c0de0c0ffee0000000000000000000000abcd";

fn dequeued(settlement: StopSettlement) -> OperatorOutcome {
    OperatorOutcome::Dequeued {
        change_id: "alpha".to_string(),
        settlement,
    }
}

fn settled_after_apply() -> StopSettlement {
    StopSettlement {
        cancelled_phase: SharedPhase::Acceptance,
        last_completed_phase: Some(SharedPhase::Apply),
        apply_commit_present: Some(true),
        apply_commit_oid: Some(OID.to_string()),
    }
}

fn envelope(revision: u64, key: &str) -> String {
    json!({
        "type": "stop_and_dequeue",
        "change_id": "alpha",
        "expected_revision": revision,
        "idempotency_key": key,
    })
    .to_string()
}

/// A settled successful stop carries the closed typed result a machine consumer
/// branches on, alongside a detail that denies rollback in prose.
#[tokio::test]
async fn agent_execution_observability_stop_result_is_published_on_the_record() {
    let h = harness(None, &[]);
    h.executor
        .set_outcome(Ok(summarize_outcome(&dequeued(settled_after_apply()))));

    let (status, record) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope(0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(record["state"], "succeeded");
    assert_eq!(record["result"]["kind"], "stop_and_dequeue");
    assert_eq!(record["result"]["cancelled_phase"], "acceptance");
    assert_eq!(record["result"]["last_completed_phase"], "apply");
    assert_eq!(record["result"]["apply_commit"]["present"], true);
    assert_eq!(record["result"]["apply_commit"]["oid"], OID);
    assert_eq!(record["result"]["effects_rolled_back"], false);

    let detail = record["detail"].as_str().expect("detail");
    assert!(detail.contains("acceptance"), "{detail}");
    assert!(detail.contains("not rolled back"), "{detail}");
}

/// Unknown evidence is published as `null`, never collapsed into `false`.
#[tokio::test]
async fn agent_execution_observability_stop_result_unknown_evidence_serializes_as_null() {
    let h = harness(None, &[]);
    h.executor
        .set_outcome(Ok(summarize_outcome(&dequeued(StopSettlement::none()))));

    let (_, record) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope(0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(record["result"]["cancelled_phase"], "none");
    assert!(record["result"]["last_completed_phase"].is_null());
    assert!(record["result"]["apply_commit"]["present"].is_null());
    assert!(record["result"]["apply_commit"]["oid"].is_null());
    assert_eq!(record["result"]["effects_rolled_back"], false);
}

/// An exact replay returns the original evidence even after the world moved on.
///
/// This is what stops a retrying client from being handed a *fresh* reading of a
/// repository that has since changed, which would be a different answer to the
/// same question.
#[tokio::test]
async fn agent_execution_observability_stop_result_replay_is_stable() {
    let h = harness(None, &[]);
    h.executor
        .set_outcome(Ok(summarize_outcome(&dequeued(settled_after_apply()))));

    let (_, first) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope(0, "k1")),
        )
        .await,
    )
    .await;

    // Later lifecycle state, and an executor that would now answer differently.
    h.projection
        .apply_state("state", None, json!({}), snapshot_with("alpha", "stopped"));
    h.executor
        .set_outcome(Ok(summarize_outcome(&dequeued(StopSettlement::none()))));

    let (status, replayed) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope(0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        h.executor.call_count(),
        1,
        "a replay must not re-run cancellation, dequeue, or Git observation"
    );
    assert_eq!(replayed["command_id"], first["command_id"]);
    assert_eq!(replayed["result"], first["result"]);
    assert_eq!(replayed["detail"], first["detail"]);
    assert_eq!(replayed["result_revision"], first["result_revision"]);
}

/// A refusal settled no evidence, so it must carry no settlement result that a
/// client could read as one.
#[tokio::test]
async fn agent_execution_observability_stop_result_absent_on_failure() {
    let h = harness(None, &[]);
    h.executor.set_outcome(Err(
        crate::web::remote_control_api::executor::CommandFailure::new(
            crate::web::remote_control_api::dto::ErrorCode::RootBusy,
            "termination did not confirm",
        ),
    ));

    let (_, record) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope(0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(record["state"], "failed");
    assert!(
        record.get("result").is_none() || record["result"].is_null(),
        "a failed stop must publish no settlement result: {record}"
    );
}

/// Commands with no typed result are unaffected: `result` is simply absent.
#[tokio::test]
async fn agent_execution_observability_stop_result_other_commands_are_unchanged() {
    let h = harness(None, &[]);
    h.executor
        .set_outcome(Ok(ExecutionSummary::changed("graceful stop requested")));

    let (_, record) = status_and_json(
        send(
            &h.router,
            post_json(
                "/api/v2/commands",
                None,
                &json!({
                    "type": "stop",
                    "expected_revision": 0,
                    "idempotency_key": "k1",
                })
                .to_string(),
            ),
        )
        .await,
    )
    .await;

    assert_eq!(record["state"], "succeeded");
    assert!(
        record.get("result").is_none(),
        "an omitted key, not a null: existing clients see no new field"
    );
}
