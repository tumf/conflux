//! The `force_stop_change` command on the `/api/v2` wire.
//!
//! Integration-scoped over the real router, the real projection owner, and the
//! real registry. What is asserted here is the *API's* half of the contract —
//! the closed command envelope, revision fencing, the typed settlement result,
//! exact idempotent replay, and per-change eligibility — not the cancellation
//! semantics, which are unit-covered next to the shared transaction.

use serde_json::json;

use crate::orchestration::execution_facts::ExecutionPhase as SharedPhase;
use crate::orchestration::operator_command::{OperatorOutcome, StopSettlement};
use crate::web::remote_control_api::dto::{ActionBlockedReason, CommandSpec, SUPPORTED_COMMANDS};
use crate::web::remote_control_api::executor::summarize_outcome;
use crate::web::remote_control_api::projection::change_actions_with_live_process_for_test;

use super::{harness, post_json, send, snapshot_with, status_and_json};

const OID: &str = "9f1c0de0c0ffee0000000000000000000000abcd";
const EPISODE: &str = "0123456789abcdef0123456789abcdef";

fn force_stopped(terminated: bool, settlement: StopSettlement) -> OperatorOutcome {
    OperatorOutcome::ForceStopped {
        change_id: "alpha".to_string(),
        execution_id: terminated.then(|| EPISODE.to_string()),
        terminated,
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

fn envelope(change_id: &str, revision: u64, key: &str) -> String {
    json!({
        "type": "force_stop_change",
        "change_id": change_id,
        "expected_revision": revision,
        "idempotency_key": key,
    })
    .to_string()
}

/// The command is part of the closed advertised set, addressed at one change.
#[test]
fn force_stop_change_is_an_advertised_single_target_command() {
    assert!(SUPPORTED_COMMANDS.contains(&"force_stop_change"));

    let spec = CommandSpec::ForceStopChange {
        change_id: "alpha".to_string(),
    };
    assert_eq!(spec.type_name(), "force_stop_change");
    assert_eq!(spec.target(), Some("alpha"));

    // Process-wide force stop stays a different command with no target at all,
    // so a caller cannot widen one into the other by adding or dropping a field.
    assert_eq!(CommandSpec::ForceStop.type_name(), "force_stop");
    assert_eq!(CommandSpec::ForceStop.target(), None);
}

/// The envelope is closed: no target list, and no smuggled extra field.
#[test]
fn force_stop_change_envelope_refuses_a_target_list_or_extra_fields() {
    let list: Result<CommandSpec, _> = serde_json::from_value(json!({
        "type": "force_stop_change",
        "change_ids": ["alpha", "beta"],
    }));
    assert!(
        list.is_err(),
        "a targeted force-stop names one change, never a list"
    );

    let smuggled: Result<CommandSpec, _> = serde_json::from_value(json!({
        "type": "force_stop_change",
        "change_id": "alpha",
        "signal": "SIGTERM",
    }));
    assert!(
        smuggled.is_err(),
        "there is no caller-selectable signal or grace window"
    );

    let ok: CommandSpec = serde_json::from_value(json!({
        "type": "force_stop_change",
        "change_id": "alpha",
    }))
    .expect("the one-target shape parses");
    assert_eq!(
        ok,
        CommandSpec::ForceStopChange {
            change_id: "alpha".to_string()
        }
    );
}

/// One command record for one change, carrying the typed settlement.
#[tokio::test]
async fn force_stop_change_settles_with_the_target_specific_termination_result() {
    let h = harness(None, &[]);
    h.executor.set_outcome(Ok(summarize_outcome(&force_stopped(
        true,
        settled_after_apply(),
    ))));

    let (status, record) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope("alpha", 0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(record["state"], "succeeded");
    assert_eq!(record["type"], "force_stop_change");
    assert_eq!(record["result"]["kind"], "force_stop_change");
    assert_eq!(record["result"]["change_id"], "alpha");
    assert_eq!(record["result"]["execution_id"], EPISODE);
    assert_eq!(record["result"]["cancelled_phase"], "acceptance");
    assert_eq!(record["result"]["last_completed_phase"], "apply");
    assert_eq!(record["result"]["terminated"], true);
    assert_eq!(record["result"]["apply_commit"]["present"], true);
    assert_eq!(record["result"]["apply_commit"]["oid"], OID);
    assert_eq!(record["result"]["effects_rolled_back"], false);

    // Exactly one command was delegated, and it was the targeted one — never a
    // process-wide ForceStop.
    let calls = h.executor.calls();
    assert_eq!(
        calls,
        vec![CommandSpec::ForceStopChange {
            change_id: "alpha".to_string()
        }]
    );

    let detail = record["detail"].as_str().expect("detail");
    assert!(detail.contains("not rolled back"), "{detail}");
    assert!(detail.contains("killed immediately"), "{detail}");
}

/// The dequeue-only settlement says plainly that nothing was signalled.
#[tokio::test]
async fn force_stop_change_dequeue_only_result_reports_no_termination() {
    let h = harness(None, &[]);
    h.executor.set_outcome(Ok(summarize_outcome(&force_stopped(
        false,
        StopSettlement::none(),
    ))));

    let (_, record) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope("alpha", 0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(record["result"]["terminated"], false);
    assert!(record["result"]["execution_id"].is_null());
    assert_eq!(record["result"]["cancelled_phase"], "none");
    assert!(record["result"]["apply_commit"]["present"].is_null());
    assert_eq!(record["result"]["effects_rolled_back"], false);

    let detail = record["detail"].as_str().expect("detail");
    assert!(detail.contains("owned no managed process"), "{detail}");
}

/// A stale revision is refused before the executor is reached at all.
#[tokio::test]
async fn force_stop_change_stale_revision_is_refused_before_any_termination() {
    let h = harness(None, &[]);
    h.projection
        .apply_state("state", None, json!({}), snapshot_with("alpha", "applying"));

    let (status, body) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope("alpha", 0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::CONFLICT);
    assert_eq!(body["error_code"], "stale_revision");
    assert_eq!(
        h.executor.call_count(),
        0,
        "revision fencing must run before anything is signalled"
    );
}

/// Exact replay returns the original settlement without killing again.
#[tokio::test]
async fn force_stop_change_exact_replay_does_not_repeat_termination() {
    let h = harness(None, &[]);
    h.executor.set_outcome(Ok(summarize_outcome(&force_stopped(
        true,
        settled_after_apply(),
    ))));

    let (_, first) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope("alpha", 0, "k1")),
        )
        .await,
    )
    .await;

    // The change was re-admitted and is running a *new* episode, and the
    // executor would now answer differently.
    h.projection
        .apply_state("state", None, json!({}), snapshot_with("alpha", "applying"));
    h.executor.set_outcome(Ok(summarize_outcome(&force_stopped(
        false,
        StopSettlement::none(),
    ))));

    let (status, replayed) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope("alpha", 0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        h.executor.call_count(),
        1,
        "a replay must not cancel a later execution episode"
    );
    assert_eq!(replayed["command_id"], first["command_id"]);
    assert_eq!(replayed["result"], first["result"]);
    assert_eq!(replayed["result_revision"], first["result_revision"]);
}

/// A refusal settles no evidence a client could read as a termination.
#[tokio::test]
async fn force_stop_change_refusal_publishes_no_settlement_result() {
    let h = harness(None, &[]);
    h.executor.set_outcome(Err(
        crate::web::remote_control_api::executor::CommandFailure::new(
            crate::web::remote_control_api::dto::ErrorCode::TargetIneligible,
            "'alpha' cannot be force-stopped with status 'merge wait'",
        ),
    ));

    let (_, record) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &envelope("alpha", 0, "k1")),
        )
        .await,
    )
    .await;

    assert_eq!(record["state"], "failed");
    assert_eq!(record["error_code"], "target_ineligible");
    assert!(
        record.get("result").is_none() || record["result"].is_null(),
        "a refused force-stop must publish no settlement result: {record}"
    );
}

/// Per-change eligibility is published, and it is the owner's own fact.
#[test]
fn force_stop_change_eligibility_is_published_per_change() {
    // A live managed process: the kill is offered.
    let live = change_actions_with_live_process_for_test("running", "applying", true);
    assert!(live.force_stop_change.allowed);
    assert!(live.force_stop_change.blocked_reason.is_none());

    // The same display status without one: refused, with a stable reason.
    let idle = change_actions_with_live_process_for_test("running", "applying", false);
    assert!(!idle.force_stop_change.allowed);
    assert_eq!(
        idle.force_stop_change.blocked_reason,
        Some(ActionBlockedReason::NoManagedProcess)
    );

    // Admitted with nothing running is still offered: it is dequeue-only.
    let queued = change_actions_with_live_process_for_test("running", "queued", false);
    assert!(queued.force_stop_change.allowed);

    // The graceful stop is unaffected by any of this.
    assert!(idle.stop_and_dequeue.allowed);

    for (status, reason) in [
        ("merged", ActionBlockedReason::FinalStatus),
        ("rejected", ActionBlockedReason::FinalStatus),
        ("merge wait", ActionBlockedReason::NoManagedProcess),
        ("resolve pending", ActionBlockedReason::NoManagedProcess),
        ("not queued", ActionBlockedReason::NotAdmitted),
        ("error", ActionBlockedReason::NotAdmitted),
        ("stalled", ActionBlockedReason::NotAdmitted),
    ] {
        let actions = change_actions_with_live_process_for_test("running", status, false);
        assert!(!actions.force_stop_change.allowed, "{status}");
        assert_eq!(
            actions.force_stop_change.blocked_reason,
            Some(reason),
            "{status}"
        );
    }
}
