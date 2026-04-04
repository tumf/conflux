use super::*;

use crate::server::api::ws::list_remote_changes_in_worktree;

// ─────────────────────────── /api/v1/control (global) ─────────────────────────

// ──────────────── Change selection toggle ────────────────────────────────────

/// POST /api/v1/projects/:id/changes/:change_id/toggle
///
/// Toggles the `selected` state of a single change. Returns the new state.
pub async fn toggle_change_selection(
    State(state): State<AppState>,
    Path((project_id, change_id)): Path<(String, String)>,
) -> Response {
    let mut registry = state.registry.write().await;
    if registry.get(&project_id).is_none() {
        return error_response(StatusCode::NOT_FOUND, "Project not found");
    }
    let new_selected = registry.toggle_change_selected(&project_id, &change_id);
    if new_selected {
        registry.clear_change_error(&project_id, &change_id);
    }

    if let Some(db) = &state.db {
        let error_message = registry
            .error_changes_for_project(&project_id)
            .and_then(|m| m.get(&change_id))
            .map(std::string::String::as_str);
        if let Err(e) = db.upsert_change_state(&project_id, &change_id, new_selected, error_message)
        {
            error!(project_id = %project_id, change_id = %change_id, error = %e, "Failed to persist change toggle state");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to persist change state: {}", e),
            );
        }
    }

    info!(

        project_id = %project_id,
        change_id = %change_id,
        selected = new_selected,
        "Change selection toggled"
    );
    (
        StatusCode::OK,
        Json(serde_json::json!({ "change_id": change_id, "selected": new_selected })),
    )
        .into_response()
}

/// POST /api/v1/projects/:id/changes/toggle-all
///
/// Toggles all changes for a project. If any change is unselected, selects all;
/// otherwise deselects all. Returns the new selected state.
pub async fn toggle_all_change_selection(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    let mut registry = state.registry.write().await;
    let entry = match registry.get(&project_id) {
        Some(e) => e.clone(),
        None => return error_response(StatusCode::NOT_FOUND, "Project not found"),
    };

    // List current change IDs from disk
    let data_dir = registry.data_dir().to_path_buf();
    let worktree_path = data_dir
        .join("worktrees")
        .join(&entry.id)
        .join(&entry.branch);
    let changes = list_remote_changes_in_worktree(&worktree_path, &entry.id, &entry.branch).await;
    let change_ids: Vec<String> = changes.iter().map(|c| c.id.clone()).collect();

    let new_selected = registry.toggle_all_changes(&project_id, &change_ids);

    if let Some(db) = &state.db {
        for change_id in &change_ids {
            let selected = registry.is_change_selected(&project_id, change_id);
            let error_message = registry
                .error_changes_for_project(&project_id)
                .and_then(|m| m.get(change_id))
                .map(std::string::String::as_str);
            if let Err(e) = db.upsert_change_state(&project_id, change_id, selected, error_message)
            {
                error!(project_id = %project_id, change_id = %change_id, error = %e, "Failed to persist toggle-all change state");
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to persist change state: {}", e),
                );
            }
        }
    }

    info!(

        project_id = %project_id,
        selected = new_selected,
        count = change_ids.len(),
        "All change selections toggled"
    );
    (
        StatusCode::OK,
        Json(serde_json::json!({ "selected": new_selected, "count": change_ids.len() })),
    )
        .into_response()
}

// ─────────────────────────────── Control ──────────────────────────────────────

/// Stub runner call recorder for unit testing.
#[allow(clippy::type_complexity)]
pub static CONTROL_CALLS: std::sync::OnceLock<Arc<std::sync::Mutex<Vec<(String, String)>>>> =
    std::sync::OnceLock::new();

/// POST /api/v1/control/run - Start orchestration across all projects.
///
/// For each project, collects changes that are currently selected and spawns a runner
/// with those change IDs. Error changes are excluded until they are explicitly re-marked.
/// Projects with no changes are skipped.
pub async fn global_control_run(State(state): State<AppState>) -> Response {
    // Record the call for test verification
    if let Some(calls) = CONTROL_CALLS.get() {
        calls
            .lock()
            .unwrap()
            .push(("_global_".to_string(), "run".to_string()));
    }

    let current_status = { state.orchestration_status.read().await.clone() };
    if current_status == OrchestrationStatus::Running {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "action": "run",
                "orchestration_status": "running",
                "message": "Orchestration is already running"
            })),
        )
            .into_response();
    }

    // Set status to Running
    {
        let mut status = state.orchestration_status.write().await;
        *status = OrchestrationStatus::Running;
    }

    let (entries, data_dir, all_selections) = {
        let registry = state.registry.read().await;
        let entries = registry.list();
        let data_dir = registry.data_dir().to_path_buf();
        let all_selections: std::collections::HashMap<
            String,
            std::collections::HashMap<String, bool>,
        > = entries
            .iter()
            .filter_map(|entry| {
                registry
                    .change_selections_for_project(&entry.id)
                    .map(|s| (entry.id.clone(), s.clone()))
            })
            .collect();
        (entries, data_dir, all_selections)
    };

    let mut started_count = 0u32;
    let mut skipped_count = 0u32;

    for entry in &entries {
        let worktree_path = data_dir
            .join("worktrees")
            .join(&entry.id)
            .join(&entry.branch);

        // Collect only the change IDs that are selected for the next run.
        let project_selections = all_selections.get(&entry.id);
        let changes =
            list_selected_change_ids_in_worktree(&worktree_path, project_selections).await;
        if changes.is_empty() {
            skipped_count += 1;
            continue;
        }

        // In unit tests we use CONTROL_CALLS as a stub signal; do not spawn real processes.
        if CONTROL_CALLS.get().is_none() {
            let req = ProjectRunRequest {
                project_id: entry.id.clone(),
                worktree_path,
                changes: Some(changes),
            };

            if let Err(e) = crate::server::runner::start_project_run(
                &state.runners,
                state.registry.clone(),
                state.db.clone(),
                req,
                state.log_tx.clone(),
            )
            .await
            {
                error!("Failed to start run for project_id={}: {}", entry.id, e);
                continue;
            }
        } else {
            // Record per-project call for tests
            if let Some(calls) = CONTROL_CALLS.get() {
                calls
                    .lock()
                    .unwrap()
                    .push((entry.id.clone(), "run".to_string()));
            }
        }

        // Update project status to Running
        let mut registry = state.registry.write().await;
        let _ = registry.set_status(&entry.id, ProjectStatus::Running);

        started_count += 1;
    }

    info!(
        "Global run: started {} projects, skipped {} (no changes)",
        started_count, skipped_count
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "action": "run",
            "orchestration_status": "running",
            "started": started_count,
            "skipped": skipped_count
        })),
    )
        .into_response()
}

/// POST /api/v1/control/stop - Stop orchestration across all projects.
///
/// Gracefully stops all running project runners and sets orchestration status to Stopped.
pub async fn global_control_stop(State(state): State<AppState>) -> Response {
    // Record the call for test verification
    if let Some(calls) = CONTROL_CALLS.get() {
        calls
            .lock()
            .unwrap()
            .push(("_global_".to_string(), "stop".to_string()));
    }

    // Set status to Stopped
    {
        let mut status = state.orchestration_status.write().await;
        *status = OrchestrationStatus::Stopped;
    }

    let entries = {
        let registry = state.registry.read().await;
        registry.list()
    };

    let mut stopped_count = 0u32;

    for entry in &entries {
        if entry.status == ProjectStatus::Running {
            crate::server::runner::stop_project_run(&state.runners, entry.id.clone()).await;

            let mut registry = state.registry.write().await;
            let _ = registry.set_status(&entry.id, ProjectStatus::Stopped);

            stopped_count += 1;
        }
    }

    info!("Global stop: stopped {} projects", stopped_count);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "action": "stop",
            "orchestration_status": "stopped",
            "stopped": stopped_count
        })),
    )
        .into_response()
}

/// GET /api/v1/control/status - Get current orchestration status.
pub async fn global_control_status(State(state): State<AppState>) -> Response {
    let status = state.orchestration_status.read().await;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "orchestration_status": status.as_str()
        })),
    )
        .into_response()
}

/// GET /api/v1/stats/overview - 全プロジェクトの成功/失敗数と平均処理時間を返す
pub(super) async fn get_stats_overview(State(state): State<AppState>) -> Response {
    let Some(db) = &state.db else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server database is not enabled",
        );
    };

    match db.get_stats_overview() {
        Ok(stats) => (
            StatusCode::OK,
            Json(StatsOverviewResponse {
                summary: StatsOverviewSummaryResponse {
                    success_count: stats.summary.success_count,
                    failure_count: stats.summary.failure_count,
                    in_progress_count: stats.summary.in_progress_count,
                    average_duration_ms: stats.summary.average_duration_ms,
                    average_duration_by_operation: stats.summary.average_duration_by_operation,
                },
                recent_events: stats
                    .recent_events
                    .into_iter()
                    .map(|e| StatsRecentEventResponse {
                        project_id: e.project_id,
                        change_id: e.change_id,
                        operation: e.operation,
                        result: e.result,
                        timestamp: e.timestamp,
                    })
                    .collect(),
                project_stats: stats
                    .project_stats
                    .into_iter()
                    .map(|p| StatsProjectResponse {
                        project_id: p.project_id,
                        apply_success_rate: p.apply_success_rate,
                        average_duration_ms: p.average_duration_ms,
                        success_count: p.success_count,
                        failure_count: p.failure_count,
                        in_progress_count: p.in_progress_count,
                    })
                    .collect(),
            }),
        )
            .into_response(),
        Err(e) => {
            error!(error = %e, "Failed to query stats overview");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to query stats overview: {}", e),
            )
        }
    }
}

/// GET /api/v1/stats/projects/:id/history - プロジェクト履歴イベントを返す
pub(super) async fn get_project_history(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let Some(db) = &state.db else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server database is not enabled",
        );
    };

    if state.registry.read().await.get(&project_id).is_none() {
        return error_response(StatusCode::NOT_FOUND, "Project not found");
    }

    let limit = query.limit.clamp(1, 1000);
    match db.get_recent_events(&project_id, limit) {
        Ok(events) => (StatusCode::OK, Json(events)).into_response(),
        Err(e) => {
            error!(project_id = %project_id, error = %e, "Failed to query project history");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to query project history: {}", e),
            )
        }
    }
}

/// GET /api/v1/logs - 永続化ログの検索
pub(super) async fn get_logs(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> Response {
    let Some(db) = &state.db else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server database is not enabled",
        );
    };

    let limit = query.limit.clamp(1, 1000);
    match db.query_logs(limit, query.before.as_deref(), query.project_id.as_deref()) {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => {
            error!(error = %e, "Failed to query persisted logs");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to query logs: {}", e),
            )
        }
    }
}

/// List change IDs in a project worktree that should be included in the next run.
/// Error changes are excluded unless they have been explicitly re-marked.
pub(super) async fn list_selected_change_ids_in_worktree(
    worktree_path: &std::path::Path,
    change_selections: Option<&std::collections::HashMap<String, bool>>,
) -> Vec<String> {
    let changes = list_remote_changes_in_worktree(worktree_path, "", "").await;
    changes
        .into_iter()
        .filter(|change| {
            let explicit_selection = change_selections.and_then(|m| m.get(&change.id)).copied();
            let selected = explicit_selection.unwrap_or(true);
            if change.status == "error" {
                explicit_selection.unwrap_or(false)
            } else {
                selected
            }
        })
        .map(|change| change.id)
        .collect()
}

// ─────────────────────────── Deprecated per-project control (removed) ─────────

// Per-project control endpoints (/projects/{id}/control/run|stop|retry) have been
// removed. Use the global /api/v1/control/run and /api/v1/control/stop endpoints
// instead. The global endpoints manage all projects as a single orchestration unit.

// ─────────────────────────── Internal: per-project run (used by global control) ─

/// Start a single project run (used internally by global_control_run and add_project auto-enqueue).
pub(super) async fn start_single_project_run(
    state: &AppState,
    project_id: &str,
    worktree_path: std::path::PathBuf,
    changes: Vec<String>,
) -> std::result::Result<(), String> {
    let req = ProjectRunRequest {
        project_id: project_id.to_string(),
        worktree_path,
        changes: if changes.is_empty() {
            None
        } else {
            Some(changes)
        },
    };

    crate::server::runner::start_project_run(
        &state.runners,
        state.registry.clone(),
        state.db.clone(),
        req,
        state.log_tx.clone(),
    )
    .await
    .map_err(|e| format!("Failed to start run: {}", e))?;

    let mut registry = state.registry.write().await;
    let _ = registry.set_status(project_id, ProjectStatus::Running);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::server::api::test_support::{create_local_git_repo, make_state as make_base_state};
    use crate::server::api::{build_router, AppState};

    fn make_state(temp_dir: &TempDir, auth_token: Option<&str>) -> AppState {
        let mut state = make_base_state(temp_dir, auth_token);
        state.db = Some(crate::server::db::ServerDb::new(temp_dir.path()).unwrap());
        state
    }

    #[tokio::test]
    async fn test_stats_and_logs_endpoints_require_auth() {
        let temp_dir = TempDir::new().unwrap();
        let router = build_router(make_state(&temp_dir, Some("secret-token")));

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/stats/overview")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/logs")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_toggle_all_change_selection_remarks_error_changes_for_next_run() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let change_dir = temp_dir
            .path()
            .join("worktrees")
            .join(&entry.id)
            .join(&entry.branch)
            .join("openspec/changes/fix-a");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# proposal\n").unwrap();

        {
            let mut registry = state.registry.write().await;
            registry.mark_change_error(&entry.id, "fix-a", "boom".to_string());
        }

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/changes/toggle-all", entry.id))
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["selected"], true);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/state")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["projects"][0]["changes"][0]["selected"], true);
        assert_eq!(json["projects"][0]["changes"][0]["status"], "error");

        CONTROL_CALLS.get_or_init(|| Arc::new(std::sync::Mutex::new(Vec::new())));
        CONTROL_CALLS.get().unwrap().lock().unwrap().clear();

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/control/run")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["started"], 1);
        assert_eq!(json["skipped"], 0);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/state")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["projects"][0]["changes"][0]["selected"], true);
        assert_eq!(json["projects"][0]["changes"][0]["status"], "error");
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
}
