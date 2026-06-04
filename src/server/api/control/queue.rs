use super::*;

/// POST /api/v1/projects/:id/changes/:change_id/stop-and-dequeue
///
/// Force-stop and dequeue a single change. For active changes, this kills the
/// project runner to guarantee process termination before returning success.
/// For queued-but-not-running changes, it simply deselects them.
pub async fn stop_and_dequeue_change(
    State(state): State<AppState>,
    Path((project_id, change_id)): Path<(String, String)>,
) -> Response {
    let mut registry = state.registry.write().await;
    let entry = match registry.get(&project_id) {
        Some(e) => e.clone(),
        None => return error_response(StatusCode::NOT_FOUND, "Project not found"),
    };

    if registry.is_change_selected(&project_id, &change_id) {
        registry.toggle_change_selected(&project_id, &change_id);
    }

    if let Some(db) = &state.db {
        let error_message = registry
            .error_changes_for_project(&project_id)
            .and_then(|m| m.get(&change_id))
            .map(std::string::String::as_str);
        if let Err(e) = db.upsert_change_state(&project_id, &change_id, false, error_message) {
            error!(project_id = %project_id, change_id = %change_id, error = %e, "Failed to persist stop-and-dequeue state");
        }
    }
    drop(registry);

    if entry.status == ProjectStatus::Running {
        crate::server::runner::stop_project_run(&state.runners, project_id.clone()).await;
        let mut registry = state.registry.write().await;
        let _ = registry.set_status(&project_id, ProjectStatus::Idle);
        info!(
            project_id = %project_id,
            change_id = %change_id,
            "Force-killed running project for change stop-and-dequeue"
        );
    }

    info!(
        project_id = %project_id,
        change_id = %change_id,
        "Change force-stopped and dequeued"
    );
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "change_id": change_id,
            "selected": false,
            "status": "not queued"
        })),
    )
        .into_response()
}
