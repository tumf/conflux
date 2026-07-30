//! Read-resource tests: discovery, coherent snapshots, and cache suppression.

use axum::http::StatusCode;
use serde_json::json;

use crate::events::LogEntry;
use crate::web::remote_control_api::dto::{
    COMMAND_RECORD_TTL_SECS, MAX_COMMAND_RECORDS, MAX_EVENTS, MAX_LOGS, SUPPORTED_COMMANDS,
};

use super::{get, harness, send, snapshot_with, status_and_json};

#[tokio::test]
async fn capabilities_describe_the_closed_command_set_and_client_specific_transports() {
    let h = harness(Some("tok"), &[]);
    let (status, body) =
        status_and_json(send(&h.router, get("/api/v2/capabilities", Some("tok"))).await).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["api_version"], "v2");
    assert_eq!(body["instance_id"], h.projection.instance_id());
    assert_eq!(body["authentication_required"], true);

    let commands: Vec<String> = serde_json::from_value(body["commands"].clone()).unwrap();
    assert_eq!(commands, SUPPORTED_COMMANDS.to_vec());

    let transports = body["transports"].as_array().unwrap();
    let sse = transports.iter().find(|t| t["name"] == "sse").unwrap();
    assert_eq!(sse["path"], "/api/v2/events");
    assert_eq!(sse["client"], "fetch-response-streaming");
    assert_eq!(
        sse["browser_native_supported"], false,
        "native EventSource must never be advertised as supported for authenticated v2"
    );
    let ws = transports
        .iter()
        .find(|t| t["name"] == "websocket")
        .unwrap();
    assert_eq!(ws["client"], "non-browser");
    assert_eq!(ws["browser_native_supported"], false);

    assert_eq!(body["limits"]["max_events"], MAX_EVENTS);
    assert_eq!(body["limits"]["max_logs"], MAX_LOGS);
    assert_eq!(body["limits"]["max_commands"], MAX_COMMAND_RECORDS);
    assert_eq!(
        body["limits"]["max_idempotency_records"],
        MAX_COMMAND_RECORDS
    );
    assert_eq!(
        body["limits"]["command_record_ttl_secs"],
        COMMAND_RECORD_TTL_SECS
    );

    let error_codes: Vec<String> = serde_json::from_value(body["error_codes"].clone()).unwrap();
    for expected in [
        "unauthorized",
        "forbidden",
        "not_found",
        "stale_revision",
        "lifecycle_conflict",
        "target_ineligible",
        "root_busy",
        "idempotency_mismatch",
        "registry_capacity",
        "validation_failed",
        "internal_error",
    ] {
        assert!(
            error_codes.contains(&expected.to_string()),
            "{expected} missing"
        );
    }
}

#[tokio::test]
async fn capabilities_report_when_authentication_is_not_enforced() {
    let h = harness(None, &[]);
    let (_, body) = status_and_json(send(&h.router, get("/api/v2/capabilities", None)).await).await;
    assert_eq!(body["authentication_required"], false);
}

#[tokio::test]
async fn instance_reports_the_process_incarnation() {
    let h = harness(None, &[]);
    let (status, body) =
        status_and_json(send(&h.router, get("/api/v2/instance", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["instance_id"], h.projection.instance_id());
    assert_eq!(body["started_at"], h.projection.started_at());
    assert_eq!(body["api_version"], "v2");
    assert_eq!(body["pid"], std::process::id());
}

#[tokio::test]
async fn state_returns_a_coherent_snapshot_with_its_revision_and_cursor() {
    let h = harness(None, &[]);
    h.projection.apply_state(
        "processing_started",
        Some("c1".to_string()),
        json!({}),
        snapshot_with("c1", "applying"),
    );

    let (status, body) = status_and_json(send(&h.router, get("/api/v2/state", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["instance_id"], h.projection.instance_id());
    assert_eq!(body["state_revision"], 1);
    assert_eq!(body["event_sequence"], 1);
    assert_eq!(body["snapshot"]["app_mode"], "running");
    assert_eq!(body["snapshot"]["changes"][0]["id"], "c1");
    assert_eq!(body["snapshot"]["changes"][0]["display_status"], "applying");
    assert_eq!(body["snapshot"]["totals"]["total"], 1);
}

#[tokio::test]
async fn read_resources_forbid_caching() {
    let h = harness(None, &[]);
    for path in [
        "/api/v2/health",
        "/api/v2/capabilities",
        "/api/v2/instance",
        "/api/v2/state",
        "/api/v2/changes",
        "/api/v2/logs",
    ] {
        let response = send(&h.router, get(path, None)).await;
        assert_eq!(
            response.headers().get("cache-control").unwrap(),
            "no-store",
            "{path} describes a live process and must never be cached"
        );
    }
}

#[tokio::test]
async fn changes_expose_reducer_derived_display_statuses() {
    let h = harness(None, &[]);
    h.projection
        .apply_state("a", None, json!({}), snapshot_with("c1", "merge wait"));

    let (status, body) = status_and_json(send(&h.router, get("/api/v2/changes", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state_revision"], 1);
    assert_eq!(body["changes"][0]["display_status"], "merge wait");
    assert_eq!(body["changes"][0]["progress_status"], "in_progress");
    assert_eq!(body["changes"][0]["completed_tasks"], 1);
    assert_eq!(body["changes"][0]["total_tasks"], 3);
}

#[tokio::test]
async fn a_single_change_can_be_fetched_and_a_missing_one_is_typed_not_found() {
    let h = harness(None, &[]);
    h.projection
        .apply_state("a", None, json!({}), snapshot_with("c1", "queued"));

    let (status, body) =
        status_and_json(send(&h.router, get("/api/v2/changes/c1", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["change"]["id"], "c1");

    let (status, body) =
        status_and_json(send(&h.router, get("/api/v2/changes/nope", None)).await).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error_code"], "not_found");
    assert_eq!(
        body["current_revision"], 1,
        "even a miss tells the client where the state is"
    );
}

#[tokio::test]
async fn logs_are_returned_oldest_first_and_bounded() {
    let h = harness(None, &[]);
    for i in 0..(MAX_LOGS + 3) {
        h.projection.apply_log(LogEntry::info(format!("line {i}")));
    }

    let (status, body) = status_and_json(send(&h.router, get("/api/v2/logs", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    let logs = body["logs"].as_array().unwrap();
    assert_eq!(logs.len(), MAX_LOGS);
    assert_eq!(logs[0]["message"], "line 3");
    assert_eq!(
        body["state_revision"], 0,
        "observational logs never advance the revision"
    );
    assert_eq!(body["event_sequence"], MAX_LOGS + 3);
}
