use super::*;

/// Stub runner call recorder for unit testing.
#[allow(clippy::type_complexity)]
pub static CONTROL_CALLS: std::sync::OnceLock<Arc<std::sync::Mutex<Vec<(String, String)>>>> =
    std::sync::OnceLock::new();

#[cfg(test)]
static CONTROL_CALLS_TEST_LOCK: std::sync::OnceLock<Arc<tokio::sync::Mutex<()>>> =
    std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) async fn lock_control_calls_for_test() -> tokio::sync::OwnedMutexGuard<()> {
    let guard = CONTROL_CALLS_TEST_LOCK
        .get_or_init(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
        .lock_owned()
        .await;
    let calls = CONTROL_CALLS.get_or_init(|| Arc::new(std::sync::Mutex::new(Vec::new())));
    calls.lock().unwrap().clear();
    guard
}

/// POST /api/v1/control/run - Start orchestration across all projects.
///
/// For each project, collects changes that are currently selected and spawns a runner
/// with those change IDs. Error changes are excluded until they are explicitly re-marked.
/// Projects with no changes are skipped.
pub async fn global_control_run(State(state): State<AppState>) -> Response {
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

        let project_selections = all_selections.get(&entry.id);
        let changes = super::list_selected_change_ids_in_worktree(
            &worktree_path,
            project_selections,
            &state.shared_orchestrator_state,
        )
        .await;
        if changes.is_empty() {
            skipped_count += 1;
            continue;
        }

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
                error!(project_id = %entry.id, error = %e, "Failed to start project run during global run");
                continue;
            }
        } else if let Some(calls) = CONTROL_CALLS.get() {
            calls
                .lock()
                .unwrap()
                .push((entry.id.clone(), "run".to_string()));
        }

        let mut registry = state.registry.write().await;
        let _ = registry.set_status(&entry.id, ProjectStatus::Running);

        started_count += 1;
    }

    info!(
        started = started_count,
        skipped = skipped_count,
        "Global run completed"
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
    if let Some(calls) = CONTROL_CALLS.get() {
        calls
            .lock()
            .unwrap()
            .push(("_global_".to_string(), "stop".to_string()));
    }

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

    info!(stopped = stopped_count, "Global stop completed");

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

/// Start a single project run (used internally by global_control_run and add_project auto-enqueue).
pub(crate) async fn start_single_project_run(
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
