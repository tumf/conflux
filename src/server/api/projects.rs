use super::*;

use crate::server::api::control::{
    list_selected_change_ids_in_worktree, start_single_project_run, CONTROL_CALLS,
};
use crate::server::api::ws::build_remote_project_snapshot_async;

// ─────────────────────────────── /api/v1/version ──────────────────────────────

/// GET /api/v1/version - return backend version string
pub async fn get_version() -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "version": format!("v{} ({})", env!("CARGO_PKG_VERSION"), env!("BUILD_NUMBER"))
        })),
    )
        .into_response()
}

// ─────────────────────────────── /api/v1/projects ─────────────────────────────

/// GET /api/v1/projects - list all projects
pub async fn list_projects(State(state): State<AppState>) -> Response {
    let registry = state.registry.read().await;
    let projects: Vec<ProjectResponse> = registry.list().into_iter().map(Into::into).collect();
    (StatusCode::OK, Json(projects)).into_response()
}

/// GET /api/v1/projects/state - list projects with their OpenSpec changes
///
/// This endpoint returns a server-oriented state snapshot suitable for any
/// dashboard or client (including the TUI): projects (remote_url + branch) each
/// with the list of OpenSpec changes discovered in that project's worktree.
pub async fn projects_state(State(state): State<AppState>) -> Response {
    let (entries, data_dir, all_selections, all_errors) = {
        let registry = state.registry.read().await;
        let entries = registry.list();
        let data_dir = registry.data_dir().to_path_buf();
        let all_selections: std::collections::HashMap<
            String,
            std::collections::HashMap<String, bool>,
        > = entries
            .iter()
            .filter_map(|e| {
                registry
                    .change_selections_for_project(&e.id)
                    .map(|s| (e.id.clone(), s.clone()))
            })
            .collect();
        let all_errors: std::collections::HashMap<
            String,
            std::collections::HashMap<String, String>,
        > = entries
            .iter()
            .filter_map(|e| {
                registry
                    .error_changes_for_project(&e.id)
                    .map(|s| (e.id.clone(), s.clone()))
            })
            .collect();
        (entries, data_dir, all_selections, all_errors)
    };

    let mut projects = Vec::new();
    for entry in &entries {
        let selections = all_selections.get(&entry.id);
        let errors = all_errors.get(&entry.id);
        projects
            .push(build_remote_project_snapshot_async(&data_dir, entry, selections, errors).await);
    }

    let sync_available = state.resolve_command.is_some();

    (
        StatusCode::OK,
        Json(ProjectsStateResponse {
            projects,
            sync_available,
        }),
    )
        .into_response()
}

pub async fn add_project(
    State(state): State<AppState>,
    Json(req): Json<AddProjectRequest>,
) -> Response {
    if req.remote_url.trim().is_empty() || req.branch.trim().is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "remote_url and branch are required",
        );
    }

    let remote_url = req.remote_url.clone();
    let branch = req.branch.clone();

    // Step 1: Register the project in the registry first so we can obtain the lock.
    let entry = {
        let mut registry = state.registry.write().await;
        match registry.add(remote_url.clone(), branch.clone()) {
            Ok(e) => e,
            Err(e) => {
                let msg = e.to_string();
                return if msg.contains("already exists") {
                    error_response(StatusCode::CONFLICT, msg)
                } else {
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, msg)
                };
            }
        }
    };

    let project_id = entry.id.clone();

    // Helper: roll back registry insertion on failure.
    let rollback = |state: &AppState, project_id: String| {
        let registry = state.registry.clone();
        async move {
            let mut reg = registry.write().await;
            if let Err(e) = reg.remove(&project_id) {
                error!("Rollback failed for project_id={}: {}", project_id, e);
            } else {
                info!("Rolled back registry entry for project_id={}", project_id);
            }
        }
    };

    // Step 2: Acquire global semaphore and per-project lock.
    let (lock, semaphore) = {
        let registry = state.registry.read().await;
        let lock = match registry.project_lock(&project_id) {
            Some(l) => l,
            None => {
                rollback(&state, project_id).await;
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Missing project lock");
            }
        };
        let semaphore = registry.global_semaphore();
        (lock, semaphore)
    };

    let _sem_permit = match semaphore.acquire().await {
        Ok(p) => p,
        Err(_) => {
            rollback(&state, project_id).await;
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Server is at maximum concurrent capacity",
            );
        }
    };

    let _guard = lock.lock().await;

    info!(
        "add_project: project_id={} remote_url={} branch={}",
        project_id, remote_url, branch
    );

    // Determine local paths.
    // The worktree uses a server-specific branch name to avoid checking out the base branch.
    // This is required so that git pull/push on the bare clone can update refs/heads/<branch>
    // without being blocked by an active checkout of that branch.
    let server_branch = server_worktree_branch(&project_id, &branch);
    let (local_repo_path, worktree_path) = {
        let registry = state.registry.read().await;
        let data_dir = registry.data_dir().to_path_buf();
        let repo = data_dir.join(&project_id);
        let wt = data_dir.join("worktrees").join(&project_id).join(&branch);
        (repo, wt)
    };

    // Step 3: Verify the branch exists on the remote.
    let ls_remote = tokio::process::Command::new("git")
        .args(["ls-remote", "--heads", &remote_url, &branch])
        .output()
        .await;

    match ls_remote {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                rollback(&state, project_id).await;
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("Branch '{}' not found on remote '{}'", branch, remote_url),
                );
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            error!("git ls-remote failed: {}", stderr);
            rollback(&state, project_id).await;
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("git operation failed: {}", stderr),
            );
        }
        Err(e) => {
            error!("Failed to run git: {}", e);
            rollback(&state, project_id).await;
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git: {}", e),
            );
        }
    }

    // Step 4: Clone as a bare repository if not already present.
    if !local_repo_path.exists() {
        let clone_output = tokio::process::Command::new("git")
            .args([
                "clone",
                "--bare",
                "--branch",
                &branch,
                "--single-branch",
                &remote_url,
                local_repo_path.to_str().unwrap_or(""),
            ])
            .output()
            .await;

        match clone_output {
            Ok(out) if out.status.success() => {
                info!("git clone (bare) succeeded: project_id={}", project_id);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                error!(
                    "git clone failed: project_id={} stderr={}",
                    project_id, stderr
                );
                rollback(&state, project_id).await;
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("git clone failed: {}", stderr),
                );
            }
            Err(e) => {
                rollback(&state, project_id).await;
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to run git clone: {}", e),
                );
            }
        }
    } else {
        info!(
            "Bare clone already exists, reusing: project_id={}",
            project_id
        );
    }

    // Step 5: Create a worktree at data_dir/worktrees/<project_id>/<branch>.
    // The worktree is created on a server-specific branch (`server-wt/<project_id>/<base_branch>`)
    // so that the bare clone's refs/heads/<base_branch> can be updated by git pull/push
    // without being blocked by an active checkout.
    if !worktree_path.exists() {
        if let Err(e) = std::fs::create_dir_all(&worktree_path) {
            error!("Failed to create worktree parent dirs: {}", e);
            rollback(&state, project_id).await;
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create worktree directory: {}", e),
            );
        }
        // Remove the pre-created dir so git worktree add can create it.
        if let Err(e) = std::fs::remove_dir(&worktree_path) {
            error!("Failed to remove pre-created worktree dir: {}", e);
            rollback(&state, project_id).await;
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to prepare worktree directory: {}", e),
            );
        }

        // Use `-b <server_branch>` to create a new branch for the worktree.
        // This avoids checking out the base branch directly in the worktree,
        // which would prevent the bare clone from updating refs/heads/<base_branch>.
        let worktree_output = tokio::process::Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &server_branch,
                worktree_path.to_str().unwrap_or(""),
                &branch,
            ])
            .current_dir(&local_repo_path)
            .output()
            .await;

        match worktree_output {
            Ok(out) if out.status.success() => {
                info!(
                    "git worktree add succeeded: project_id={} path={:?} server_branch={}",
                    project_id, worktree_path, server_branch
                );
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                error!(
                    "git worktree add failed: project_id={} stderr={}",
                    project_id, stderr
                );
                // Clean up bare clone on worktree failure.
                let _ = std::fs::remove_dir_all(&local_repo_path);
                rollback(&state, project_id).await;
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("git worktree add failed: {}", stderr),
                );
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&local_repo_path);
                rollback(&state, project_id).await;
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to run git worktree add: {}", e),
                );
            }
        }
    } else {
        // Worktree already exists. Check if it is checking out the base branch directly,
        // which would block git pull/push on the bare clone.
        // If so, return an error with instructions to recreate the worktree.
        let head_output = tokio::process::Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&local_repo_path)
            .output()
            .await;

        if let Ok(out) = head_output {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            // Parse worktree list output to find the worktree at our path and its branch.
            // Format:
            //   worktree /path/to/wt
            //   HEAD <sha>
            //   branch refs/heads/<branch>
            let wt_path_str = worktree_path.to_str().unwrap_or("");
            let mut in_our_worktree = false;
            let mut checked_out_branch: Option<String> = None;
            for line in stdout.lines() {
                if line.starts_with("worktree ") {
                    let wt_path = line.trim_start_matches("worktree ");
                    in_our_worktree = wt_path == wt_path_str;
                    if !in_our_worktree {
                        checked_out_branch = None;
                    }
                } else if in_our_worktree && line.starts_with("branch refs/heads/") {
                    checked_out_branch =
                        Some(line.trim_start_matches("branch refs/heads/").to_string());
                }
            }

            if checked_out_branch.as_deref() == Some(branch.as_str()) {
                // The existing worktree is checking out the base branch directly.
                // This blocks bare clone pull/push. The user must recreate it.
                error!(
                    "Existing worktree checks out base branch '{}' directly: project_id={} path={:?}. \
                    This blocks git pull/push on the bare clone.",
                    branch, project_id, worktree_path
                );
                let err_msg = format!(
                    "Existing worktree at {:?} checks out the base branch '{}' directly. \
                    This prevents git pull/push on the bare clone. \
                    To fix: remove the project (DELETE /api/v1/projects/{}), then re-add it \
                    so the worktree is recreated on a server-specific branch ({}). \
                    Alternatively, run: git worktree remove {:?} && re-add via API.",
                    worktree_path, branch, project_id, server_branch, worktree_path
                );
                rollback(&state, project_id).await;
                return error_response(StatusCode::CONFLICT, err_msg);
            }
        }

        info!(
            "Worktree already exists, reusing: project_id={} path={:?}",
            project_id, worktree_path
        );
    }

    // Step 6: Execute repo-root .wt/setup in the new worktree (if present).
    // Failure is treated as add_project failure and triggers rollback.
    if let Err(e) =
        crate::vcs::git::commands::run_worktree_setup(&worktree_path, &worktree_path).await
    {
        error!(
            "worktree setup failed: project_id={} worktree={:?} error={}",
            project_id, worktree_path, e
        );

        // Best-effort cleanup for partially provisioned project files.
        if let Err(cleanup_err) = std::fs::remove_dir_all(&worktree_path) {
            error!(
                "Failed to cleanup worktree after setup failure: project_id={} path={:?} error={}",
                project_id, worktree_path, cleanup_err
            );
        }
        if let Err(cleanup_err) = std::fs::remove_dir_all(&local_repo_path) {
            error!(
                "Failed to cleanup bare clone after setup failure: project_id={} path={:?} error={}",
                project_id, local_repo_path, cleanup_err
            );
        }

        rollback(&state, project_id).await;
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("worktree setup failed: {}", e),
        );
    }

    info!("Project added with clone and worktree: id={}", project_id);

    // Task 8: Auto-enqueue if orchestration is currently running.
    // When a new project is added during Running state, automatically spawn a runner for it.
    {
        let orch_status = state.orchestration_status.read().await;
        if *orch_status == OrchestrationStatus::Running {
            let changes = list_selected_change_ids_in_worktree(&worktree_path, None).await;
            if !changes.is_empty() {
                info!(
                    "Auto-enqueuing new project during Running state: project_id={} changes={:?}",
                    project_id, changes
                );
                if CONTROL_CALLS.get().is_none() {
                    if let Err(e) =
                        start_single_project_run(&state, &project_id, worktree_path, changes).await
                    {
                        error!(
                            "Failed to auto-enqueue new project: project_id={} err={}",
                            project_id, e
                        );
                    }
                }
            }
        }
    }

    (StatusCode::CREATED, Json(ProjectResponse::from(entry))).into_response()
}

/// DELETE /api/v1/projects/:id - remove a project
pub async fn delete_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    if let Some(db) = &state.db {
        if let Err(e) = db.delete_change_states_for_project(&project_id) {
            warn!(project_id = %project_id, error = %e, "Failed to clear persisted change states before deleting project");
        }
    }

    let mut registry = state.registry.write().await;
    match registry.remove(&project_id) {
        Ok(_) => {
            info!("Project deleted: id={}", project_id);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                error_response(StatusCode::NOT_FOUND, msg)
            } else {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::server::api::test_support::{
        create_local_git_repo, make_state, run_sync_monitor_once_for_tests,
    };

    #[cfg(feature = "heavy-tests")]
    use crate::server::api::test_support::{create_local_git_repo_with_setup, make_router};

    #[cfg(feature = "heavy-tests")]
    #[tokio::test]
    async fn test_add_project_without_repo_root_setup_succeeds_without_marker() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

        let body = serde_json::json!({
            "remote_url": remote_url,
            "branch": "main"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects")
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let project_id = json["id"].as_str().expect("Response must contain id");

        let marker_path = temp_dir
            .path()
            .join("worktrees")
            .join(project_id)
            .join("main")
            .join(".setup-root-path");
        assert!(
            !marker_path.exists(),
            "No setup marker should exist when repo-root .wt/setup is absent"
        );
    }

    #[cfg(feature = "heavy-tests")]
    #[tokio::test]
    async fn test_add_project_setup_failure_returns_422_and_rolls_back_registry() {
        let temp_dir = TempDir::new().unwrap();
        let origin =
            create_local_git_repo_with_setup(temp_dir.path(), Some("#!/bin/sh\nexit 42\n"));
        let remote_url = format!("file://{}", origin.to_str().unwrap());
        let expected_project_id = crate::server::registry::generate_project_id(&remote_url, "main");

        let state = make_state(&temp_dir, None);
        let router = build_router(state.clone());

        let body = serde_json::json!({
            "remote_url": remote_url,
            "branch": "main"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects")
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let error_message = json["error"].as_str().unwrap_or_default();
        assert!(
            error_message.contains("worktree setup failed"),
            "error should mention setup failure, got: {}",
            json
        );

        let registry = state.registry.read().await;
        assert!(
            registry.list().is_empty(),
            "Registry should be empty after setup failure rollback"
        );

        let local_repo_path = temp_dir.path().join(&expected_project_id);
        let worktree_path = temp_dir
            .path()
            .join("worktrees")
            .join(&expected_project_id)
            .join("main");
        assert!(
            !local_repo_path.exists(),
            "Bare clone should be cleaned up after setup failure"
        );
        assert!(
            !worktree_path.exists(),
            "Worktree should be cleaned up after setup failure"
        );
    }

    #[tokio::test]
    async fn test_projects_state_includes_sync_metadata_fields_after_monitor_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_string_lossy());

        let state = make_state(&temp_dir, None);
        let router = build_router(state.clone());

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

        run_sync_monitor_once_for_tests(&state).await;

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

        let project = &json["projects"][0];
        assert_eq!(project["sync_state"], "up_to_date");
        assert_eq!(project["ahead_count"], 0);
        assert_eq!(project["behind_count"], 0);
        assert_eq!(project["sync_required"], false);
        assert!(project["local_sha"].as_str().is_some());
        assert!(project["remote_sha"].as_str().is_some());
        assert!(project["last_remote_check_at"].as_str().is_some());
        assert!(project["remote_check_error"].is_null());
    }

    #[cfg(feature = "heavy-tests")]
    #[test]
    fn test_app_state_resolve_command_comes_from_top_level_config() {
        let top_level_resolve_cmd = Some("echo top-level-resolve".to_string());
        let app_state_resolve_command = top_level_resolve_cmd.clone();

        assert_eq!(
            app_state_resolve_command,
            Some("echo top-level-resolve".to_string()),
            "AppState resolve_command should come from top-level config resolve_command"
        );
    }
}
