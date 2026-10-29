//! The `refresh_proposal` command and the `refresh-preview` read on the
//! `/api/v2` wire.
//!
//! The envelope tests are unit-scoped. The end-to-end tests are integration:
//! the production router, the real application transaction
//! ([`crate::web::remote_control_api::executor::wired_for_test`]), and a
//! disposable real-Git candidate with real hooks.

use std::collections::HashMap;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;

use crate::events::ExecutionEvent;
use crate::orchestration::operator_command::{
    ExecutionMarkStore, OperatorCommandService, ParallelRuntime, WorktreeBaseDirCandidates,
};
use crate::orchestration::operator_coordinator::CoreMode;
use crate::orchestration::proposal_refresh::tests::{Fixture as GitFixture, CHANGE};
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService};
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::auth::RemoteControlAuth;
use crate::web::remote_control_api::dto::{CommandSpec, ALL_ERROR_CODES, SUPPORTED_COMMANDS};
use crate::web::remote_control_api::projection::Projection;
use crate::web::remote_control_api::{router, RemoteControlState};
use crate::web::state::WebState;

use super::{get, harness, post_json, send, status_and_json};

const BUSY: &str = "beta";

fn change(id: &str) -> crate::openspec::Change {
    crate::openspec::Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: crate::openspec::ProposalMetadata::default(),
    }
}

fn refresh(change_id: &str, source: &str, head: &str) -> serde_json::Value {
    json!({
        "type": "refresh_proposal",
        "change_id": change_id,
        "source_ref": "develop",
        "expected_source": source,
        "expected_head": head,
    })
}

// ============================================================================
// Envelope (unit)
// ============================================================================

#[test]
fn candidate_proposal_refresh_is_an_advertised_closed_single_target_command() {
    assert!(SUPPORTED_COMMANDS.contains(&"refresh_proposal"));
    assert!(ALL_ERROR_CODES
        .iter()
        .any(|code| code.as_str() == "refresh_refused"));

    let spec: CommandSpec =
        serde_json::from_value(refresh(CHANGE, &"a".repeat(40), &"b".repeat(40)))
            .expect("the frozen shape parses");
    assert_eq!(spec.type_name(), "refresh_proposal");
    assert_eq!(spec.target(), Some(CHANGE));
    assert!(spec.validate().is_ok());

    let mut smuggled = refresh(CHANGE, &"a".repeat(40), &"b".repeat(40));
    smuggled["paths"] = json!(["src/lib.rs"]);
    assert!(
        serde_json::from_value::<CommandSpec>(smuggled).is_err(),
        "no caller-selected path may ride along"
    );

    for (source, head) in [
        ("a".repeat(12), "b".repeat(40)),
        ("a".repeat(40), "HEAD".to_string()),
        ("A".repeat(40), "b".repeat(40)),
    ] {
        let spec: CommandSpec = serde_json::from_value(refresh(CHANGE, &source, &head)).unwrap();
        assert!(spec.validate().is_err(), "{source} / {head}");
    }
    let blank: CommandSpec = serde_json::from_value(json!({
        "type": "refresh_proposal",
        "change_id": CHANGE,
        "source_ref": " ",
        "expected_source": "a".repeat(40),
        "expected_head": "b".repeat(40),
    }))
    .unwrap();
    assert!(blank.validate().is_err());
}

#[tokio::test]
async fn candidate_proposal_refresh_malformed_command_creates_no_record() {
    let h = harness(None, &[]);
    let mut body = refresh(CHANGE, "short", &"b".repeat(40));
    body["expected_revision"] = json!(0);
    body["idempotency_key"] = json!("k1");

    let (status, error) = status_and_json(
        send(
            &h.router,
            post_json("/api/v2/commands", None, &body.to_string()),
        )
        .await,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error_code"], "validation_failed");
    assert_eq!(h.executor.call_count(), 0);
    assert_eq!(h.projection.registry_sizes(), (0, 0));
}

#[tokio::test]
async fn candidate_proposal_refresh_preview_requires_an_explicit_source_ref() {
    let h = harness(None, &[]);
    let (status, error) = status_and_json(
        send(
            &h.router,
            get("/api/v2/proposals/alpha/refresh-preview", None),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error_code"], "validation_failed");
}

#[tokio::test]
async fn candidate_proposal_refresh_capabilities_advertise_the_command_and_preview() {
    let h = harness(None, &[]);
    let (status, caps) =
        status_and_json(send(&h.router, get("/api/v2/capabilities", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert!(caps["commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command == "refresh_proposal"));
    let refresh = &caps["proposal_refresh"];
    assert_eq!(refresh["preview_available"], true);
    assert_eq!(
        refresh["preview_route"],
        "/api/v2/proposals/{change_id}/refresh-preview"
    );
    assert_eq!(refresh["command"], "refresh_proposal");
    let reasons: Vec<&str> = refresh["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|reason| reason.as_str().unwrap())
        .collect();
    for reason in [
        "target_active",
        "no_candidate_worktree",
        "hook_rejected",
        "refresh_commit_unexpected_tree",
    ] {
        assert!(reasons.contains(&reason), "{reason}");
    }
}

#[test]
fn candidate_proposal_refresh_openapi_documents_the_route_variant_result_and_action() {
    let document = crate::web::openapi::document_yaml();
    assert!(document.contains("/api/v2/proposals/{change_id}/refresh-preview"));
    assert!(document.contains("refresh_proposal"));
    assert!(document.contains("RefreshProposalResult"));
    assert!(document.contains("RefreshPreviewResponse"));
    assert!(document.contains("refresh_refused"));
}

// ============================================================================
// Production router over the real transaction (integration)
// ============================================================================

struct Wired {
    git: GitFixture,
    router: axum::Router,
    projection: Arc<Projection>,
    reducer: Arc<tokio::sync::RwLock<OrchestratorState>>,
    marks: Arc<ExecutionMarkStore>,
    web_state: Arc<WebState>,
    scheduler: Arc<RecordingScheduler>,
    core_mode: Arc<CoreMode>,
}

impl Wired {
    async fn new() -> Self {
        let git = GitFixture::new();
        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec![CHANGE.to_string(), BUSY.to_string()],
            10,
        )));
        {
            let mut guard = reducer.write().await;
            guard.apply_execution_event(&ExecutionEvent::ProcessingError {
                id: CHANGE.to_string(),
                error: "stale verification declaration".to_string(),
            });
            guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
                change_id: BUSY.to_string(),
                command: "apply".to_string(),
            });
        }
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        let scheduler = Arc::new(RecordingScheduler::new());
        let service = Arc::new(
            OperatorCommandService::new(
                reducer.clone(),
                Arc::new(crate::tui::queue::DynamicQueue::new()),
                Arc::new(crate::orchestration::operator_command::NoopQueueHooks),
                marks.clone(),
            )
            .with_parallel(parallel.clone())
            .with_candidate_worktrees(Arc::new(WorktreeBaseDirCandidates::new(
                git.root.clone(),
                git.worktree.parent().unwrap().to_path_buf(),
            ))),
        );
        let run_control = Arc::new(RunControlService::new(
            reducer.clone(),
            service,
            scheduler.clone(),
            Arc::new(ResolveReservations::new()),
            parallel,
        ));

        let web_state = Arc::new(WebState::new(&[]));
        web_state.set_shared_state(reducer.clone()).await;
        web_state.set_execution_marks(marks.clone()).await;
        web_state
            .seed_workspace_observation_for_tests(&[change(CHANGE), change(BUSY)], "select")
            .await;
        web_state
            .seed_worktree_paths_for_tests(
                git.root.clone(),
                HashMap::from([
                    (CHANGE.to_string(), git.worktree.clone()),
                    (BUSY.to_string(), git.worktree.clone()),
                ]),
            )
            .await;

        let core_mode = Arc::new(CoreMode::new());
        let (executor, application) = crate::web::remote_control_api::executor::wired_for_test(
            reducer.clone(),
            run_control,
            web_state.clone(),
            core_mode.clone(),
        );
        let runtime = web_state.remote_control();
        runtime.bind(Arc::new(executor)).await;
        runtime.bind_gate(application.gate()).await;
        let projection = runtime.projection();
        let auth = RemoteControlAuth::new(None, &[]).expect("no-token auth is valid");
        let router = router(
            RemoteControlState::new(projection.clone(), Arc::new(auth), runtime.clone())
                .with_gate(runtime.gate())
                .with_refresh_paths(web_state.clone()),
        );

        Self {
            git,
            router,
            projection,
            reducer,
            marks,
            web_state,
            scheduler,
            core_mode,
        }
    }

    async fn preview(&self, change_id: &str) -> serde_json::Value {
        let (status, body) = status_and_json(
            send(
                &self.router,
                get(
                    &format!("/api/v2/proposals/{change_id}/refresh-preview?source_ref=develop"),
                    None,
                ),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn submit(
        &self,
        command: serde_json::Value,
        revision: u64,
        key: &str,
    ) -> (StatusCode, serde_json::Value) {
        let mut object = command.as_object().unwrap().clone();
        object.insert("expected_revision".to_string(), json!(revision));
        object.insert("idempotency_key".to_string(), json!(key));
        let body = serde_json::Value::Object(object).to_string();
        let (status, mut json) =
            status_and_json(send(&self.router, post_json("/api/v2/commands", None, &body)).await)
                .await;
        // A commit running hooks may outlive the synchronous grace window.
        while json["state"] == "running" {
            tokio::task::yield_now().await;
            let id = json["command_id"].as_str().unwrap().to_string();
            let (_, next) = status_and_json(
                send(&self.router, get(&format!("/api/v2/commands/{id}"), None)).await,
            )
            .await;
            json = next;
        }
        (status, json)
    }

    async fn actions(&self, change_id: &str) -> serde_json::Value {
        let (_, state) =
            status_and_json(send(&self.router, get("/api/v2/state", None)).await).await;
        state["snapshot"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|change| change["id"] == change_id)
            .unwrap_or_else(|| panic!("{change_id} is projected"))["actions"]["refresh_proposal"]
            .clone()
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_api_previews_then_applies_one_fenced_command() {
    let wired = Wired::new().await;

    // Per-change eligibility is the shared table.
    let alpha = wired.actions(CHANGE).await;
    assert_eq!(alpha["allowed"], true, "{alpha}");
    let beta = wired.actions(BUSY).await;
    assert_eq!(beta["allowed"], false);
    assert_eq!(beta["blocked_reason"], "target_active");

    // The preview is a read: no record, no revision.
    let revision = wired.projection.revision();
    let preview = wired.preview(CHANGE).await;
    assert_eq!(preview["eligible"], true, "{preview}");
    assert_eq!(preview["refusal"], serde_json::Value::Null);
    assert_eq!(preview["source_commit"], wired.git.develop());
    assert_eq!(preview["candidate_head"], wired.git.head());
    assert_eq!(preview["path"], "openspec/changes/alpha/proposal.md");
    assert_eq!(preview["no_op"], false);
    assert!(preview["patch"]
        .as_str()
        .unwrap()
        .contains("+verification: corrected"));
    assert_eq!(preview["instance_id"], wired.projection.instance_id());
    assert_eq!(preview["state_revision"], revision);
    assert_eq!(wired.projection.revision(), revision);
    assert_eq!(wired.projection.registry_sizes(), (0, 0));

    // An ineligible row previews its lifecycle reason without touching Git.
    let busy = wired.preview(BUSY).await;
    assert_eq!(busy["eligible"], false);
    assert_eq!(busy["refusal"], "target_active");
    assert_eq!(busy["source_commit"], serde_json::Value::Null);

    // A stale revision is the ordinary conflict, with no Git effect.
    let head = wired.git.head();
    let command = refresh(
        CHANGE,
        preview["source_commit"].as_str().unwrap(),
        preview["candidate_head"].as_str().unwrap(),
    );
    let (status, stale) = wired.submit(command.clone(), revision + 7, "stale").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["error_code"], "stale_revision");
    assert_eq!(wired.git.head(), head);

    // The apply: one record, settled with the typed result.
    let (status, record) = wired.submit(command, revision, "apply").await;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["state"], "succeeded", "{record}");
    assert_eq!(record["type"], "refresh_proposal");
    let result = &record["result"];
    assert_eq!(result["kind"], "refresh_proposal");
    assert_eq!(result["outcome"], "refreshed");
    assert_eq!(result["reason"], serde_json::Value::Null);
    assert_eq!(result["candidate_head_before"], head);
    assert_eq!(result["candidate_head_after"], wired.git.head());
    assert_eq!(result["source_commit"], wired.git.develop());
    assert_eq!(
        result["effects"],
        json!({"file_written": true, "staged": true, "committed": true, "clean_after": true})
    );
    assert_eq!(wired.git.rev(&wired.git.worktree, "HEAD^"), head);
    // No outcome was dispatched: the record settles at the admitted revision.
    assert_eq!(record["result_revision"], revision);

    // Nothing about the row moved, and nothing was started or scheduled.
    assert_eq!(wired.reducer.read().await.display_status(CHANGE), "error");
    assert!(!wired.marks.is_marked(CHANGE));
    assert!(
        wired.scheduler.calls().is_empty(),
        "{:?}",
        wired.scheduler.calls()
    );
    assert_eq!(
        wired.core_mode.get(),
        crate::orchestration::operator_command::OperatorMode::Select
    );
    assert_eq!(wired.reducer.read().await.display_status(BUSY), "applying");

    // A second apply with fresh expectations is a no-op success.
    let again = wired.preview(CHANGE).await;
    assert_eq!(again["no_op"], true);
    let (_, unchanged) = wired
        .submit(
            refresh(
                CHANGE,
                again["source_commit"].as_str().unwrap(),
                again["candidate_head"].as_str().unwrap(),
            ),
            wired.projection.revision(),
            "again",
        )
        .await;
    assert_eq!(unchanged["state"], "no_op", "{unchanged}");
    assert_eq!(unchanged["result"]["outcome"], "unchanged");
    let _ = &wired.web_state;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_api_reports_refusals_with_their_effects() {
    let wired = Wired::new().await;
    let preview = wired.preview(CHANGE).await;
    let source = preview["source_commit"].as_str().unwrap().to_string();
    let head = preview["candidate_head"].as_str().unwrap().to_string();

    // Lifecycle refusal: the published table, before any Git read.
    let (status, ineligible) = wired
        .submit(
            refresh(BUSY, &source, &head),
            wired.projection.revision(),
            "busy",
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(ineligible["state"], "failed");
    assert_eq!(ineligible["error_code"], "target_ineligible");
    assert!(ineligible["detail"]
        .as_str()
        .unwrap()
        .contains("target_active"));
    assert_eq!(ineligible.get("result"), None);

    // Command-time refusal: wrong expectation, no effect.
    let (_, mismatch) = wired
        .submit(
            refresh(CHANGE, &"0".repeat(40), &head),
            wired.projection.revision(),
            "mismatch",
        )
        .await;
    assert_eq!(mismatch["error_code"], "refresh_refused");
    assert_eq!(mismatch["result"]["reason"], "refresh_expectation_mismatch");
    assert_eq!(mismatch["result"]["effects"]["file_written"], false);
    assert_eq!(wired.git.head(), head);

    // Hook rejection: the written file stays staged and the record says so.
    wired.git.hook("echo nope >&2\nexit 1");
    let (status, rejected) = wired
        .submit(
            refresh(CHANGE, &source, &head),
            wired.projection.revision(),
            "hook",
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(rejected["state"], "failed");
    assert_eq!(rejected["error_code"], "refresh_refused");
    assert_eq!(rejected["result"]["kind"], "refresh_proposal");
    assert_eq!(rejected["result"]["reason"], "hook_rejected");
    assert_eq!(
        rejected["result"]["effects"],
        json!({"file_written": true, "staged": true, "committed": false, "clean_after": false})
    );
    assert!(rejected["result"]["hook_output_tail"]
        .as_str()
        .unwrap()
        .contains("nope"));
    assert_eq!(wired.git.head(), head);
    assert_eq!(wired.git.status(), "M  openspec/changes/alpha/proposal.md");
    assert_eq!(wired.reducer.read().await.display_status(CHANGE), "error");
}
