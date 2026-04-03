use super::*;

// ─────────────────────────── /api/v1/projects/:id/worktrees ───────────────────

/// Request body for creating a worktree in server mode.
#[derive(Debug, Deserialize)]
pub struct ServerCreateWorktreeRequest {
    /// Change ID to create the worktree for
    pub change_id: String,
    /// Optional base commit (defaults to HEAD of project branch)
    #[serde(default)]
    pub base_commit: Option<String>,
}

/// Response for worktree operations that return a success message.
#[derive(Debug, Serialize)]
pub(super) struct WorktreeOpResponse {
    success: bool,
    message: String,
}

/// Helper: resolve the project's main worktree path from registry.
pub(super) async fn resolve_project_worktree_path(
    state: &AppState,
    project_id: &str,
) -> Result<(std::path::PathBuf, crate::server::registry::ProjectEntry), Response> {
    let registry = state.registry.read().await;
    let entry = registry.get(project_id).cloned().ok_or_else(|| {
        error_response(
            StatusCode::NOT_FOUND,
            format!("Project not found: {}", project_id),
        )
    })?;
    let data_dir = registry.data_dir().to_path_buf();
    let worktree_path = data_dir
        .join("worktrees")
        .join(project_id)
        .join(&entry.branch);
    Ok((worktree_path, entry))
}

/// GET /api/v1/projects/:id/worktrees - list all worktrees for a project
pub async fn server_list_worktrees(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    let (worktree_path, _entry) = match resolve_project_worktree_path(&state, &project_id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if !worktree_path.exists() {
        return (
            StatusCode::OK,
            Json(Vec::<crate::remote::types::RemoteWorktreeInfo>::new()),
        )
            .into_response();
    }

    match crate::worktree_ops::get_worktrees(&worktree_path).await {
        Ok(worktrees) => {
            let remote_worktrees: Vec<crate::remote::types::RemoteWorktreeInfo> =
                worktrees.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(remote_worktrees)).into_response()
        }
        Err(e) => {
            error!(
                project_id = %project_id,
                error = %e,
                "Failed to list worktrees"
            );
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list worktrees: {}", e),
            )
        }
    }
}

/// POST /api/v1/projects/:id/worktrees - create a new worktree
pub async fn server_create_worktree(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(req): Json<ServerCreateWorktreeRequest>,
) -> Response {
    let (worktree_path, _entry) = match resolve_project_worktree_path(&state, &project_id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if !worktree_path.exists() {
        return error_response(
            StatusCode::NOT_FOUND,
            format!(
                "Project worktree not found at {:?}. Add the project first.",
                worktree_path
            ),
        );
    }

    // Check if worktree already exists for this change_id
    match crate::worktree_ops::worktree_exists(&worktree_path, &req.change_id).await {
        Ok(true) => {
            return error_response(
                StatusCode::CONFLICT,
                format!("Worktree for '{}' already exists", req.change_id),
            );
        }
        Ok(false) => {}
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to check worktree existence: {}", e),
            );
        }
    }

    // Determine workspace base directory for new worktrees under this project
    let workspace_base = {
        let registry = state.registry.read().await;
        registry.data_dir().join("worktrees").join(&project_id)
    };

    // Sanitize change_id for branch name
    let branch_name = req.change_id.replace(['/', '\\', ' '], "-");
    let new_worktree_path = workspace_base.join(&branch_name);

    // Ensure base directory exists
    if let Err(e) = std::fs::create_dir_all(&workspace_base) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create workspace directory: {}", e),
        );
    }

    // Get base commit
    let base_commit = match req.base_commit {
        Some(commit) => commit,
        None => match crate::vcs::git::commands::get_current_commit(&worktree_path).await {
            Ok(commit) => commit,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to get current commit: {}", e),
                );
            }
        },
    };

    // Create worktree
    if let Err(e) = crate::vcs::git::commands::worktree_add(
        &worktree_path,
        new_worktree_path.to_str().unwrap_or(""),
        &branch_name,
        &base_commit,
    )
    .await
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create worktree: {}", e),
        );
    }

    // Execute setup script if it exists
    let _ = crate::vcs::git::commands::run_worktree_setup(&worktree_path, &new_worktree_path).await;

    info!(
        project_id = %project_id,
        change_id = %req.change_id,
        "Worktree created successfully"
    );

    let worktree_info = crate::remote::types::RemoteWorktreeInfo {
        path: new_worktree_path.to_string_lossy().to_string(),
        label: branch_name.clone(),
        head: base_commit[..8.min(base_commit.len())].to_string(),
        branch: branch_name,
        is_detached: false,
        is_main: false,
        is_merging: false,
        has_commits_ahead: false,
        merge_conflict: None,
    };

    (StatusCode::CREATED, Json(worktree_info)).into_response()
}

/// DELETE /api/v1/projects/:id/worktrees/:branch - delete a worktree
pub async fn server_delete_worktree(
    State(state): State<AppState>,
    Path((project_id, branch)): Path<(String, String)>,
) -> Response {
    // Acquire active command slot for this worktree root
    let _active_guard = match try_acquire_active_command(
        &state.active_commands,
        &project_id,
        RootKind::Worktree(branch.clone()),
        "delete",
    )
    .await
    {
        Ok(guard) => guard,
        Err(resp) => return resp,
    };

    let (worktree_path, _entry) = match resolve_project_worktree_path(&state, &project_id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if !worktree_path.exists() {
        return error_response(StatusCode::NOT_FOUND, "Project worktree not found");
    }

    // Get worktree list to find the target
    let worktrees = match crate::worktree_ops::get_worktrees(&worktree_path).await {
        Ok(wts) => wts,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list worktrees: {}", e),
            );
        }
    };

    let worktree = match worktrees.iter().find(|wt| wt.branch == branch) {
        Some(wt) => wt,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                format!("Worktree '{}' not found", branch),
            );
        }
    };

    // Validate deletion
    let (can_delete, reason) = crate::worktree_ops::can_delete_worktree(worktree).await;
    if !can_delete {
        let error_msg = reason.unwrap_or_else(|| "Cannot delete worktree".to_string());
        return error_response(StatusCode::CONFLICT, error_msg);
    }

    // Delete worktree
    if let Err(e) =
        crate::vcs::git::commands::worktree_remove(&worktree_path, worktree.path.to_str().unwrap())
            .await
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to remove worktree: {}", e),
        );
    }

    // Delete branch
    let _ = crate::vcs::git::commands::branch_delete(&worktree_path, &branch).await;

    info!(
        project_id = %project_id,
        branch = %branch,
        "Worktree deleted successfully"
    );

    (
        StatusCode::OK,
        Json(WorktreeOpResponse {
            success: true,
            message: format!("Worktree '{}' deleted successfully", branch),
        }),
    )
        .into_response()
}

/// POST /api/v1/projects/:id/worktrees/:branch/merge - merge a worktree branch
pub async fn server_merge_worktree(
    State(state): State<AppState>,
    Path((project_id, branch)): Path<(String, String)>,
) -> Response {
    // Merge operates on the base worktree (merging branch into base), so guard the base root.
    // Also guard the worktree root to prevent concurrent operations on the branch being merged.
    let _active_guard_base = match try_acquire_active_command(
        &state.active_commands,
        &project_id,
        RootKind::Base,
        "merge",
    )
    .await
    {
        Ok(guard) => guard,
        Err(resp) => return resp,
    };
    let _active_guard_wt = match try_acquire_active_command(
        &state.active_commands,
        &project_id,
        RootKind::Worktree(branch.clone()),
        "merge",
    )
    .await
    {
        Ok(guard) => guard,
        Err(resp) => return resp,
    };

    let (worktree_path, _entry) = match resolve_project_worktree_path(&state, &project_id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if !worktree_path.exists() {
        return error_response(StatusCode::NOT_FOUND, "Project worktree not found");
    }

    // Get worktree list to find the target
    let worktrees = match crate::worktree_ops::get_worktrees(&worktree_path).await {
        Ok(wts) => wts,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list worktrees: {}", e),
            );
        }
    };

    let worktree = match worktrees.iter().find(|wt| wt.branch == branch) {
        Some(wt) => wt,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                format!("Worktree '{}' not found", branch),
            );
        }
    };

    // Validate merge
    let (can_merge, reason) = crate::worktree_ops::can_merge_worktree(worktree);
    if !can_merge {
        let error_msg = reason.unwrap_or_else(|| "Cannot merge worktree".to_string());
        return error_response(StatusCode::CONFLICT, error_msg);
    }

    // Get base branch name
    let base_branch = match worktrees.iter().find(|wt| wt.is_main) {
        Some(wt) => wt.branch.clone(),
        None => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to determine base branch",
            );
        }
    };

    // Checkout base branch
    if let Err(e) = crate::vcs::git::commands::checkout(&worktree_path, &base_branch).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to checkout base branch: {}", e),
        );
    }

    // Merge branch
    if let Err(e) = crate::vcs::git::commands::merge(&worktree_path, &branch).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to merge branch: {}", e),
        );
    }

    info!(
        project_id = %project_id,
        branch = %branch,
        base_branch = %base_branch,
        "Worktree merged successfully"
    );

    (
        StatusCode::OK,
        Json(WorktreeOpResponse {
            success: true,
            message: format!(
                "Branch '{}' merged into '{}' successfully",
                branch, base_branch
            ),
        }),
    )
        .into_response()
}

/// POST /api/v1/projects/:id/worktrees/refresh - refresh worktree conflict detection
pub async fn server_refresh_worktrees(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    // Delegate to the list endpoint (always fresh from git)
    server_list_worktrees(State(state), Path(project_id)).await
}
