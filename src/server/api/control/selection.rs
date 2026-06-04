use super::*;

use crate::remote::types::RemoteStateUpdate;
use crate::server::api::ws::list_remote_changes_in_worktree;

async fn build_remote_change_for_update(
    state: &AppState,
    project_id: &str,
    change_id: &str,
) -> Option<RemoteChange> {
    let (entry, data_dir, selection_state, error_state) = {
        let registry = state.registry.read().await;
        let entry = registry.get(project_id)?.clone();
        let data_dir = registry.data_dir().to_path_buf();
        let selection_state = registry
            .change_selections_for_project(project_id)
            .and_then(|m| m.get(change_id))
            .copied();
        let error_state = registry
            .error_changes_for_project(project_id)
            .and_then(|m| m.get(change_id))
            .cloned();
        (entry, data_dir, selection_state, error_state)
    };

    let worktree_path = data_dir
        .join("worktrees")
        .join(&entry.id)
        .join(&entry.branch);

    let changes = list_remote_changes_in_worktree(
        &worktree_path,
        &entry.id,
        &entry.branch,
        &state.shared_orchestrator_state,
    )
    .await;

    let mut change = changes.into_iter().find(|c| c.id == change_id)?;
    let is_rejected = change.status == "rejected";
    if is_rejected {
        change.selected = false;
        return Some(change);
    }

    if error_state.is_some() {
        change.status = "error".to_string();
        change.iteration_number = None;
    }

    let default_selected = error_state.is_none();
    change.selected = selection_state.unwrap_or(default_selected);
    Some(change)
}

async fn emit_change_update_if_available(state: &AppState, project_id: &str, change_id: &str) {
    let Some(change) = build_remote_change_for_update(state, project_id, change_id).await else {
        warn!(project_id = %project_id, change_id = %change_id, "Skip change_update broadcast because change snapshot is unavailable");
        return;
    };

    if let Err(err) = state
        .state_update_tx
        .send(RemoteStateUpdate::ChangeUpdate { change })
    {
        debug!(project_id = %project_id, change_id = %change_id, error = %err, "No WebSocket subscribers received change_update broadcast");
    }
}

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

    drop(registry);
    emit_change_update_if_available(&state, &project_id, &change_id).await;

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

    let data_dir = registry.data_dir().to_path_buf();
    let worktree_path = data_dir
        .join("worktrees")
        .join(&entry.id)
        .join(&entry.branch);
    let changes = list_remote_changes_in_worktree(
        &worktree_path,
        &entry.id,
        &entry.branch,
        &state.shared_orchestrator_state,
    )
    .await;
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

    drop(registry);

    for change_id in &change_ids {
        emit_change_update_if_available(&state, &project_id, change_id).await;
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
