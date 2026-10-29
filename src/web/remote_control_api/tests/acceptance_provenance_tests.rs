//! `ChangeResource.acceptance_provenance`: published additively through the
//! production projection and router, `null` without a settlement, documented in
//! the OpenAPI contract, and never an input to any routing decision.

use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::Value;

use crate::events::{ExecutionEvent, StalledBlocker};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::acceptance::provenance::{
    AcceptanceProvenance, CandidateDivergence, DecisionSource, DivergenceStatus, ReasonCode,
    Remediation,
};
use crate::orchestration::state::OrchestratorState;
use crate::web::state::WebState;

use super::{get, harness_with_projection, send, status_and_json};

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

fn native_tool_unavailable() -> AcceptanceProvenance {
    AcceptanceProvenance {
        decision_source: DecisionSource::NativeVerification,
        reviewer_started: false,
        reason_code: ReasonCode::ToolUnavailable,
        defect_code: None,
        verification_id: Some("focused-gate".to_string()),
        declaration_path: Some("openspec/changes/native/proposal.md".to_string()),
        candidate_commit: Some("a".repeat(40)),
        proposal_content_id: Some("b".repeat(40)),
        change_input_digest: Some("c".repeat(64)),
        program: Some("missing-tool".to_string()),
        argument_count: Some(2),
        env_names: vec!["API_TOKEN".to_string()],
        remediation: Remediation::SatisfyEnvironment,
        candidate_divergence: CandidateDivergence {
            status: DivergenceStatus::Identical,
            source_ref: Some("main".to_string()),
            source_commit: Some("d".repeat(40)),
            source_blob: Some("b".repeat(40)),
            candidate_blob: Some("b".repeat(40)),
            reason: None,
        },
    }
}

fn reviewer_fail() -> AcceptanceProvenance {
    AcceptanceProvenance {
        decision_source: DecisionSource::Reviewer,
        reviewer_started: true,
        reason_code: ReasonCode::ReviewerFail,
        verification_id: None,
        declaration_path: None,
        program: None,
        argument_count: None,
        env_names: Vec::new(),
        remediation: Remediation::RepairCandidate,
        ..native_tool_unavailable()
    }
}

/// A reducer and monitoring state wired the way the runner binds them, with
/// the same lifecycle events applied to both.
async fn wired(change_ids: &[&str]) -> (WebState, Arc<tokio::sync::RwLock<OrchestratorState>>) {
    let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        change_ids.iter().map(|id| id.to_string()).collect(),
        0,
    )));
    let web_state = WebState::new(&[]);
    web_state.set_shared_state(reducer.clone()).await;
    let changes: Vec<Change> = change_ids.iter().map(|id| change(id)).collect();
    web_state.update(&changes).await;
    (web_state, reducer)
}

async fn observe(
    web_state: &WebState,
    reducer: &Arc<tokio::sync::RwLock<OrchestratorState>>,
    event: ExecutionEvent,
) {
    reducer.write().await.apply_execution_event(&event);
    web_state.apply_execution_event(&event).await;
}

async fn native_stop(web_state: &WebState, reducer: &Arc<tokio::sync::RwLock<OrchestratorState>>) {
    observe(
        web_state,
        reducer,
        ExecutionEvent::AcceptanceGated {
            change_id: "native".to_string(),
            blocker: StalledBlocker::acceptance_external(
                "infrastructure",
                "missing-tool is absent",
            ),
        },
    )
    .await;
}

async fn settle(
    web_state: &WebState,
    reducer: &Arc<tokio::sync::RwLock<OrchestratorState>>,
    change_id: &str,
    provenance: AcceptanceProvenance,
) {
    observe(
        web_state,
        reducer,
        ExecutionEvent::AcceptanceSettled {
            change_id: change_id.to_string(),
            provenance: Box::new(provenance),
        },
    )
    .await;
}

async fn read_change(web_state: &WebState, change_id: &str) -> Value {
    web_state.sync_remote_control_projection().await;
    let harness = harness_with_projection(web_state.remote_control().projection(), None, &[]);
    let (status, body) = status_and_json(
        send(
            &harness.router,
            get(&format!("/api/v2/changes/{change_id}"), None),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["change"].clone()
}

#[tokio::test]
async fn acceptance_provenance_is_published_on_the_change_row_through_the_router() {
    let (web_state, reducer) = wired(&["native", "reviewed", "untouched"]).await;
    native_stop(&web_state, &reducer).await;
    settle(&web_state, &reducer, "native", native_tool_unavailable()).await;
    settle(&web_state, &reducer, "reviewed", reviewer_fail()).await;

    let native = read_change(&web_state, "native").await;
    assert_eq!(native["display_status"], "blocked");
    let record = &native["acceptance_provenance"];
    assert_eq!(record["decision_source"], "native_verification");
    assert_eq!(record["reviewer_started"], false);
    assert_eq!(record["reason_code"], "tool_unavailable");
    assert_eq!(record["remediation"], "satisfy_environment");
    assert_eq!(record["program"], "missing-tool");
    assert_eq!(record["argument_count"], 2);
    assert_eq!(record["env_names"], serde_json::json!(["API_TOKEN"]));
    assert_eq!(record["candidate_divergence"]["status"], "identical");
    assert_eq!(
        serde_json::from_value::<AcceptanceProvenance>(record.clone()).unwrap(),
        native_tool_unavailable(),
        "the row carries the record verbatim"
    );

    let reviewed = read_change(&web_state, "reviewed").await;
    let record = &reviewed["acceptance_provenance"];
    assert_eq!(record["decision_source"], "reviewer");
    assert_eq!(record["reviewer_started"], true);
    assert_eq!(record["reason_code"], "reviewer_fail");
    assert!(record["verification_id"].is_null());
    assert_eq!(record["remediation"], "repair_candidate");

    let untouched = read_change(&web_state, "untouched").await;
    assert!(
        untouched
            .as_object()
            .expect("change object")
            .contains_key("acceptance_provenance"),
        "the field is always present"
    );
    assert!(untouched["acceptance_provenance"].is_null());
}

/// Two owners that differ only in the record publish identical routing facts,
/// and a restarted owner — which has no record — publishes `null` with the
/// same facts again.
#[tokio::test]
async fn acceptance_provenance_is_never_a_routing_input() {
    let (with_record, with_reducer) = wired(&["native"]).await;
    let (without_record, without_reducer) = wired(&["native"]).await;
    native_stop(&with_record, &with_reducer).await;
    native_stop(&without_record, &without_reducer).await;
    settle(
        &with_record,
        &with_reducer,
        "native",
        native_tool_unavailable(),
    )
    .await;

    let mut a = read_change(&with_record, "native").await;
    let mut b = read_change(&without_record, "native").await;
    assert!(!a["acceptance_provenance"].is_null());
    assert!(b["acceptance_provenance"].is_null());
    for row in [&mut a, &mut b] {
        row.as_object_mut().unwrap().remove("acceptance_provenance");
        // Latest activity names the last observed event, which is the
        // settlement itself on one side; it is presentation, not routing.
        row.as_object_mut().unwrap().remove("latest_activity");
    }
    assert_eq!(
        a, b,
        "display status, blocker, actions, and queue intent agree"
    );

    let (with, without) = (with_reducer.read().await, without_reducer.read().await);
    assert_eq!(
        with.display_status("native"),
        without.display_status("native")
    );
    assert_eq!(
        with.change_runtime("native")
            .map(|rt| rt.queue_intent.clone()),
        without
            .change_runtime("native")
            .map(|rt| rt.queue_intent.clone())
    );
    assert_eq!(with.all_blocker_views(), without.all_blocker_views());
    assert_eq!(with.all_error_details(), without.all_error_details());
}

#[test]
fn acceptance_provenance_openapi_documents_the_field_and_every_token() {
    let document = serde_json::to_value(crate::web::openapi::document()).unwrap();
    let schemas = &document["components"]["schemas"];
    assert!(
        schemas["ChangeResource"]["properties"]
            .as_object()
            .expect("ChangeResource properties")
            .contains_key("acceptance_provenance"),
        "the field is documented on the change resource"
    );
    for schema in [
        "AcceptanceProvenance",
        "CandidateDivergence",
        "DecisionSource",
        "DivergenceStatus",
        "DivergenceUnknownReason",
        "ReasonCode",
        "Remediation",
    ] {
        assert!(schemas.get(schema).is_some(), "{schema} is documented");
    }
    let rendered = crate::web::openapi::document_yaml();
    let mut tokens: Vec<&str> = ReasonCode::ALL.iter().map(|code| code.as_str()).collect();
    tokens.extend(
        Remediation::ALL
            .iter()
            .map(|remediation| remediation.as_str()),
    );
    tokens.extend([
        "native_verification",
        "reviewer",
        "identical",
        "diverged",
        "unknown",
        "no_manifest",
        "base_unresolvable",
        "source_proposal_missing",
        "candidate_proposal_missing",
    ]);
    for token in tokens {
        assert!(rendered.contains(token), "token '{token}' is documented");
    }
}
