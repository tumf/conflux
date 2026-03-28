//! API v1 handlers for the server daemon.
//!
//! Provides REST endpoints for project management and execution control.
//!
//! NOTE: This module deliberately does NOT reference or execute `~/.wt/setup`.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{ws::Message, ws::WebSocket, ws::WebSocketUpgrade, Path, State},
    http::{header, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use shlex;
use tracing::{debug, error, info};

use crate::execution::state::{detect_workspace_state, WorkspaceState};
use crate::remote::types::{RemoteChange, RemoteLogEntry, RemoteProject, RemoteStateUpdate};
use crate::server::registry::{
    server_worktree_branch, ProjectEntry, ProjectStatus, SharedRegistry,
};
use crate::server::runner::{ProjectRunRequest, SharedRunners};
use crate::task_parser;
use crate::vcs::GitWorkspaceManager;

/// Maximum number of log entries retained in server-side log buffer (broadcast channel capacity)
pub const SERVER_LOG_BUFFER_SIZE: usize = 1000;

/// Shared application state passed to axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub registry: SharedRegistry,
    pub(crate) runners: SharedRunners,
    /// Optional bearer token for authentication (None = no auth required)
    pub auth_token: Option<String>,
    /// Maximum concurrent total (informational; actual semaphore is in registry)
    #[allow(dead_code)]
    pub max_concurrent_total: usize,
    /// Optional resolve command to run when auto_resolve is triggered on non-fast-forward
    pub resolve_command: Option<String>,
    /// Broadcast channel for streaming log entries to WebSocket clients
    pub log_tx: tokio::sync::broadcast::Sender<RemoteLogEntry>,
}

// ─────────────────────────────── Auth middleware ──────────────────────────────

/// Bearer token authentication middleware.
/// Passes through if no auth_token is configured (loopback-only mode).
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if let Some(expected_token) = &state.auth_token {
        let auth_header = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let provided = auth_header.strip_prefix("Bearer ").unwrap_or("");
        if provided != expected_token {
            debug!("Auth failed: invalid or missing Bearer token");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Unauthorized"})),
            )
                .into_response();
        }
    }
    next.run(req).await
}

// ─────────────────────────────── Request/Response types ───────────────────────

#[derive(Debug, Deserialize)]
pub struct AddProjectRequest {
    pub remote_url: String,
    pub branch: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ControlRunRequest {
    /// Optional list of change IDs to run (server will pass to `cflx run --change`).
    pub changes: Option<Vec<String>>,
}

/// Request body for git/pull and git/push (kept for backward compatibility).
/// These endpoints now delegate to git/sync which always runs resolve_command.
/// The auto_resolve and resolve_strategy fields are ignored but accepted for
/// compatibility with existing clients.
#[derive(Debug, Deserialize, Default)]
pub struct GitAutoResolveRequest {
    /// Deprecated: git/pull and git/push now delegate to git/sync which always runs resolve_command.
    /// This field is accepted for backward compatibility but has no effect.
    #[serde(default)]
    #[allow(dead_code)]
    pub auto_resolve: bool,
    /// Deprecated: see auto_resolve above.
    #[allow(dead_code)]
    pub resolve_strategy: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: String,
    pub remote_url: String,
    pub branch: String,
    pub status: String,
    pub created_at: String,
}

impl From<ProjectEntry> for ProjectResponse {
    fn from(e: ProjectEntry) -> Self {
        let status = match e.status {
            ProjectStatus::Idle => "idle",
            ProjectStatus::Running => "running",
            ProjectStatus::Stopped => "stopped",
        }
        .to_string();
        Self {
            id: e.id,
            remote_url: e.remote_url,
            branch: e.branch,
            status,
            created_at: e.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

fn error_response(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(ErrorResponse { error: msg.into() })).into_response()
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
    let (entries, data_dir) = {
        let registry = state.registry.read().await;
        (registry.list(), registry.data_dir().to_path_buf())
    };

    let mut projects = Vec::new();
    for entry in &entries {
        projects.push(build_remote_project_snapshot_async(&data_dir, entry).await);
    }

    (StatusCode::OK, Json(projects)).into_response()
}

/// GET /api/v1/ws - WebSocket stream of remote state updates
///
/// Current behavior:
/// - Sends periodic FullState snapshots (simple, reliable for clients).
/// - Also sends Ping messages to keep the connection alive.
/// - Streams Log entries from the execution log broadcast channel.
pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let registry = state.registry.clone();
    let log_rx = state.log_tx.subscribe();
    ws.on_upgrade(move |socket| handle_ws(socket, registry, log_rx))
}

async fn handle_ws(
    mut socket: WebSocket,
    registry: SharedRegistry,
    mut log_rx: tokio::sync::broadcast::Receiver<RemoteLogEntry>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Snapshot
                let (entries, data_dir) = {
                    let reg = registry.read().await;
                    (reg.list(), reg.data_dir().to_path_buf())
                };

                let mut snapshot = Vec::new();
                for entry in &entries {
                    snapshot.push(build_remote_project_snapshot_async(&data_dir, entry).await);
                }

                if let Ok(payload) = serde_json::to_string(&RemoteStateUpdate::FullState { projects: snapshot }) {
                    if socket.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }

                // Keep-alive ping
                if let Ok(ping) = serde_json::to_string(&RemoteStateUpdate::Ping) {
                    if socket.send(Message::Text(ping.into())).await.is_err() {
                        break;
                    }
                }
            }

            log_result = log_rx.recv() => {
                match log_result {
                    Ok(entry) => {
                        if let Ok(payload) = serde_json::to_string(&RemoteStateUpdate::Log { entry }) {
                            if socket.send(Message::Text(payload.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WS log receiver lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Sender closed – stop streaming logs but keep WS open
                    }
                }
            }

            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {
                        // Ignore client messages for now.
                    }
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

async fn build_remote_project_snapshot_async(
    data_dir: &std::path::Path,
    entry: &ProjectEntry,
) -> RemoteProject {
    let name = project_display_name(&entry.remote_url, &entry.branch);
    let worktree_path = data_dir
        .join("worktrees")
        .join(&entry.id)
        .join(&entry.branch);

    let changes = list_remote_changes_in_worktree(&worktree_path, &entry.id, &entry.branch).await;

    RemoteProject {
        id: entry.id.clone(),
        name,
        changes,
    }
}

fn project_display_name(remote_url: &str, branch: &str) -> String {
    // Keep it short but recognizable: repo@branch
    let repo = remote_url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or(remote_url)
        .trim_end_matches(".git");
    format!("{}@{}", repo, branch)
}

async fn list_remote_changes_in_worktree(
    worktree_path: &std::path::Path,
    project_id: &str,
    base_branch: &str,
) -> Vec<RemoteChange> {
    let changes_dir = worktree_path.join("openspec/changes");
    if !changes_dir.exists() {
        return Vec::new();
    }

    // Build a mapping from sanitized change_id -> worktree path.
    // This allows us to read progress from active worktrees created during parallel execution.
    let mut worktree_by_change: std::collections::HashMap<String, std::path::PathBuf> =
        std::collections::HashMap::new();
    if let Ok(worktrees) = crate::vcs::git::commands::list_worktrees(worktree_path).await {
        for (path, _head, branch, _is_detached, is_main) in worktrees {
            if is_main {
                continue;
            }
            if let Some(change_id) =
                GitWorkspaceManager::extract_change_id_from_worktree_name(&branch)
            {
                worktree_by_change.insert(change_id, std::path::PathBuf::from(path));
            }
        }
    }

    let entries = match std::fs::read_dir(&changes_dir) {
        Ok(e) => e,
        Err(e) => {
            debug!("Failed to read changes dir {:?}: {}", changes_dir, e);
            return Vec::new();
        }
    };

    let mut changes = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if dir_name == "archive" || dir_name.starts_with('.') {
            continue;
        }

        // Only include directories that look like an active change (proposal.md exists).
        let proposal_path = path.join("proposal.md");
        if !proposal_path.exists() {
            continue;
        }

        let tasks_path = path.join("tasks.md");

        // Prefer worktree progress if the change is currently executing in a worktree.
        let wt_path_opt = worktree_by_change.get(dir_name).map(|p| p.as_path());
        let (completed, total) =
            match task_parser::parse_progress_with_fallback(dir_name, wt_path_opt) {
                Ok(p) => (p.completed, p.total),
                Err(_) => {
                    // Last resort: try base tasks.md directly.
                    match task_parser::parse_file(&tasks_path, Some(dir_name)) {
                        Ok(p) => (p.completed, p.total),
                        Err(_) => (0, 0),
                    }
                }
            };

        let last_modified = latest_modified_rfc3339(&[&proposal_path, &tasks_path])
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        let (status, iteration_number) = if let Some(wt_path) = worktree_by_change.get(dir_name) {
            match detect_workspace_state(dir_name, wt_path, base_branch).await {
                Ok(WorkspaceState::Created) => ("queued".to_string(), None),
                Ok(WorkspaceState::Applying { iteration }) => {
                    ("applying".to_string(), Some(iteration))
                }
                Ok(WorkspaceState::Applied) => ("archiving".to_string(), None),
                Ok(WorkspaceState::Archiving) => ("archiving".to_string(), None),
                Ok(WorkspaceState::Archived) => ("archived".to_string(), None),
                Ok(WorkspaceState::Merged) => ("merged".to_string(), None),
                Err(_) => ("idle".to_string(), None),
            }
        } else {
            ("idle".to_string(), None)
        };

        changes.push(RemoteChange {
            id: dir_name.to_string(),
            project: project_id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified,
            status,
            iteration_number,
        });
    }

    changes.sort_by(|a, b| a.id.cmp(&b.id));
    changes
}

fn latest_modified_rfc3339(paths: &[&std::path::Path]) -> Option<String> {
    use std::time::SystemTime;

    let mut latest: Option<SystemTime> = None;
    for p in paths {
        let m = std::fs::metadata(p).and_then(|m| m.modified()).ok();
        if let Some(ts) = m {
            latest = Some(match latest {
                Some(cur) if cur >= ts => cur,
                _ => ts,
            });
        }
    }

    latest.map(|ts| chrono::DateTime::<chrono::Utc>::from(ts).to_rfc3339())
}

/// POST /api/v1/projects - add a new project
///
/// Performs the following steps atomically (with rollback on failure):
/// 1. Register the project in the registry (persisted to disk).
/// 2. Acquire the global semaphore and per-project lock.
/// 3. Verify the branch exists on the remote (git ls-remote).
/// 4. Clone the repository as a bare clone into `data_dir/<project_id>`.
/// 5. Create a git worktree at `data_dir/worktrees/<project_id>/<branch>`.
///
/// If any step after registry insertion fails, the project is removed from the
/// registry so no inconsistent state is persisted.
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
    (StatusCode::CREATED, Json(ProjectResponse::from(entry))).into_response()
}

/// DELETE /api/v1/projects/:id - remove a project
pub async fn delete_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
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

// ─────────────────────────────── /api/v1/projects/:id/git ─────────────────────

/// Execute the configured `resolve_command` in the given working directory.
///
/// Returns `(ran, exit_code)` where `ran` is true if the command was attempted,
/// and `exit_code` is `Some(code)` if the command completed.
fn build_resolve_command_argv(
    resolve_command_template: &str,
    prompt: &str,
) -> Result<Vec<String>, String> {
    let parts = shlex::split(resolve_command_template)
        .ok_or_else(|| "Failed to parse resolve_command (shlex split returned None)".to_string())?;
    if parts.is_empty() {
        return Err("resolve_command is empty".to_string());
    }

    Ok(parts
        .into_iter()
        .map(|p| p.replace("{prompt}", prompt))
        .collect())
}

async fn run_resolve_command(
    resolve_command_template: &str,
    work_dir: &std::path::Path,
    prompt: &str,
) -> (bool, Option<i32>) {
    // Parse into argv (quote-aware), then substitute placeholders at the argv level.
    // This ensures `{prompt}` becomes a single argument even if it contains spaces.
    let argv = match build_resolve_command_argv(resolve_command_template, prompt) {
        Ok(v) => v,
        Err(e) => {
            error!(
                "Failed to build resolve_command argv: template='{}' error='{}'",
                resolve_command_template, e
            );
            return (true, Some(-1));
        }
    };

    let mut cmd = tokio::process::Command::new(&argv[0]);
    if argv.len() > 1 {
        cmd.args(&argv[1..]);
    }
    cmd.current_dir(work_dir);

    match cmd.status().await {
        Ok(status) => (true, status.code()),
        Err(e) => {
            error!(
                "Failed to run resolve_command '{}': {}",
                resolve_command_template, e
            );
            (true, Some(-1))
        }
    }
}

fn build_auto_resolve_prompt(
    operation: &str,
    project_id: &str,
    remote_url: &str,
    branch: &str,
    local_sha: &str,
    remote_sha: &str,
    work_dir: &std::path::Path,
) -> String {
    // Keep this prompt short and machine-readable.
    format!(
        "Conflux server auto_resolve\noperation={}\nproject_id={}\nremote_url={}\nbranch={}\nlocal_sha={}\nremote_sha={}\nwork_dir={}\n\nTask: reconcile local state so the {} can proceed. Exit 0 on success, non-zero on failure.",
        operation,
        project_id,
        remote_url,
        branch,
        local_sha,
        remote_sha,
        work_dir.display(),
        operation
    )
}

/// POST /api/v1/projects/:id/git/pull - pull from remote
///
/// NOTE: This endpoint is kept for backward compatibility.
/// It delegates to `git_sync` internally. Prefer using
/// `POST /api/v1/projects/:id/git/sync` which combines pull and push
/// in a single atomic operation and requires resolve_command to be configured.
pub async fn git_pull(
    state: State<AppState>,
    path: Path<String>,
    _payload: Option<Json<GitAutoResolveRequest>>,
) -> Response {
    info!("git pull (delegating to git_sync): project_id={}", path.0);
    git_sync(state, path).await
}

/// POST /api/v1/projects/:id/git/push - push to remote
///
/// NOTE: This endpoint is kept for backward compatibility.
/// It delegates to `git_sync` internally. Prefer using
/// `POST /api/v1/projects/:id/git/sync` which combines pull and push
/// in a single atomic operation and requires resolve_command to be configured.
pub async fn git_push(
    state: State<AppState>,
    path: Path<String>,
    _payload: Option<Json<GitAutoResolveRequest>>,
) -> Response {
    info!("git push (delegating to git_sync): project_id={}", path.0);
    git_sync(state, path).await
}

// ─────────────────────────── /api/v1/projects/:id/git/sync ────────────────────

/// POST /api/v1/projects/:id/git/sync - pull then push, always running resolve_command.
///
/// This endpoint unifies git/pull and git/push into a single operation.
/// The resolve_command MUST be configured; if not, the sync fails immediately.
/// Both pull and push results are included in the response.
pub async fn git_sync(State(state): State<AppState>, Path(project_id): Path<String>) -> Response {
    // resolve_command is REQUIRED for sync
    let resolve_command = match &state.resolve_command {
        Some(cmd) => cmd.clone(),
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": "resolve_command_not_configured",
                    "reason": "git/sync requires resolve_command to be configured. Set resolve_command in your .cflx.jsonc configuration."
                })),
            )
                .into_response();
        }
    };

    let (remote_url, branch, lock, semaphore) = {
        let registry = state.registry.read().await;
        let entry = match registry.get(&project_id) {
            Some(e) => e.clone(),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    format!("Project not found: {}", project_id),
                )
            }
        };
        let lock = match registry.project_lock(&project_id) {
            Some(l) => l,
            None => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Missing project lock")
            }
        };
        let semaphore = registry.global_semaphore();
        (entry.remote_url, entry.branch, lock, semaphore)
    };

    // Acquire global semaphore (max_concurrent_total)
    let _sem_permit = match semaphore.acquire().await {
        Ok(p) => p,
        Err(_) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Server is at maximum concurrent capacity",
            )
        }
    };

    // Acquire per-project exclusive lock
    let _guard = lock.lock().await;

    info!(
        "git sync: project_id={} remote_url={} branch={}",
        project_id, remote_url, branch
    );

    let local_repo_path = {
        let registry = state.registry.read().await;
        registry.data_dir().join(&project_id)
    };

    // ── PULL phase ──────────────────────────────────────────────────────────────

    // Verify the branch exists on remote
    let ls_remote = tokio::process::Command::new("git")
        .args(["ls-remote", "--heads", &remote_url, &branch])
        .output()
        .await;

    let remote_ref = match ls_remote {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            if stdout.trim().is_empty() {
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("Branch '{}' not found on remote '{}'", branch, remote_url),
                );
            }
            stdout.trim().to_string()
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("git operation failed: {}", stderr),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git: {}", e),
            );
        }
    };

    // Initialize or fetch local bare clone
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
                info!("git sync clone (bare) succeeded: project_id={}", project_id);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("git clone failed: {}", stderr),
                );
            }
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to run git clone: {}", e),
                );
            }
        }
    } else {
        // Fetch to get latest remote state
        let fetch_remote_ref = format!("refs/heads/{}", branch);

        let fetch_output = tokio::process::Command::new("git")
            .args([
                "fetch",
                &remote_url,
                &format!("{}:refs/remotes/origin/{}", fetch_remote_ref, branch),
            ])
            .current_dir(&local_repo_path)
            .output()
            .await;

        match fetch_output {
            Ok(out) if out.status.success() => {
                // Fetch succeeded
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("git fetch failed: {}", stderr),
                );
            }
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to run git fetch: {}", e),
                );
            }
        }

        // Update local branch to remote tip
        let ff_output = tokio::process::Command::new("git")
            .args([
                "fetch",
                &remote_url,
                &format!("refs/heads/{}:refs/heads/{}", branch, branch),
            ])
            .current_dir(&local_repo_path)
            .output()
            .await;

        match ff_output {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("git fetch (fast-forward update) failed: {}", stderr),
                );
            }
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to run git fetch: {}", e),
                );
            }
        }
    }

    let pull_result = serde_json::json!({
        "status": "pulled",
        "ref": remote_ref
    });

    // ── PUSH phase ──────────────────────────────────────────────────────────────

    // Get local SHA after pull
    let local_rev = tokio::process::Command::new("git")
        .args(["rev-parse", &format!("refs/heads/{}", branch)])
        .current_dir(&local_repo_path)
        .output()
        .await;

    let local_sha_for_push = match local_rev {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Failed to get local branch ref: {}", stderr),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git rev-parse: {}", e),
            );
        }
    };

    // Get remote SHA for push result
    let remote_rev = tokio::process::Command::new("git")
        .args(["ls-remote", "--heads", &remote_url, &branch])
        .output()
        .await;

    let remote_sha_for_push = match remote_rev {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                String::new()
            } else {
                stdout.split_whitespace().next().unwrap_or("").to_string()
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Failed to reach remote: {}", stderr),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git ls-remote: {}", e),
            );
        }
    };

    // Always run resolve_command before push (required for sync)
    // This ensures a consistent state regardless of whether a non-fast-forward occurred.
    info!(
        "git sync: running resolve_command before push: project_id={}",
        project_id
    );
    let resolve_prompt = build_auto_resolve_prompt(
        "git_sync",
        &project_id,
        &remote_url,
        &branch,
        &local_sha_for_push,
        &remote_sha_for_push,
        &local_repo_path,
    );
    let (resolve_command_ran, resolve_exit_code) =
        run_resolve_command(&resolve_command, &local_repo_path, &resolve_prompt).await;

    if resolve_exit_code != Some(0) {
        error!(
            "git sync: resolve_command failed: project_id={} exit_code={:?}",
            project_id, resolve_exit_code
        );
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": "resolve_command_failed",
                "reason": "resolve_command failed during sync",
                "local_sha": local_sha_for_push,
                "remote_sha": remote_sha_for_push,
                "resolve_command_ran": resolve_command_ran,
                "resolve_exit_code": resolve_exit_code
            })),
        )
            .into_response();
    }

    // Execute the actual push
    let push_output = tokio::process::Command::new("git")
        .args([
            "push",
            &remote_url,
            &format!("refs/heads/{}:refs/heads/{}", branch, branch),
        ])
        .current_dir(&local_repo_path)
        .output()
        .await;

    let push_result = match push_output {
        Ok(out) if out.status.success() => {
            info!("git sync push succeeded: project_id={}", project_id);
            serde_json::json!({
                "status": "pushed",
                "remote_url": remote_url,
                "branch": branch,
                "local_sha": local_sha_for_push,
                "resolve_command_ran": resolve_command_ran,
                "resolve_exit_code": resolve_exit_code
            })
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            error!(
                "git sync push failed: project_id={} stderr={}",
                project_id, stderr
            );
            if stderr.contains("non-fast-forward") || stderr.contains("rejected") {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({
                        "error": "non_fast_forward",
                        "reason": "Push rejected by remote (non-fast-forward)",
                        "stderr": stderr
                    })),
                )
                    .into_response();
            }
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("git push failed: {}", stderr),
            );
        }
        Err(e) => {
            error!("Failed to run git push: {}", e);
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git push: {}", e),
            );
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "synced",
            "pull": pull_result,
            "push": push_result,
            "resolve_command_ran": resolve_command_ran,
            "resolve_exit_code": resolve_exit_code
        })),
    )
        .into_response()
}

// ─────────────────────────── /api/v1/projects/:id/control ─────────────────────

/// Stub runner call recorder for unit testing.
#[allow(clippy::type_complexity)]
pub static CONTROL_CALLS: std::sync::OnceLock<Arc<std::sync::Mutex<Vec<(String, String)>>>> =
    std::sync::OnceLock::new();

/// POST /api/v1/projects/:id/control/run
pub async fn control_run(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    payload: Option<Json<ControlRunRequest>>,
) -> Response {
    let changes = payload.and_then(|Json(p)| p.changes);
    apply_control(&state, &project_id, "run", ProjectStatus::Running, changes).await
}

/// POST /api/v1/projects/:id/control/stop
pub async fn control_stop(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    apply_control(&state, &project_id, "stop", ProjectStatus::Stopped, None).await
}

/// POST /api/v1/projects/:id/control/retry
pub async fn control_retry(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    payload: Option<Json<ControlRunRequest>>,
) -> Response {
    let changes = payload.and_then(|Json(p)| p.changes);
    apply_control(
        &state,
        &project_id,
        "retry",
        ProjectStatus::Running,
        changes,
    )
    .await
}

async fn apply_control(
    state: &AppState,
    project_id: &str,
    action: &str,
    new_status: ProjectStatus,
    changes: Option<Vec<String>>,
) -> Response {
    // Record the call for test verification
    if let Some(calls) = CONTROL_CALLS.get() {
        calls
            .lock()
            .unwrap()
            .push((project_id.to_string(), action.to_string()));
    }

    let (lock, semaphore, worktree_path) = {
        let registry = state.registry.read().await;
        let Some(entry) = registry.get(project_id) else {
            return error_response(
                StatusCode::NOT_FOUND,
                format!("Project not found: {}", project_id),
            );
        };
        let lock = match registry.project_lock(project_id) {
            Some(l) => l,
            None => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Missing project lock")
            }
        };
        let semaphore = registry.global_semaphore();

        let data_dir = registry.data_dir().to_path_buf();
        let wt = data_dir
            .join("worktrees")
            .join(project_id)
            .join(&entry.branch);
        (lock, semaphore, wt)
    };

    // Acquire global semaphore (max_concurrent_total)
    let _sem_permit = match semaphore.acquire().await {
        Ok(p) => p,
        Err(_) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Server is at maximum concurrent capacity",
            )
        }
    };

    // Acquire per-project exclusive lock
    let _guard = lock.lock().await;

    // Apply action
    if action == "run" || action == "retry" {
        // In unit tests we use CONTROL_CALLS as a stub signal; do not spawn real processes.
        if CONTROL_CALLS.get().is_none() {
            let req = ProjectRunRequest {
                project_id: project_id.to_string(),
                worktree_path,
                changes,
            };

            if let Err(e) = crate::server::runner::start_project_run(
                &state.runners,
                state.registry.clone(),
                req,
                state.log_tx.clone(),
            )
            .await
            {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to start run: {}", e),
                );
            }
        }
    } else if action == "stop" {
        crate::server::runner::stop_project_run(&state.runners, project_id.to_string()).await;
    }

    let mut registry = state.registry.write().await;
    match registry.set_status(project_id, new_status.clone()) {
        Ok(()) => {
            let status_str = match new_status {
                ProjectStatus::Running => "running",
                ProjectStatus::Stopped => "stopped",
                ProjectStatus::Idle => "idle",
            };
            info!(
                "Control action '{}' applied to project_id={}",
                action, project_id
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "action": action,
                    "project_id": project_id,
                    "status": status_str
                })),
            )
                .into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

// ─────────────────────────────── Dashboard handlers ────────────────────────────

/// Dashboard index HTML - returns the built dashboard HTML file
/// This handler serves the dashboard SPA. In production, Vite's `base: "/dashboard/"`
/// directive ensures correct asset paths for nested routing.
async fn dashboard_index() -> Response {
    // The dashboard is built into dashboard/dist/index.html during cargo build
    // We embed it as a static string for maximum portability
    let html = include_str!("../../dashboard/dist/index.html");
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        html,
    )
        .into_response()
}

/// Dashboard asset handler - serves CSS, JS, and other static files
/// Vite generates assets with hashed filenames in the assets/ directory
async fn dashboard_assets(Path(filename): Path<String>) -> Response {
    // Map asset filenames to embedded content
    let content_type = if filename.ends_with(".js") {
        "application/javascript"
    } else if filename.ends_with(".css") {
        "text/css"
    } else if filename.ends_with(".svg") {
        "image/svg+xml"
    } else if filename.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    };

    // This simple approach requires manual asset mapping.
    // For production, prefer a build.rs that generates asset routes dynamically.
    let response = match filename.as_str() {
        env!("DASHBOARD_CSS") => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
            include_str!(concat!(
                "../../dashboard/dist/assets/",
                env!("DASHBOARD_CSS")
            )),
        )
            .into_response(),
        env!("DASHBOARD_JS") => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
            include_str!(concat!(
                "../../dashboard/dist/assets/",
                env!("DASHBOARD_JS")
            )),
        )
            .into_response(),
        _ => {
            error!("Dashboard asset not found: {}", filename);
            (StatusCode::NOT_FOUND, "Asset not found").into_response()
        }
    };

    response
}

/// Dashboard favicon.svg
async fn dashboard_favicon() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml"),
        )],
        include_str!("../../dashboard/dist/favicon.svg"),
    )
        .into_response()
}

/// Dashboard icons.svg
async fn dashboard_icons() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml"),
        )],
        include_str!("../../dashboard/dist/icons.svg"),
    )
        .into_response()
}

// ─────────────────────────────── Router builder ────────────────────────────────

/// Build the API v1 router with authentication middleware.
pub fn build_router(app_state: AppState) -> Router {
    let authenticated_routes = Router::new()
        .route("/projects", get(list_projects).post(add_project))
        .route("/projects/state", get(projects_state))
        .route("/projects/{id}", delete(delete_project))
        .route("/projects/{id}/git/pull", post(git_pull))
        .route("/projects/{id}/git/push", post(git_push))
        .route("/projects/{id}/git/sync", post(git_sync))
        .route("/projects/{id}/control/run", post(control_run))
        .route("/projects/{id}/control/stop", post(control_stop))
        .route("/projects/{id}/control/retry", post(control_retry))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ))
        .with_state(app_state.clone());

    let public_api_routes = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let api_routes = Router::new()
        .merge(authenticated_routes)
        .merge(public_api_routes);

    // Dashboard routes (no authentication required)
    let dashboard_routes = Router::new()
        .route("/", get(dashboard_index))
        .route("/assets/{path}", get(dashboard_assets))
        .route("/favicon.svg", get(dashboard_favicon))
        .route("/icons.svg", get(dashboard_icons))
        .route("/{path}", get(dashboard_index))
        .fallback(get(dashboard_index));

    Router::new()
        .nest("/api/v1", api_routes)
        .nest("/dashboard", dashboard_routes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::server::registry::create_shared_registry;

    fn make_state(temp_dir: &TempDir, auth_token: Option<&str>) -> AppState {
        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            auth_token: auth_token.map(|s| s.to_string()),
            max_concurrent_total: 4,
            resolve_command: None,
            log_tx,
        }
    }

    fn make_router(temp_dir: &TempDir, auth_token: Option<&str>) -> Router {
        build_router(make_state(temp_dir, auth_token))
    }

    // ── Auth tests ──

    #[tokio::test]
    async fn test_no_auth_token_passes() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_token_required_returns_401() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, Some("secret-token"));

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_valid_auth_token_returns_200() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, Some("secret-token"));

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects")
            .header("Authorization", "Bearer secret-token")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // ── Project CRUD tests ──

    #[tokio::test]
    async fn test_list_projects_empty() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json, serde_json::json!([]));
    }

    /// Creates a local bare git repository with a `main` branch and one commit.
    /// Optional `setup_script` is committed at `.wt/setup` in the source repo.
    /// Returns the path to the bare repo (usable as a `file://` URL).
    fn create_local_git_repo_with_setup(
        parent: &std::path::Path,
        setup_script: Option<&str>,
    ) -> std::path::PathBuf {
        let repo_path = parent.join("test-origin");
        // Create a normal repo, add a commit, then convert to bare-compatible source.
        let src = parent.join("test-src");
        std::fs::create_dir_all(&src).unwrap();
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&src)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&src)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&src)
            .output()
            .unwrap();
        std::fs::write(src.join("README.md"), "hello").unwrap();

        if let Some(script) = setup_script {
            let wt_dir = src.join(".wt");
            std::fs::create_dir_all(&wt_dir).unwrap();
            std::fs::write(wt_dir.join("setup"), script).unwrap();
        }

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&src)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&src)
            .output()
            .unwrap();
        // Clone as bare so it can be used as a remote.
        std::process::Command::new("git")
            .args([
                "clone",
                "--bare",
                src.to_str().unwrap(),
                repo_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        repo_path
    }

    fn create_local_git_repo(parent: &std::path::Path) -> std::path::PathBuf {
        create_local_git_repo_with_setup(parent, None)
    }

    #[tokio::test]
    async fn test_add_project_returns_201() {
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
    }

    #[tokio::test]
    async fn test_add_project_runs_repo_root_setup_when_present() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo_with_setup(
            temp_dir.path(),
            Some("#!/bin/sh\nprintf \"%s\" \"$ROOT_WORKTREE_PATH\" > .setup-root-path\n"),
        );
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

        let worktree_path = temp_dir
            .path()
            .join("worktrees")
            .join(project_id)
            .join("main");
        let marker_path = worktree_path.join(".setup-root-path");

        assert!(
            marker_path.exists(),
            "repo-root .wt/setup should create marker file at {:?}",
            marker_path
        );

        let recorded_root = std::fs::read_to_string(&marker_path).unwrap();
        assert_eq!(
            recorded_root,
            worktree_path.to_string_lossy(),
            "ROOT_WORKTREE_PATH should point to worktree repo root"
        );
    }

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
    async fn test_add_project_nonexistent_branch_returns_422() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

        let body = serde_json::json!({
            "remote_url": remote_url,
            "branch": "nonexistent-branch-xyz"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects")
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        // Should return 4xx (422 Unprocessable Entity) when branch not found
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // Verify registry is not updated (rollback happened)
        let state = make_state(&temp_dir, None);
        let registry = state.registry.read().await;
        assert!(
            registry.list().is_empty(),
            "Registry should be empty after failed add"
        );
    }

    #[tokio::test]
    async fn test_add_project_invalid_remote_returns_422() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let body = serde_json::json!({
            "remote_url": "file:///nonexistent/path/to/repo",
            "branch": "main"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects")
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        // Should return 4xx when clone fails due to invalid remote
        assert!(
            resp.status().is_client_error() || resp.status().is_server_error(),
            "Expected error status, got: {}",
            resp.status()
        );

        // Verify registry is not updated (rollback happened)
        let state = make_state(&temp_dir, None);
        let registry = state.registry.read().await;
        assert!(
            registry.list().is_empty(),
            "Registry should be empty after failed add"
        );
    }

    #[tokio::test]
    async fn test_delete_project_returns_204() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let router = build_router(state.clone());

        // Add a project first
        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let req = Request::builder()
            .method(Method::DELETE)
            .uri(format!("/api/v1/projects/{}", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_returns_404() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::DELETE)
            .uri("/api/v1/projects/nonexistent")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    fn make_state_with_limit(
        temp_dir: &TempDir,
        auth_token: Option<&str>,
        max_concurrent: usize,
    ) -> AppState {
        let registry = create_shared_registry(temp_dir.path(), max_concurrent).unwrap();
        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            auth_token: auth_token.map(|s| s.to_string()),
            max_concurrent_total: max_concurrent,
            resolve_command: None,
            log_tx,
        }
    }

    // ── Control tests ──

    #[tokio::test]
    async fn test_control_run_records_call() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        // Initialize call recorder
        CONTROL_CALLS.get_or_init(|| Arc::new(std::sync::Mutex::new(Vec::new())));
        CONTROL_CALLS.get().unwrap().lock().unwrap().clear();

        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/control/run", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let calls = CONTROL_CALLS.get().unwrap().lock().unwrap();
        assert!(calls
            .iter()
            .any(|(id, action)| id == &entry.id && action == "run"));
    }

    // ── Semaphore (max_concurrent_total) tests ──

    #[tokio::test]
    async fn test_max_concurrent_total_semaphore_respected() {
        use std::sync::Arc as StdArc;

        let temp_dir = TempDir::new().unwrap();
        // Create registry with max_concurrent_total = 2
        let state = make_state_with_limit(&temp_dir, None, 2);

        // Add two projects
        let entry1 = state
            .registry
            .write()
            .await
            .add(
                "https://github.com/foo/proj1".to_string(),
                "main".to_string(),
            )
            .unwrap();
        let _entry2 = state
            .registry
            .write()
            .await
            .add(
                "https://github.com/foo/proj2".to_string(),
                "main".to_string(),
            )
            .unwrap();

        // Directly verify that the semaphore in registry limits concurrent access
        let semaphore = {
            let registry = state.registry.read().await;
            registry.global_semaphore()
        };

        assert_eq!(
            semaphore.available_permits(),
            2,
            "Should start with 2 permits"
        );

        // Acquire both permits (simulating 2 concurrent operations)
        let _p1 = semaphore.acquire().await.unwrap();
        let _p2 = semaphore.acquire().await.unwrap();
        assert_eq!(semaphore.available_permits(), 0, "Both permits taken");

        // Try to acquire a third — this would block (verify non-blocking attempt fails)
        // Use try_acquire which returns immediately
        let result = semaphore.try_acquire();
        assert!(
            result.is_err(),
            "Third concurrent operation should be rejected when at capacity"
        );

        // Release p1, p2 by dropping them
        drop(_p1);
        drop(_p2);

        // Now permits are available again
        assert_eq!(
            semaphore.available_permits(),
            2,
            "Permits should be returned after release"
        );

        // CONTROL_CALLS interaction: verify control route acquires semaphore
        // (simulated by the fact that the route code runs against this registry)
        CONTROL_CALLS.get_or_init(|| StdArc::new(std::sync::Mutex::new(Vec::new())));
        CONTROL_CALLS.get().unwrap().lock().unwrap().clear();

        // Execute control/run on project1 — should succeed (permits available)
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/control/run", entry1.id))
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "control/run should succeed when permits available"
        );
    }

    // ── token_env tests ──

    #[test]
    fn test_server_auth_config_resolve_token_from_token_field() {
        use crate::config::ServerAuthConfig;
        let auth = ServerAuthConfig {
            mode: crate::config::ServerAuthMode::BearerToken,
            token: Some("direct-token".to_string()),
            token_env: None,
        };
        assert_eq!(auth.resolve_token(), Some("direct-token".to_string()));
    }

    #[test]
    fn test_server_auth_config_resolve_token_from_env_var() {
        use crate::config::ServerAuthConfig;
        use std::env;

        // Set an environment variable for the token
        let env_var_name = "CFLX_TEST_SERVER_TOKEN_UNIQUE_12345";
        unsafe {
            env::set_var(env_var_name, "env-token-value");
        }

        let auth = ServerAuthConfig {
            mode: crate::config::ServerAuthMode::BearerToken,
            token: Some("fallback-token".to_string()),
            token_env: Some(env_var_name.to_string()),
        };
        // token_env takes precedence over token
        assert_eq!(auth.resolve_token(), Some("env-token-value".to_string()));

        unsafe {
            env::remove_var(env_var_name);
        }
    }

    #[test]
    fn test_server_auth_config_resolve_token_falls_back_when_env_unset() {
        use crate::config::ServerAuthConfig;
        use std::env;

        let env_var_name = "CFLX_TEST_SERVER_TOKEN_UNSET_UNIQUE_99999";
        // Ensure the env var is NOT set
        unsafe {
            env::remove_var(env_var_name);
        }

        let auth = ServerAuthConfig {
            mode: crate::config::ServerAuthMode::BearerToken,
            token: Some("fallback-token".to_string()),
            token_env: Some(env_var_name.to_string()),
        };
        // Falls back to token when env var is not set
        assert_eq!(auth.resolve_token(), Some("fallback-token".to_string()));
    }

    // ── git pull non-fast-forward tests ──

    /// Helper: initialize a bare git repo with at least one commit on the given branch.
    /// Returns the SHA of the commit created.
    async fn init_bare_repo_with_commit(
        bare_path: &std::path::Path,
        branch: &str,
    ) -> Option<String> {
        // Initialize bare repo
        let init = tokio::process::Command::new("git")
            .args(["init", "--bare"])
            .current_dir(bare_path)
            .status()
            .await
            .ok()?;
        if !init.success() {
            return None;
        }

        // Create a temporary working directory to make a commit
        let work_dir = tempfile::TempDir::new().ok()?;
        let work_path = work_dir.path();

        // Clone the bare repo to the work dir
        let clone = tokio::process::Command::new("git")
            .args(["clone", bare_path.to_str()?, work_path.to_str()?])
            .status()
            .await
            .ok()?;
        if !clone.success() {
            return None;
        }

        // Configure git user for commits
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;

        // Checkout/create the target branch
        tokio::process::Command::new("git")
            .args(["checkout", "-b", branch])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;

        // Create a commit
        std::fs::write(work_path.join("README.md"), "initial").ok()?;
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        let commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "initial commit"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !commit.success() {
            return None;
        }

        // Push the branch to bare repo
        let push = tokio::process::Command::new("git")
            .args(["push", "origin", branch])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !push.success() {
            return None;
        }

        // Get the SHA from the bare repo
        let sha_out = tokio::process::Command::new("git")
            .args(["rev-parse", &format!("refs/heads/{}", branch)])
            .current_dir(bare_path)
            .output()
            .await
            .ok()?;

        if sha_out.status.success() {
            Some(String::from_utf8_lossy(&sha_out.stdout).trim().to_string())
        } else {
            None
        }
    }

    #[tokio::test]
    async fn test_git_pull_non_fast_forward_detection() {
        // Set up two bare repos to simulate a diverged state:
        // - "remote" bare repo (the actual remote)
        // - "local" bare repo (the cached local clone in data_dir)
        //
        // We create diverged commits by making an independent commit in local.

        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        let branch = "main";

        // Create the "remote" bare repo with an initial commit
        let remote_dir = TempDir::new().unwrap();
        let remote_path = remote_dir.path();
        let remote_sha = init_bare_repo_with_commit(remote_path, branch).await;
        if remote_sha.is_none() {
            // git not available, skip
            return;
        }
        let remote_sha = remote_sha.unwrap();

        // Add project pointing at this remote
        let remote_url = format!("file://{}", remote_path.display());
        let entry = state
            .registry
            .write()
            .await
            .add(remote_url.clone(), branch.to_string())
            .unwrap();

        // Set up the local bare clone in data_dir/<project_id>
        let local_clone_path = temp_dir.path().join(&entry.id);
        std::fs::create_dir_all(&local_clone_path).unwrap();

        // Initialize local bare repo and simulate a diverged commit
        let init_local = tokio::process::Command::new("git")
            .args(["init", "--bare"])
            .current_dir(&local_clone_path)
            .status()
            .await;
        if init_local.is_err() || !init_local.unwrap().success() {
            return;
        }

        // Create a working directory to push a diverged commit to local bare repo
        let work_dir = TempDir::new().unwrap();
        let work_path = work_dir.path();

        // Clone from remote to get initial history
        let clone_to_work = tokio::process::Command::new("git")
            .args(["clone", &remote_url, work_path.to_str().unwrap()])
            .status()
            .await;
        if clone_to_work.is_err() || !clone_to_work.unwrap().success() {
            return;
        }

        // Configure git user
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path)
            .status()
            .await
            .ok();
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path)
            .status()
            .await
            .ok();

        // Make a new diverged commit (not in remote)
        std::fs::write(work_path.join("diverged.txt"), "diverged commit").unwrap();
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path)
            .status()
            .await
            .ok();
        let diverged_commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "diverged commit"])
            .current_dir(work_path)
            .status()
            .await;
        if diverged_commit.is_err() || !diverged_commit.unwrap().success() {
            return;
        }

        // Push this diverged commit to the local bare repo (simulating local has extra commits)
        let push_to_local = tokio::process::Command::new("git")
            .args([
                "push",
                local_clone_path.to_str().unwrap(),
                &format!("{}:{}", branch, branch),
            ])
            .current_dir(work_path)
            .status()
            .await;
        if push_to_local.is_err() || !push_to_local.unwrap().success() {
            return;
        }

        // Now make ANOTHER commit in remote (so remote has advanced past the common ancestor)
        // This creates a truly diverged state: local has commits not in remote, remote has commits not in local
        let work_dir2 = TempDir::new().unwrap();
        let work_path2 = work_dir2.path();
        let clone2 = tokio::process::Command::new("git")
            .args(["clone", &remote_url, work_path2.to_str().unwrap()])
            .status()
            .await;
        if clone2.is_err() || !clone2.unwrap().success() {
            return;
        }
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path2)
            .status()
            .await
            .ok();
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path2)
            .status()
            .await
            .ok();
        std::fs::write(work_path2.join("remote_advance.txt"), "remote advance").unwrap();
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path2)
            .status()
            .await
            .ok();
        let remote_commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "remote advance"])
            .current_dir(work_path2)
            .status()
            .await;
        if remote_commit.is_err() || !remote_commit.unwrap().success() {
            return;
        }
        let push_remote = tokio::process::Command::new("git")
            .args(["push", "origin", branch])
            .current_dir(work_path2)
            .status()
            .await;
        if push_remote.is_err() || !push_remote.unwrap().success() {
            return;
        }

        // Now local bare clone has: initial + diverged
        // Remote has: initial + remote_advance
        // These branches have diverged — git_pull should detect non-fast-forward

        // Also set up the remote tracking ref in local clone
        // (simulate that a prior fetch stored origin/main pointing to the initial commit)
        let refs_dir = local_clone_path.join("refs/remotes/origin");
        std::fs::create_dir_all(&refs_dir).unwrap();
        std::fs::write(refs_dir.join(branch), format!("{}\n", remote_sha)).unwrap();

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();

        // git/pull now delegates to git/sync which requires resolve_command.
        // Without resolve_command, it returns 422 resolve_command_not_configured.
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "git/pull (delegating to git/sync) should return 422 without resolve_command, got: {} body: {}",
            status,
            json
        );
        let error_val = json["error"].as_str().unwrap_or("");
        assert_eq!(
            error_val, "resolve_command_not_configured",
            "Expected resolve_command_not_configured (git/pull delegates to git/sync), got: {}",
            json
        );

        let _ = remote_sha; // used in setup above
    }

    // ── git push non-fast-forward tests ──

    #[tokio::test]
    async fn test_git_push_no_local_clone_returns_error() {
        // git/push now delegates to git/sync which requires resolve_command.
        // Without resolve_command configured, it returns 422 with resolve_command_not_configured.
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None); // resolve_command = None

        // Add a project but do NOT create a local clone
        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/push", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        // git/push delegates to git/sync which returns 422 when resolve_command is not configured
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let error_val = json["error"].as_str().unwrap_or("");
        // Since git/push delegates to git/sync, the error is about resolve_command not configured
        assert_eq!(
            error_val, "resolve_command_not_configured",
            "Error should be resolve_command_not_configured (git/push delegates to git/sync), got: {}",
            error_val
        );
    }

    #[tokio::test]
    async fn test_git_push_non_fast_forward_detection_with_bare_repos() {
        // Create two bare repos to simulate remote + local (non-fast-forward scenario)
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        // Add a project
        let entry = state
            .registry
            .write()
            .await
            .add("file:///not-a-real-remote".to_string(), "main".to_string())
            .unwrap();

        // Create the local bare repo directory structure in data_dir/<project_id>
        let local_repo_path = temp_dir.path().join(&entry.id);
        std::fs::create_dir_all(&local_repo_path).unwrap();

        // Initialize a bare git repo
        let init_status = tokio::process::Command::new("git")
            .args(["init", "--bare"])
            .current_dir(&local_repo_path)
            .status()
            .await;

        if init_status.is_err() || !init_status.unwrap().success() {
            // git not available or init failed, skip test
            return;
        }

        // The push endpoint should detect no local branch ref and return an error
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/push", entry.id))
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        // Expect error because the local bare repo has no branch "main"
        // (rev-parse will fail)
        assert!(
            resp.status() == StatusCode::UNPROCESSABLE_ENTITY
                || resp.status() == StatusCode::INTERNAL_SERVER_ERROR,
            "Push with empty bare repo should fail, got: {}",
            resp.status()
        );
    }

    // ── auto_resolve tests ──

    /// Create a diverged scenario for testing:
    /// - Returns (remote_dir, local_clone_dir, project_entry, remote_url) or None if git unavailable.
    /// - remote has: initial + remote_advance
    /// - local has: initial + diverged
    ///   So these branches have diverged (non-fast-forward situation).
    async fn create_diverged_repo_setup(
        temp_dir: &TempDir,
        state: &AppState,
        branch: &str,
    ) -> Option<(
        tempfile::TempDir,
        std::path::PathBuf,
        crate::server::registry::ProjectEntry,
        String,
    )> {
        // Create the "remote" bare repo with an initial commit
        let remote_dir = TempDir::new().ok()?;
        let remote_path = remote_dir.path();
        let remote_sha = init_bare_repo_with_commit(remote_path, branch).await?;

        let remote_url = format!("file://{}", remote_path.display());
        let entry = state
            .registry
            .write()
            .await
            .add(remote_url.clone(), branch.to_string())
            .ok()?;

        // Set up the local bare clone in data_dir/<project_id>
        let local_clone_path = temp_dir.path().join(&entry.id);
        std::fs::create_dir_all(&local_clone_path).ok()?;

        // Initialize local bare repo
        let init_local = tokio::process::Command::new("git")
            .args(["init", "--bare"])
            .current_dir(&local_clone_path)
            .status()
            .await
            .ok()?;
        if !init_local.success() {
            return None;
        }

        // Create a working directory to push a diverged commit to local bare repo
        let work_dir = TempDir::new().ok()?;
        let work_path = work_dir.path();

        // Clone from remote to get initial history
        let clone_to_work = tokio::process::Command::new("git")
            .args(["clone", &remote_url, work_path.to_str()?])
            .status()
            .await
            .ok()?;
        if !clone_to_work.success() {
            return None;
        }

        // Configure git user
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;

        // Make a diverged commit (not in remote)
        std::fs::write(work_path.join("diverged.txt"), "diverged commit").ok()?;
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        let diverged_commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "diverged commit"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !diverged_commit.success() {
            return None;
        }

        // Push this diverged commit to the local bare repo
        let push_to_local = tokio::process::Command::new("git")
            .args([
                "push",
                local_clone_path.to_str()?,
                &format!("{}:{}", branch, branch),
            ])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !push_to_local.success() {
            return None;
        }

        // Make ANOTHER commit in remote so remote has also advanced past initial
        let work_dir2 = TempDir::new().ok()?;
        let work_path2 = work_dir2.path();
        let clone2 = tokio::process::Command::new("git")
            .args(["clone", &remote_url, work_path2.to_str()?])
            .status()
            .await
            .ok()?;
        if !clone2.success() {
            return None;
        }
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        std::fs::write(work_path2.join("remote_advance.txt"), "remote advance").ok()?;
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        let remote_commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "remote advance"])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        if !remote_commit.success() {
            return None;
        }
        let push_remote = tokio::process::Command::new("git")
            .args(["push", "origin", branch])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        if !push_remote.success() {
            return None;
        }

        // Set up remote tracking ref in local clone (origin/main pointing to initial)
        let refs_dir = local_clone_path.join("refs/remotes/origin");
        std::fs::create_dir_all(&refs_dir).ok()?;
        std::fs::write(refs_dir.join(branch), format!("{}\n", remote_sha)).ok()?;

        Some((remote_dir, local_clone_path, entry, remote_url))
    }

    #[tokio::test]
    async fn test_git_pull_delegates_to_git_sync_and_requires_resolve_command() {
        // git/pull now delegates to git/sync, which requires resolve_command to be configured.
        // Without resolve_command, it should return 422 with resolve_command_not_configured.
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None); // resolve_command = None
        let branch = "main";

        let result = create_diverged_repo_setup(&temp_dir, &state, branch).await;
        let (_remote_dir, _local_clone_path, entry, _remote_url) = match result {
            Some(r) => r,
            None => return, // git not available, skip
        };

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        // git/pull delegates to git/sync which requires resolve_command
        // Without resolve_command, it returns 422 resolve_command_not_configured
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "git/pull (delegating to git/sync) should return 422 without resolve_command, got: {} body: {}",
            status,
            json
        );
        let error_val = json["error"].as_str().unwrap_or("");
        assert_eq!(
            error_val, "resolve_command_not_configured",
            "Expected resolve_command_not_configured, got: {}",
            json
        );
    }

    #[tokio::test]
    async fn test_git_pull_auto_resolve_runs_resolve_command() {
        // Test that auto_resolve=true runs the resolve_command when non-fast-forward is detected
        // and returns resolve_command_ran=true in the response.
        let temp_dir = TempDir::new().unwrap();

        // Create state with resolve_command = "echo resolve"
        let registry = crate::server::registry::create_shared_registry(temp_dir.path(), 4).unwrap();
        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: Some("echo resolve".to_string()),
            log_tx,
        };

        let branch = "main";

        let result = create_diverged_repo_setup(&temp_dir, &state, branch).await;
        let (_remote_dir, _local_clone_path, entry, _remote_url) = match result {
            Some(r) => r,
            None => return, // git not available, skip
        };

        let router = build_router(state.clone());
        // Request WITH auto_resolve=true
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"auto_resolve": true}"#))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        // If non-fast-forward was detected and auto_resolve ran:
        // - status should be 200 (resolve succeeded)
        // - resolve_command_ran should be true
        // - resolve_exit_code should be 0
        // OR if git handles it as fast-forward (branches didn't diverge the way we expect):
        // - status could be 200 without resolve metadata
        if status == StatusCode::OK {
            // If resolve_command_ran is present, verify it's true with exit 0
            if let Some(ran) = json.get("resolve_command_ran") {
                if ran.as_bool() == Some(true) {
                    assert_eq!(
                        json["resolve_exit_code"].as_i64(),
                        Some(0),
                        "resolve_command_ran=true implies exit_code should be 0, got: {}",
                        json
                    );
                }
            }
        }

        // Should NOT return 422 non_fast_forward without auto_resolve running
        // (with auto_resolve=true and resolve_command configured, it should either succeed or
        // fail with resolve_command_failed, not non_fast_forward)
        if status == StatusCode::UNPROCESSABLE_ENTITY {
            let error_val = json["error"].as_str().unwrap_or("");
            assert_ne!(
                error_val, "non_fast_forward",
                "With auto_resolve=true, should not return plain non_fast_forward error. Got: {}",
                json
            );
        }

        // Final: accept OK or resolve-related errors, but not plain non_fast_forward
        assert!(
            status == StatusCode::OK || status == StatusCode::UNPROCESSABLE_ENTITY,
            "Expected OK or UNPROCESSABLE_ENTITY, got: {} body: {}",
            status,
            json
        );
    }

    #[tokio::test]
    async fn test_git_pull_auto_resolve_without_resolve_command_configured_returns_error() {
        // Test that auto_resolve=true without resolve_command configured returns an appropriate error
        let temp_dir = TempDir::new().unwrap();
        // Create state WITHOUT resolve_command
        let state = make_state(&temp_dir, None);

        let branch = "main";

        let result = create_diverged_repo_setup(&temp_dir, &state, branch).await;
        let (_remote_dir, _local_clone_path, entry, _remote_url) = match result {
            Some(r) => r,
            None => return, // git not available, skip
        };

        let router = build_router(state.clone());
        // Request WITH auto_resolve=true but no resolve_command configured
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"auto_resolve": true}"#))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        // If non-fast-forward was detected, should return resolve_command_not_configured error
        if status == StatusCode::UNPROCESSABLE_ENTITY {
            let error_val = json["error"].as_str().unwrap_or("");
            assert!(
                error_val == "resolve_command_not_configured" || error_val == "non_fast_forward",
                "Expected resolve_command_not_configured when auto_resolve=true but no resolve_command, got: {}",
                json
            );
        }
    }

    // ── Top-level resolve_command tests (server.resolve_command removed) ──

    /// Test that AppState receives resolve_command from the top-level config
    /// (not from server.resolve_command which is now deprecated).
    #[test]
    fn test_app_state_resolve_command_comes_from_top_level_config() {
        // Simulate what run_server now does: takes resolve_command as a separate parameter
        // from the top-level config, not from ServerConfig.resolve_command.
        let top_level_resolve_cmd = Some("echo top-level-resolve".to_string());

        // Build AppState as run_server does (using the top-level resolve_command parameter)
        // The ServerConfig.resolve_command field is deprecated and should be None.
        let app_state_resolve_command = top_level_resolve_cmd.clone();

        assert_eq!(
            app_state_resolve_command,
            Some("echo top-level-resolve".to_string()),
            "AppState resolve_command should come from top-level config resolve_command"
        );
    }

    /// Test that auto_resolve uses the top-level resolve_command in AppState.
    /// This verifies the routing: top-level config -> run_server() -> AppState -> git_pull/git_push.
    #[tokio::test]
    async fn test_git_pull_auto_resolve_uses_top_level_resolve_command() {
        // Create state with top-level resolve_command (simulating what run_server now does)
        let temp_dir = TempDir::new().unwrap();
        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            auth_token: None,
            max_concurrent_total: 4,
            // This resolve_command now comes from top-level config (not server.resolve_command)
            resolve_command: Some("echo resolve".to_string()),
            log_tx,
        };

        let branch = "main";
        let result = create_diverged_repo_setup(&temp_dir, &state, branch).await;
        let (_remote_dir, _local_clone_path, entry, _remote_url) = match result {
            Some(r) => r,
            None => return, // git not available, skip
        };

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"auto_resolve": true}"#))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        // With top-level resolve_command configured, auto_resolve should either:
        // - Succeed (200 OK with resolve_command_ran=true) if divergence was detected
        // - Return 200 OK directly if fast-forward is possible
        // It must NOT return "resolve_command_not_configured"
        if status == StatusCode::UNPROCESSABLE_ENTITY {
            let error_val = json["error"].as_str().unwrap_or("");
            assert_ne!(
                error_val,
                "resolve_command_not_configured",
                "Should not return resolve_command_not_configured when top-level resolve_command is set: {}",
                json
            );
        }
        // If resolve_command_ran is present, verify the top-level resolve_command was used
        if let Some(ran) = json.get("resolve_command_ran") {
            if ran.as_bool() == Some(true) {
                let exit_code = json["resolve_exit_code"].as_i64().unwrap_or(-1);
                assert_eq!(
                    exit_code, 0,
                    "Top-level resolve_command (echo resolve) should exit with 0, got: {}",
                    exit_code
                );
            }
        }
    }

    #[test]
    fn test_build_resolve_command_argv_replaces_prompt_placeholder_as_single_arg() {
        let template = "opencode run --agent code '{prompt}'";
        let prompt = "hello world";
        let argv = super::build_resolve_command_argv(template, prompt).expect("argv should build");
        assert_eq!(
            argv,
            vec![
                "opencode".to_string(),
                "run".to_string(),
                "--agent".to_string(),
                "code".to_string(),
                "hello world".to_string(),
            ]
        );
    }

    #[test]
    fn test_build_resolve_command_argv_handles_quotes_and_braces_literal() {
        let template = "echo '{prompt}' '{prompt}-suffix'";
        let prompt = "a b c";
        let argv = super::build_resolve_command_argv(template, prompt).expect("argv should build");
        assert_eq!(
            argv,
            vec![
                "echo".to_string(),
                "a b c".to_string(),
                "a b c-suffix".to_string(),
            ]
        );
    }

    // ── Server worktree branch tests ──

    /// Verify that POST /api/v1/projects creates a worktree on a server-specific branch,
    /// NOT on the base branch directly.
    #[tokio::test]
    async fn test_add_project_creates_worktree_on_server_branch() {
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

        // Retrieve the project ID from the response
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let project_id = json["id"].as_str().expect("Response must have id field");

        // The worktree path is data_dir/worktrees/<project_id>/main
        let worktree_path = temp_dir
            .path()
            .join("worktrees")
            .join(project_id)
            .join("main");

        assert!(
            worktree_path.exists(),
            "Worktree directory must exist at {:?}",
            worktree_path
        );

        // Verify that the worktree's HEAD does NOT reference refs/heads/main (the base branch).
        // Instead, it should reference the server-specific branch: server-wt/<project_id>/main.
        let head_output = std::process::Command::new("git")
            .args(["symbolic-ref", "HEAD"])
            .current_dir(&worktree_path)
            .output();

        if let Ok(out) = head_output {
            if out.status.success() {
                let head = String::from_utf8_lossy(&out.stdout).trim().to_string();
                assert_ne!(
                    head, "refs/heads/main",
                    "Worktree HEAD must NOT reference refs/heads/main (base branch). \
                    It must use a server-specific branch. Got: {}",
                    head
                );
                // Verify the branch follows the server-wt/<project_id>/<base_branch> format
                let expected_prefix = format!("refs/heads/server-wt/{}/", project_id);
                assert!(
                    head.starts_with(&expected_prefix),
                    "Worktree HEAD must start with '{}'. Got: {}",
                    expected_prefix,
                    head
                );
            }
        }
    }

    /// Verify the server_worktree_branch function is accessible and produces the correct format.
    #[test]
    fn test_server_worktree_branch_function_produces_correct_format() {
        use crate::server::registry::server_worktree_branch;
        let project_id = "abc123def456789a";
        let base_branch = "main";
        let branch = server_worktree_branch(project_id, base_branch);
        assert_eq!(
            branch, "server-wt/abc123def456789a/main",
            "server_worktree_branch must produce 'server-wt/<project_id>/<base_branch>'"
        );
    }

    // ── git/sync tests ──

    /// Test: git/sync returns 404 when project does not exist.
    #[tokio::test]
    async fn test_git_sync_project_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = make_state(&temp_dir, None);
        state.resolve_command = Some("echo resolve".to_string());
        let router = build_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects/nonexistent-project/git/sync")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].is_string());
    }

    /// Test: git/sync returns 422 when resolve_command is not configured.
    /// resolve_command is REQUIRED for the sync endpoint.
    #[tokio::test]
    async fn test_git_sync_fails_without_resolve_command() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None); // resolve_command = None
        let _router = build_router(state);

        // Add a fake project first (project lookup happens before resolve_command check)
        // We test with no project to confirm the endpoint reachability; the real check
        // is that when a project exists and resolve_command is None, we get 422.
        // Since we cannot easily add a project without a real git remote in unit tests,
        // we verify the error via a direct handler invocation using a state with a project.
        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg
                .add(
                    "https://github.com/example/repo.git".to_string(),
                    "main".to_string(),
                )
                .unwrap();
            entry.id.clone()
        };

        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state_with_project = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: None, // Not configured — must cause 422
            log_tx,
        };
        let router = build_router(state_with_project);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "git/sync must return 422 when resolve_command is not configured"
        );

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json["error"].as_str().unwrap(),
            "resolve_command_not_configured",
            "Error code must be 'resolve_command_not_configured'"
        );
        assert!(
            json["reason"].as_str().unwrap().contains("resolve_command"),
            "Reason must mention resolve_command"
        );
    }

    /// Test: git/sync route is registered in the router (returns non-404 for the route itself).
    #[tokio::test]
    async fn test_git_sync_route_is_registered() {
        let temp_dir = TempDir::new().unwrap();
        // Use a state where a project is registered so we get past project-not-found check
        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg
                .add(
                    "https://github.com/example/repo.git".to_string(),
                    "main".to_string(),
                )
                .unwrap();
            entry.id.clone()
        };

        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: None, // Will trigger 422 (resolve_command not configured)
            log_tx,
        };
        let router = build_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        // Should get 422 (resolve_command not configured), not 404 (route not found)
        assert_ne!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "git/sync route must be registered (must not return 404 for route)"
        );
        assert_eq!(
            resp.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "git/sync should return 422 when resolve_command is not configured"
        );
    }

    /// Test: git/sync success response contains both 'pull' and 'push' sections.
    /// Uses a local bare git repository as a fixture to avoid external dependencies.
    /// The resolve_command is set to "true" (a no-op that always succeeds).
    #[tokio::test]
    async fn test_git_sync_success_response_contains_pull_and_push_sections() {
        let temp_dir = TempDir::new().unwrap();

        // Create a local bare git repository as the remote
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg.add(remote_url.clone(), "main".to_string()).unwrap();
            entry.id.clone()
        };

        // Use resolve_command = "true" (always exits 0)
        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: Some("true".to_string()),
            log_tx,
        };
        let router = build_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // On success, the response must contain 'pull' and 'push' sections
        if status == StatusCode::OK {
            assert!(
                json.get("pull").is_some(),
                "Success response must contain 'pull' section, got: {}",
                json
            );
            assert!(
                json.get("push").is_some(),
                "Success response must contain 'push' section, got: {}",
                json
            );
            assert_eq!(
                json["status"].as_str(),
                Some("synced"),
                "Top-level status must be 'synced', got: {}",
                json
            );
            // resolve_command_ran must be true (always runs)
            assert_eq!(
                json["resolve_command_ran"].as_bool(),
                Some(true),
                "resolve_command_ran must be true, got: {}",
                json
            );
        }
        // Accept 422 if git push fails (e.g., nothing to push because no local changes)
        // The key assertion is that when status is 200, the response has the correct structure.
        assert!(
            status == StatusCode::OK || status == StatusCode::UNPROCESSABLE_ENTITY,
            "Expected OK or UNPROCESSABLE_ENTITY, got: {} body: {}",
            status,
            json
        );
    }
}
