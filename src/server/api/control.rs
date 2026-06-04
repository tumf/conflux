use super::*;

use crate::server::api::ws::list_remote_changes_in_worktree;

mod global;
mod queue;
mod selection;
mod stats;

#[cfg(test)]
pub(crate) use global::lock_control_calls_for_test;
pub(super) use global::start_single_project_run;
pub(crate) use global::CONTROL_CALLS;
pub use global::{global_control_run, global_control_status, global_control_stop};
pub use queue::stop_and_dequeue_change;
pub use selection::{toggle_all_change_selection, toggle_change_selection};
pub(super) use stats::{get_logs, get_project_history, get_stats_overview};

/// List change IDs in a project worktree that should be included in the next run.
/// Error changes are excluded unless they have been explicitly re-marked.
pub(super) async fn list_selected_change_ids_in_worktree(
    worktree_path: &std::path::Path,
    change_selections: Option<&std::collections::HashMap<String, bool>>,
    shared_orchestrator_state: &Arc<
        tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>,
    >,
) -> Vec<String> {
    let changes =
        list_remote_changes_in_worktree(worktree_path, "", "", shared_orchestrator_state).await;
    changes
        .into_iter()
        .filter(|change| {
            let explicit_selection = change_selections.and_then(|m| m.get(&change.id)).copied();
            let selected = explicit_selection.unwrap_or(true);

            if change.status == "rejected" {
                return false;
            }

            if change.status == "stalled" {
                return explicit_selection.unwrap_or(false);
            }

            if change.status == "error" {
                explicit_selection.unwrap_or(false)
            } else {
                selected
            }
        })
        .map(|change| change.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::orchestration::state::{ExecutionMode, OrchestratorState, ReducerCommand};
    use crate::server::api::test_support::{create_local_git_repo, make_state as make_base_state};
    use crate::server::api::{build_router, AppState};
    use crate::server::registry::{OrchestrationStatus, ProjectEntry};

    fn make_state(temp_dir: &TempDir, auth_token: Option<&str>) -> AppState {
        let mut state = make_base_state(temp_dir, auth_token);
        state.db = Some(crate::server::db::ServerDb::new(temp_dir.path()).unwrap());
        state
    }

    async fn add_test_project(state: &AppState) -> ProjectEntry {
        state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap()
    }

    fn create_change(temp_dir: &TempDir, entry: &ProjectEntry, change_id: &str) {
        let change_dir = temp_dir
            .path()
            .join("worktrees")
            .join(&entry.id)
            .join(&entry.branch)
            .join("openspec/changes")
            .join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# proposal\n").unwrap();
    }

    fn request(method: Method, uri: impl AsRef<str>) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri.as_ref())
            .body(Body::empty())
            .unwrap()
    }

    async fn response_json(resp: Response) -> serde_json::Value {
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn test_stats_and_logs_endpoints_require_auth() {
        let temp_dir = TempDir::new().unwrap();
        let router = build_router(make_state(&temp_dir, Some("secret-token")));

        let req = request(Method::GET, "/api/v1/stats/overview");
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let req = request(Method::GET, "/api/v1/logs");
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_toggle_change_selection_emits_change_update_without_waiting_full_state() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let entry = add_test_project(&state).await;
        create_change(&temp_dir, &entry, "fix-a");

        {
            let mut registry = state.registry.write().await;
            registry.mark_change_error(&entry.id, "fix-a", "boom".to_string());
        }

        let mut state_updates = state.state_update_tx.subscribe();
        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!(
                "/api/v1/projects/{}/changes/fix-a/toggle",
                entry.id
            ))
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = response_json(resp).await;
        assert_eq!(json["change_id"], "fix-a");
        assert_eq!(json["selected"], true);

        let update =
            tokio::time::timeout(std::time::Duration::from_millis(500), state_updates.recv())
                .await
                .expect("change_update should be emitted immediately after toggle")
                .expect("state update channel should stay open");

        match update {
            RemoteStateUpdate::ChangeUpdate { change } => {
                assert_eq!(change.project, entry.id);
                assert_eq!(change.id, "fix-a");
                assert_eq!(change.status, "error");
                assert!(change.selected);
            }
            other => panic!("Expected ChangeUpdate, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_toggle_all_change_selection_remarks_error_changes_for_next_run() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let entry = add_test_project(&state).await;
        create_change(&temp_dir, &entry, "fix-a");

        {
            let mut registry = state.registry.write().await;
            registry.mark_change_error(&entry.id, "fix-a", "boom".to_string());
        }

        let router = build_router(state.clone());
        let mut state_updates = state.state_update_tx.subscribe();

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/changes/toggle-all", entry.id))
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = response_json(resp).await;
        assert_eq!(json["selected"], true);

        let first_update =
            tokio::time::timeout(std::time::Duration::from_millis(500), state_updates.recv())
                .await
                .expect("toggle-all should emit immediate change_update")
                .expect("state update channel should stay open");
        match first_update {
            RemoteStateUpdate::ChangeUpdate { change } => {
                assert_eq!(change.project, entry.id);
                assert_eq!(change.id, "fix-a");
                assert_eq!(change.status, "error");
                assert!(change.selected);
            }
            other => panic!("Expected ChangeUpdate after toggle-all, got {other:?}"),
        }

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/state")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = response_json(resp).await;
        assert_eq!(json["projects"][0]["changes"][0]["selected"], true);
        assert_eq!(json["projects"][0]["changes"][0]["status"], "error");

        let _control_calls_guard = lock_control_calls_for_test().await;

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/control/run")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = response_json(resp).await;
        assert_eq!(json["started"], 1);
        assert_eq!(json["skipped"], 0);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/state")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = response_json(resp).await;
        assert_eq!(json["projects"][0]["changes"][0]["selected"], true);
        assert_eq!(json["projects"][0]["changes"][0]["status"], "error");
    }

    #[tokio::test]
    async fn test_stalled_change_must_be_explicitly_selected_for_global_run() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let entry = add_test_project(&state).await;
        create_change(&temp_dir, &entry, "fix-stalled");

        {
            let mut shared = state.shared_orchestrator_state.write().await;
            *shared = OrchestratorState::with_mode(
                vec!["fix-stalled".to_string()],
                0,
                ExecutionMode::Parallel,
            );
            shared.mark_stalled("fix-stalled".to_string());
            shared.apply_execution_event(&crate::events::ExecutionEvent::WorkspaceStatusUpdated {
                change_id: "fix-stalled".to_string(),
                workspace_name: "fix-stalled".to_string(),
                status: crate::vcs::WorkspaceStatus::Blocked,
            });
            assert_eq!(shared.display_status("fix-stalled"), "stalled");
        }

        let _control_calls_guard = lock_control_calls_for_test().await;

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/control/run")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = response_json(resp).await;
        assert_eq!(json["started"], 0);
        assert_eq!(json["skipped"], 1);

        {
            let mut registry = state.registry.write().await;
            registry.toggle_change_selected(&entry.id, "fix-stalled");
            registry.toggle_change_selected(&entry.id, "fix-stalled");
        }
        {
            let mut shared = state.shared_orchestrator_state.write().await;
            let outcome =
                shared.apply_command(ReducerCommand::AddToQueue("fix-stalled".to_string()));
            assert!(matches!(
                outcome,
                crate::orchestration::state::ReduceOutcome::Changed(_)
            ));
        }

        {
            let mut status = state.orchestration_status.write().await;
            *status = OrchestrationStatus::Idle;
        }

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/control/run")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = response_json(resp).await;
        assert_eq!(json["started"], 1);
        assert_eq!(json["skipped"], 0);
    }

    #[tokio::test]
    async fn test_stats_and_logs_endpoints_return_data() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = build_router(make_state(&temp_dir, None));

        let add_body = serde_json::json!({
            "remote_url": remote_url,
            "branch": "main"
        });
        let add_req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects")
            .header("Content-Type", "application/json")
            .body(Body::from(add_body.to_string()))
            .unwrap();
        let add_resp = router.clone().oneshot(add_req).await.unwrap();
        assert_eq!(add_resp.status(), StatusCode::CREATED);

        let body_bytes = axum::body::to_bytes(add_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let project_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let project_id = project_json["id"].as_str().unwrap();

        let db = crate::server::db::ServerDb::new(temp_dir.path()).unwrap();
        db.insert_change_event(
            project_id,
            "change-1",
            None,
            "apply",
            1,
            true,
            1234,
            Some(0),
            None,
            Some("ok"),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        db.insert_log(
            Some(project_id),
            "info",
            "persisted-log",
            Some("change-1"),
            Some("apply"),
            Some(1),
        )
        .unwrap();

        let overview_resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/stats/overview")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(overview_resp.status(), StatusCode::OK);

        let overview_body = axum::body::to_bytes(overview_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let overview_json: serde_json::Value = serde_json::from_slice(&overview_body).unwrap();

        assert_eq!(overview_json["summary"]["success_count"], 1);
        assert_eq!(overview_json["summary"]["failure_count"], 0);
        assert_eq!(overview_json["summary"]["in_progress_count"], 0);
        assert_eq!(overview_json["summary"]["average_duration_ms"], 1234.0);

        let recent_events = overview_json["recent_events"]
            .as_array()
            .expect("recent_events must be an array");
        assert!(!recent_events.is_empty(), "recent_events must not be empty");
        let first_event = &recent_events[0];
        assert_eq!(first_event["project_id"], project_id);
        assert_eq!(first_event["change_id"], "change-1");
        assert_eq!(first_event["operation"], "apply");
        assert_eq!(first_event["result"], "success");
        assert!(
            first_event["timestamp"].as_str().is_some(),
            "recent_events[0].timestamp must be a string"
        );

        let project_stats = overview_json["project_stats"]
            .as_array()
            .expect("project_stats must be an array");
        assert!(!project_stats.is_empty(), "project_stats must not be empty");
        let first_project_stats = &project_stats[0];
        assert_eq!(first_project_stats["project_id"], project_id);
        assert_eq!(first_project_stats["success_count"], 1);
        assert_eq!(first_project_stats["failure_count"], 0);
        assert_eq!(first_project_stats["in_progress_count"], 0);
        assert_eq!(first_project_stats["average_duration_ms"], 1234.0);
        assert_eq!(first_project_stats["apply_success_rate"], 1.0);

        let history_resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!(
                        "/api/v1/stats/projects/{}/history?limit=10",
                        project_id
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(history_resp.status(), StatusCode::OK);

        let history_body = axum::body::to_bytes(history_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let history_json: serde_json::Value = serde_json::from_slice(&history_body).unwrap();
        assert!(!history_json.as_array().unwrap().is_empty());

        let logs_resp = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/v1/logs?project_id={}&limit=10", project_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logs_resp.status(), StatusCode::OK);

        let logs_body = axum::body::to_bytes(logs_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let logs_json: serde_json::Value = serde_json::from_slice(&logs_body).unwrap();
        assert!(!logs_json.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_selected_change_ids_excludes_rejected_changes() {
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes");
        std::fs::create_dir_all(&changes_dir).unwrap();

        let active_change_dir = changes_dir.join("change-active");
        std::fs::create_dir_all(&active_change_dir).unwrap();
        std::fs::write(active_change_dir.join("proposal.md"), "# proposal\n").unwrap();

        let rejected_change_dir = changes_dir.join("change-rejected");
        std::fs::create_dir_all(&rejected_change_dir).unwrap();
        std::fs::write(rejected_change_dir.join("proposal.md"), "# proposal\n").unwrap();
        std::fs::write(rejected_change_dir.join("REJECTED.md"), "# REJECTED\n").unwrap();

        let selected = list_selected_change_ids_in_worktree(
            temp_dir.path(),
            None,
            &Arc::new(tokio::sync::RwLock::new(
                crate::orchestration::state::OrchestratorState::default(),
            )),
        )
        .await;

        assert_eq!(selected, vec!["change-active".to_string()]);
    }

    #[tokio::test]
    async fn test_global_control_run_skips_rejected_changes() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let entry = add_test_project(&state).await;
        create_change(&temp_dir, &entry, "rejected-only");
        let rejected_change_dir = temp_dir
            .path()
            .join("worktrees")
            .join(&entry.id)
            .join(&entry.branch)
            .join("openspec/changes/rejected-only");
        std::fs::write(rejected_change_dir.join("REJECTED.md"), "# REJECTED\n").unwrap();

        let _control_calls_guard = lock_control_calls_for_test().await;
        let calls = CONTROL_CALLS.get().unwrap();

        let router = build_router(state.clone());
        let resp = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/control/run")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = response_json(resp).await;
        assert_eq!(json["started"], 0);
        assert_eq!(json["skipped"], 1);

        let recorded_calls = calls.lock().unwrap().clone();
        assert_eq!(
            recorded_calls,
            vec![("_global_".to_string(), "run".to_string())],
            "project-level run must not start a per-project run when only rejected changes are present"
        );
    }

    #[tokio::test]
    async fn test_stop_and_dequeue_change_deselects_and_returns_ok() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        // Create a change on disk
        let change_dir = temp_dir
            .path()
            .join("worktrees")
            .join(&entry.id)
            .join(&entry.branch)
            .join("openspec/changes/fix-a");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# proposal\n").unwrap();

        // Select the change first
        {
            let mut registry = state.registry.write().await;
            registry.toggle_change_selected(&entry.id, "fix-a");
        }

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!(
                "/api/v1/projects/{}/changes/fix-a/stop-and-dequeue",
                entry.id
            ))
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = response_json(resp).await;
        assert_eq!(json["change_id"], "fix-a");
        assert_eq!(json["selected"], false);
        assert_eq!(json["status"], "not queued");

        // Verify change is deselected in registry
        let registry = state.registry.read().await;
        assert!(!registry.is_change_selected(&entry.id, "fix-a"));
    }

    #[tokio::test]
    async fn test_stop_and_dequeue_change_not_found_project() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        let router = build_router(state);
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects/nonexistent/changes/fix-a/stop-and-dequeue")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
