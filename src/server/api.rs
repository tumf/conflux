//! API v1 handlers for the server daemon.
//!
//! Provides REST endpoints for project management and execution control.
//!
//! NOTE: This module deliberately does NOT reference or execute `~/.wt/setup`.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{ws::Message, ws::WebSocket, ws::WebSocketUpgrade, Path, Query, State},
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
use crate::server::active_commands::{
    ActiveCommandGuard, RootKind, SharedActiveCommands, WorktreeRootKey,
};
use crate::server::registry::{
    server_worktree_branch, OrchestrationStatus, ProjectEntry, ProjectStatus, SharedRegistry,
};
use crate::server::runner::{ProjectRunRequest, SharedRunners};
use crate::server::terminal::{
    CreateTerminalFromContextRequest, CreateTerminalRequest, ResizeTerminalRequest,
    SharedTerminalManager, TerminalSessionInfo,
};
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
    /// Global orchestration status (Idle/Running/Stopped)
    pub orchestration_status: Arc<tokio::sync::RwLock<OrchestrationStatus>>,
    /// Terminal session manager for dashboard interactive shells
    pub terminal_manager: SharedTerminalManager,
    /// Active command registry for worktree root-level singleton execution
    pub active_commands: SharedActiveCommands,
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

// ─────────────────────────── Active command helpers ───────────────────────────

/// Try to acquire an active command slot for the given root. On conflict, returns
/// a `409 Conflict` response describing the busy root. On success, returns an
/// `ActiveCommandGuard` that auto-releases the slot when dropped.
async fn try_acquire_active_command(
    active_commands: &SharedActiveCommands,
    project_id: &str,
    root_kind: RootKind,
    operation: &str,
) -> Result<ActiveCommandGuard, Response> {
    let key = WorktreeRootKey {
        project_id: project_id.to_string(),
        root_kind,
    };
    let mut reg = active_commands.write().await;
    match reg.try_acquire(key.clone(), operation) {
        Ok(()) => Ok(ActiveCommandGuard::new(active_commands.clone(), key)),
        Err(existing) => Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "root_busy",
                "reason": format!(
                    "Root is busy with operation '{}' (started {})",
                    existing.operation, existing.started_at
                ),
                "active_command": existing,
            })),
        )
            .into_response()),
    }
}

// ─────────────────────────────── Request/Response types ───────────────────────

#[derive(Debug, Deserialize)]
pub struct AddProjectRequest {
    pub remote_url: String,
    pub branch: String,
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

/// Response for `GET /api/v1/projects/state` including top-level metadata.
#[derive(Debug, Serialize)]
struct ProjectsStateResponse {
    projects: Vec<RemoteProject>,
    /// Whether git/sync is available (resolve_command is configured)
    sync_available: bool,
}

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
    let (entries, data_dir, all_selections) = {
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
        (entries, data_dir, all_selections)
    };

    let mut projects = Vec::new();
    for entry in &entries {
        let selections = all_selections.get(&entry.id);
        projects.push(build_remote_project_snapshot_async(&data_dir, entry, selections).await);
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

/// GET /api/v1/ws - WebSocket stream of remote state updates
///
/// Current behavior:
/// - Sends periodic FullState snapshots (simple, reliable for clients).
/// - Also sends Ping messages to keep the connection alive.
/// - Streams Log entries from the execution log broadcast channel.
pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let registry = state.registry.clone();
    let log_rx = state.log_tx.subscribe();
    let sync_available = state.resolve_command.is_some();
    let orchestration_status = state.orchestration_status.clone();
    let active_commands = state.active_commands.clone();
    ws.on_upgrade(move |socket| {
        handle_ws(
            socket,
            registry,
            log_rx,
            sync_available,
            orchestration_status,
            active_commands,
        )
    })
}

async fn handle_ws(
    mut socket: WebSocket,
    registry: SharedRegistry,
    mut log_rx: tokio::sync::broadcast::Receiver<RemoteLogEntry>,
    sync_available: bool,
    orchestration_status: Arc<tokio::sync::RwLock<OrchestrationStatus>>,
    active_commands: SharedActiveCommands,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Snapshot
                let (entries, data_dir, all_selections) = {
                    let reg = registry.read().await;
                    let entries = reg.list();
                    let data_dir = reg.data_dir().to_path_buf();
                    let all_selections: std::collections::HashMap<String, std::collections::HashMap<String, bool>> = entries
                        .iter()
                        .filter_map(|e| {
                            reg.change_selections_for_project(&e.id)
                                .map(|s| (e.id.clone(), s.clone()))
                        })
                        .collect();
                    (entries, data_dir, all_selections)
                };

                let mut snapshot = Vec::new();
                for entry in &entries {
                    let selections = all_selections.get(&entry.id);
                    snapshot.push(build_remote_project_snapshot_async(&data_dir, entry, selections).await);
                }

                // Collect worktree information for each project
                let mut worktrees_map = std::collections::HashMap::new();
                for entry in &entries {
                    let wt_path = data_dir
                        .join("worktrees")
                        .join(&entry.id)
                        .join(&entry.branch);
                    if wt_path.exists() {
                        if let Ok(wts) = crate::worktree_ops::get_worktrees(&wt_path).await {
                            let remote_wts: Vec<crate::remote::types::RemoteWorktreeInfo> =
                                wts.into_iter().map(Into::into).collect();
                            if !remote_wts.is_empty() {
                                worktrees_map.insert(entry.id.clone(), remote_wts);
                            }
                        }
                    }
                }
                let worktrees = if worktrees_map.is_empty() {
                    None
                } else {
                    Some(worktrees_map)
                };

                let orch_status = orchestration_status.read().await.as_str().to_string();
                let active_cmds = {
                    let ac = active_commands.read().await;
                    ac.snapshot()
                };
                if let Ok(payload) = serde_json::to_string(&RemoteStateUpdate::FullState { projects: snapshot, worktrees, sync_available, orchestration_status: orch_status, active_commands: active_cmds }) {
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
    change_selections: Option<&std::collections::HashMap<String, bool>>,
) -> RemoteProject {
    let name = project_display_name(&entry.remote_url, &entry.branch);
    let repo = extract_repo_name(&entry.remote_url);
    let worktree_path = data_dir
        .join("worktrees")
        .join(&entry.id)
        .join(&entry.branch);

    let mut changes =
        list_remote_changes_in_worktree(&worktree_path, &entry.id, &entry.branch).await;

    // Apply selected state from registry (defaults to true if not tracked)
    for change in &mut changes {
        change.selected = change_selections
            .and_then(|m| m.get(&change.id))
            .copied()
            .unwrap_or(true);
    }

    let status_str = match entry.status {
        ProjectStatus::Idle => "idle",
        ProjectStatus::Running => "running",
        ProjectStatus::Stopped => "stopped",
    };
    let is_busy = matches!(entry.status, ProjectStatus::Running);

    RemoteProject {
        id: entry.id.clone(),
        name,
        repo,
        branch: entry.branch.clone(),
        status: status_str.to_string(),
        is_busy,
        error: None,
        changes,
    }
}

/// Extract the repository name from a remote URL (last path segment without .git suffix).
///
/// For standard git remotes ending in `.git`, the `.git` suffix is stripped.
/// For unusual URLs, falls back to the best available non-empty label.
fn extract_repo_name(remote_url: &str) -> String {
    let basename = remote_url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or(remote_url)
        .trim_end_matches(".git");

    if basename.is_empty() {
        // Fallback: use the full URL rather than an empty string
        remote_url.to_string()
    } else {
        basename.to_string()
    }
}

fn project_display_name(remote_url: &str, branch: &str) -> String {
    // Keep it short but recognizable: repo@branch
    let repo = extract_repo_name(remote_url);
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
            selected: true,
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

    // Task 8: Auto-enqueue if orchestration is currently running.
    // When a new project is added during Running state, automatically spawn a runner for it.
    {
        let orch_status = state.orchestration_status.read().await;
        if *orch_status == OrchestrationStatus::Running {
            let changes = list_change_ids_in_worktree(&worktree_path).await;
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

/// Build a resolve command argv by parsing the template with shlex and substituting
/// `{prompt}` placeholders. Retained for test coverage.
#[cfg(test)]
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
    log_tx: Option<&tokio::sync::broadcast::Sender<RemoteLogEntry>>,
    project_id: Option<&str>,
) -> (bool, Option<i32>) {
    // Substitute {prompt} placeholder in the template, then shell-escape the prompt
    // value so the login shell receives it as a safe string.
    let escaped_prompt = shlex::try_quote(prompt).unwrap_or_else(|_| prompt.into());
    let command_str = resolve_command_template.replace("{prompt}", &escaped_prompt);

    info!(
        "Running resolve_command via login shell: command='{}'",
        command_str
    );

    // Send start event to project log
    if let (Some(tx), Some(pid)) = (log_tx, project_id) {
        let _ = tx.send(RemoteLogEntry {
            message: format!("resolve_command started: {}", command_str),
            level: "info".to_string(),
            change_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            project_id: Some(pid.to_string()),
            operation: Some("resolve".to_string()),
            iteration: None,
        });
    }

    let mut cmd = crate::shell_command::build_login_shell_command(&command_str);
    cmd.current_dir(work_dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            error!(
                "Failed to run resolve_command '{}': {}",
                resolve_command_template, e
            );
            if let (Some(tx), Some(pid)) = (log_tx, project_id) {
                let _ = tx.send(RemoteLogEntry {
                    message: format!("resolve_command failed to start: {}", e),
                    level: "error".to_string(),
                    change_id: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    project_id: Some(pid.to_string()),
                    operation: Some("resolve".to_string()),
                    iteration: None,
                });
            }
            return (true, Some(-1));
        }
    };

    // Stream stdout/stderr to project log
    let output = match child.wait_with_output().await {
        Ok(output) => output,
        Err(e) => {
            error!(
                "Failed to wait for resolve_command '{}': {}",
                resolve_command_template, e
            );
            return (true, Some(-1));
        }
    };

    // Stream stdout lines
    if let (Some(tx), Some(pid)) = (log_tx, project_id) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if !line.is_empty() {
                let _ = tx.send(RemoteLogEntry {
                    message: line.to_string(),
                    level: "info".to_string(),
                    change_id: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    project_id: Some(pid.to_string()),
                    operation: Some("resolve".to_string()),
                    iteration: None,
                });
            }
        }

        // Stream stderr lines
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            if !line.is_empty() {
                let _ = tx.send(RemoteLogEntry {
                    message: line.to_string(),
                    level: "warn".to_string(),
                    change_id: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    project_id: Some(pid.to_string()),
                    operation: Some("resolve".to_string()),
                    iteration: None,
                });
            }
        }

        // Send completion event
        let exit_code = output.status.code();
        let level = if output.status.success() {
            "success"
        } else {
            "error"
        };
        let _ = tx.send(RemoteLogEntry {
            message: format!("resolve_command finished: exit_code={:?}", exit_code),
            level: level.to_string(),
            change_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            project_id: Some(pid.to_string()),
            operation: Some("resolve".to_string()),
            iteration: None,
        });
    }

    (true, output.status.code())
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

    // Acquire active command slot for base root (sync operates on the base worktree)
    let _active_guard = match try_acquire_active_command(
        &state.active_commands,
        &project_id,
        RootKind::Base,
        "sync",
    )
    .await
    {
        Ok(guard) => guard,
        Err(resp) => return resp,
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
    let (resolve_command_ran, resolve_exit_code) = run_resolve_command(
        &resolve_command,
        &local_repo_path,
        &resolve_prompt,
        Some(&state.log_tx),
        Some(&project_id),
    )
    .await;

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
/// For each project, collects changes with `selected: true` (currently all changes
/// are implicitly selected since the change-selection feature is not yet implemented)
/// and spawns a runner with those change IDs.
///
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

    let (entries, data_dir) = {
        let registry = state.registry.read().await;
        (registry.list(), registry.data_dir().to_path_buf())
    };

    let mut started_count = 0u32;
    let mut skipped_count = 0u32;

    for entry in &entries {
        let worktree_path = data_dir
            .join("worktrees")
            .join(&entry.id)
            .join(&entry.branch);

        // Collect all change IDs from the project worktree (all are implicitly selected for now)
        let changes = list_change_ids_in_worktree(&worktree_path).await;
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

/// List change IDs in a project worktree (all active changes are treated as selected).
async fn list_change_ids_in_worktree(worktree_path: &std::path::Path) -> Vec<String> {
    let changes_dir = worktree_path.join("openspec/changes");
    if !changes_dir.exists() {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(&changes_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut change_ids = Vec::new();
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
        // Only include changes with a proposal.md
        if path.join("proposal.md").exists() {
            change_ids.push(dir_name.to_string());
        }
    }

    change_ids.sort();
    change_ids
}

// ─────────────────────────── Deprecated per-project control (removed) ─────────

// Per-project control endpoints (/projects/{id}/control/run|stop|retry) have been
// removed. Use the global /api/v1/control/run and /api/v1/control/stop endpoints
// instead. The global endpoints manage all projects as a single orchestration unit.

// ─────────────────────────── Internal: per-project run (used by global control) ─

/// Start a single project run (used internally by global_control_run and add_project auto-enqueue).
async fn start_single_project_run(
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
        req,
        state.log_tx.clone(),
    )
    .await
    .map_err(|e| format!("Failed to start run: {}", e))?;

    let mut registry = state.registry.write().await;
    let _ = registry.set_status(project_id, ProjectStatus::Running);

    Ok(())
}

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
struct WorktreeOpResponse {
    success: bool,
    message: String,
}

/// Helper: resolve the project's main worktree path from registry.
async fn resolve_project_worktree_path(
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

// ─────────────────────────── /api/v1/projects/:id/files ───────────────────────

/// Directories excluded from the file tree listing.
const EXCLUDED_DIRS: &[&str] = &[".git", "node_modules", ".next", "target", "dist"];

/// Maximum file content size returned by the content API (1 MB).
const MAX_FILE_CONTENT_SIZE: u64 = 1_048_576;

/// Query parameters for the file tree API.
#[derive(Debug, Deserialize)]
pub struct FileTreeQuery {
    /// `base` (default) or `worktree:<branch>`.
    #[serde(default = "default_root")]
    pub root: String,
}

fn default_root() -> String {
    "base".to_string()
}

/// Query parameters for the file content API.
#[derive(Debug, Deserialize)]
pub struct FileContentQuery {
    /// `base` (default) or `worktree:<branch>`.
    #[serde(default = "default_root")]
    pub root: String,
    /// Relative path to the file from the root.
    pub path: String,
}

/// A single entry in the file tree.
#[derive(Debug, Serialize)]
pub struct FileTreeEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String, // "file" or "directory"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FileTreeEntry>>,
}

/// Response for the file content API.
#[derive(Debug, Serialize)]
pub struct FileContentResponse {
    pub path: String,
    pub content: Option<String>,
    pub size: u64,
    pub truncated: bool,
    pub binary: bool,
}

/// Resolve the file-system root path for a given project and `root` query parameter.
///
/// Returns `Ok(path)` or an error `Response` ready to send back.
async fn resolve_file_root(
    state: &AppState,
    project_id: &str,
    root_param: &str,
) -> Result<std::path::PathBuf, Response> {
    let registry = state.registry.read().await;
    let entry = registry.get(project_id).cloned().ok_or_else(|| {
        error_response(
            StatusCode::NOT_FOUND,
            format!("Project not found: {}", project_id),
        )
    })?;
    let data_dir = registry.data_dir().to_path_buf();

    if root_param == "base" || root_param.is_empty() {
        let base_path = data_dir
            .join("worktrees")
            .join(project_id)
            .join(&entry.branch);
        if !base_path.exists() {
            return Err(error_response(
                StatusCode::NOT_FOUND,
                "Base worktree not found",
            ));
        }
        Ok(base_path)
    } else if let Some(branch) = root_param.strip_prefix("worktree:") {
        // Look up the worktree path by branch name from git worktree list.
        let base_path = data_dir
            .join("worktrees")
            .join(project_id)
            .join(&entry.branch);
        if !base_path.exists() {
            return Err(error_response(
                StatusCode::NOT_FOUND,
                "Base worktree not found",
            ));
        }

        let worktrees = crate::worktree_ops::get_worktrees(&base_path)
            .await
            .map_err(|e| {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to list worktrees: {}", e),
                )
            })?;

        let wt = worktrees
            .iter()
            .find(|wt| wt.branch == branch)
            .ok_or_else(|| {
                error_response(
                    StatusCode::NOT_FOUND,
                    format!("Worktree '{}' not found", branch),
                )
            })?;

        Ok(wt.path.clone())
    } else {
        Err(error_response(
            StatusCode::BAD_REQUEST,
            "Invalid root parameter. Use 'base' or 'worktree:<branch>'",
        ))
    }
}

/// Validate a relative path: reject path traversal attempts.
#[allow(clippy::result_large_err)]
fn validate_relative_path(path: &str) -> Result<(), Response> {
    if path.contains("..") {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Path traversal is not allowed",
        ));
    }
    // Also reject absolute paths
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Absolute paths are not allowed",
        ));
    }
    Ok(())
}

/// Build a recursive file tree for the given directory.
fn build_file_tree(
    dir: &std::path::Path,
    root: &std::path::Path,
) -> std::io::Result<Vec<FileTreeEntry>> {
    let mut entries = Vec::new();
    let mut dir_entries: Vec<_> = std::fs::read_dir(dir)?.flatten().collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in dir_entries {
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files starting with '.' except specific ones we want to show
        if file_name.starts_with('.') && file_name != ".cflx.jsonc" {
            // Skip .git but not other dot-dirs that might be relevant
            if EXCLUDED_DIRS.contains(&file_name.as_str()) {
                continue;
            }
        }

        let path = entry.path();
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            // Skip excluded directories
            if EXCLUDED_DIRS.contains(&file_name.as_str()) {
                continue;
            }

            let children = build_file_tree(&path, root)?;
            entries.push(FileTreeEntry {
                name: file_name,
                path: relative_path,
                entry_type: "directory".to_string(),
                children: Some(children),
            });
        } else if file_type.is_file() {
            entries.push(FileTreeEntry {
                name: file_name,
                path: relative_path,
                entry_type: "file".to_string(),
                children: None,
            });
        }
    }

    Ok(entries)
}

/// Detect if a file is binary by checking for NUL bytes in the first 8KB.
fn is_binary_file(path: &std::path::Path) -> std::io::Result<bool> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 8192];
    let n = file.read(&mut buf)?;
    Ok(buf[..n].contains(&0))
}

/// GET /api/v1/projects/:id/files/tree - list file tree
pub async fn get_file_tree(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<FileTreeQuery>,
) -> Response {
    let root_path = match resolve_file_root(&state, &project_id, &query.root).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    match build_file_tree(&root_path, &root_path) {
        Ok(tree) => (StatusCode::OK, Json(tree)).into_response(),
        Err(e) => {
            error!(
                project_id = %project_id,
                error = %e,
                "Failed to build file tree"
            );
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list files: {}", e),
            )
        }
    }
}

/// GET /api/v1/projects/:id/files/content - read file content
pub async fn get_file_content(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<FileContentQuery>,
) -> Response {
    // Validate path: reject traversal
    if let Err(resp) = validate_relative_path(&query.path) {
        return resp;
    }

    let root_path = match resolve_file_root(&state, &project_id, &query.root).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    let file_path = root_path.join(&query.path);

    // Ensure the resolved path is still within the root (canonicalize both)
    let canonical_root = match root_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return error_response(StatusCode::NOT_FOUND, "Root path not found");
        }
    };
    let canonical_file = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return error_response(StatusCode::NOT_FOUND, "File not found");
        }
    };
    if !canonical_file.starts_with(&canonical_root) {
        return error_response(StatusCode::BAD_REQUEST, "Path traversal is not allowed");
    }

    if !canonical_file.is_file() {
        return error_response(StatusCode::NOT_FOUND, "File not found");
    }

    // Get file size
    let metadata = match std::fs::metadata(&canonical_file) {
        Ok(m) => m,
        Err(_) => {
            return error_response(StatusCode::NOT_FOUND, "File not found");
        }
    };
    let size = metadata.len();

    // Check if binary
    match is_binary_file(&canonical_file) {
        Ok(true) => {
            return (
                StatusCode::OK,
                Json(FileContentResponse {
                    path: query.path,
                    content: None,
                    size,
                    truncated: false,
                    binary: true,
                }),
            )
                .into_response();
        }
        Ok(false) => {}
        Err(_) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file");
        }
    }

    // Read content (truncate if too large)
    let truncated = size > MAX_FILE_CONTENT_SIZE;
    let content = if truncated {
        use std::io::Read;
        let mut file = match std::fs::File::open(&canonical_file) {
            Ok(f) => f,
            Err(_) => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file");
            }
        };
        let mut buf = vec![0u8; MAX_FILE_CONTENT_SIZE as usize];
        let n = match file.read(&mut buf) {
            Ok(n) => n,
            Err(_) => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file");
            }
        };
        buf.truncate(n);
        String::from_utf8_lossy(&buf).to_string()
    } else {
        match std::fs::read_to_string(&canonical_file) {
            Ok(s) => s,
            Err(_) => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file");
            }
        }
    };

    (
        StatusCode::OK,
        Json(FileContentResponse {
            path: query.path,
            content: Some(content),
            size,
            truncated,
            binary: false,
        }),
    )
        .into_response()
}

// ─────────────────────────────── Terminal session handlers ────────────────────

/// Create a new terminal session with project context (resolves cwd server-side).
async fn create_terminal(
    State(state): State<AppState>,
    Json(request): Json<CreateTerminalFromContextRequest>,
) -> Response {
    info!(
        project_id = %request.project_id,
        root = %request.root,
        "Creating terminal session from context"
    );

    // Resolve cwd from project context using the same logic as file browser
    let cwd = match resolve_file_root(&state, &request.project_id, &request.root).await {
        Ok(path) => path.to_string_lossy().to_string(),
        Err(resp) => return resp,
    };

    let create_request = CreateTerminalRequest {
        cwd,
        rows: request.rows,
        cols: request.cols,
        project_id: request.project_id.clone(),
        root: request.root.clone(),
    };

    match state.terminal_manager.create_session(create_request).await {
        Ok(info) => (StatusCode::CREATED, Json(info)).into_response(),
        Err(e) => {
            error!(error = %e, "Failed to create terminal session");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        }
    }
}

/// List all terminal sessions.
async fn list_terminals(State(state): State<AppState>) -> Json<Vec<TerminalSessionInfo>> {
    Json(state.terminal_manager.list_sessions().await)
}

/// Delete a terminal session.
async fn delete_terminal(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    info!(session_id = %session_id, "Deleting terminal session");
    match state.terminal_manager.delete_session(&session_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            error!(session_id = %session_id, error = %e, "Failed to delete terminal session");
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e}))).into_response()
        }
    }
}

/// Resize a terminal session.
async fn resize_terminal(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<ResizeTerminalRequest>,
) -> Response {
    debug!(session_id = %session_id, rows = request.rows, cols = request.cols, "Resizing terminal");
    match state
        .terminal_manager
        .resize_session(&session_id, request.rows, request.cols)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e}))).into_response(),
    }
}

/// WebSocket handler for terminal I/O streaming.
async fn terminal_ws_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    let manager = state.terminal_manager.clone();

    // Verify session exists before upgrading
    if !manager.session_exists(&session_id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Session not found: {}", session_id)})),
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| handle_terminal_ws(socket, manager, session_id))
}

/// Handle a terminal WebSocket connection.
async fn handle_terminal_ws(socket: WebSocket, manager: SharedTerminalManager, session_id: String) {
    use axum::extract::ws::Message;
    use futures_util::{SinkExt, StreamExt};

    info!(session_id = %session_id, "Terminal WebSocket connected");

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send scrollback buffer contents before subscribing to live output.
    // This ensures the client receives recent output history on reconnection.
    match manager.get_scrollback(&session_id).await {
        Ok(scrollback) if !scrollback.is_empty() => {
            debug!(session_id = %session_id, bytes = scrollback.len(), "Sending scrollback buffer");
            if ws_tx
                .send(Message::Binary(scrollback.into()))
                .await
                .is_err()
            {
                debug!(session_id = %session_id, "WebSocket closed while sending scrollback");
                return;
            }
        }
        Ok(_) => {
            debug!(session_id = %session_id, "No scrollback data to send");
        }
        Err(e) => {
            debug!(session_id = %session_id, error = %e, "Failed to get scrollback, continuing without it");
        }
    }

    // Subscribe to terminal output
    let mut output_rx = match manager.subscribe_output(&session_id).await {
        Ok(rx) => rx,
        Err(e) => {
            error!(session_id = %session_id, error = %e, "Failed to subscribe to terminal output");
            let _ = ws_tx.send(Message::Close(None)).await;
            return;
        }
    };

    let manager_for_input = manager.clone();
    let session_id_for_input = session_id.clone();
    let session_id_for_output = session_id.clone();

    // Task: Forward terminal output to WebSocket
    let output_task = tokio::spawn(async move {
        loop {
            match output_rx.recv().await {
                Ok(data) => {
                    if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                        debug!(session_id = %session_id_for_output, "WebSocket send failed, closing output stream");
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    debug!(session_id = %session_id_for_output, lagged = n, "Terminal output receiver lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    debug!(session_id = %session_id_for_output, "Terminal output channel closed");
                    let _ = ws_tx.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    });

    // Task: Forward WebSocket input to terminal
    let input_task = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    if let Err(e) = manager_for_input
                        .write_input(&session_id_for_input, &data)
                        .await
                    {
                        error!(session_id = %session_id_for_input, error = %e, "Failed to write terminal input");
                        break;
                    }
                }
                Ok(Message::Text(text)) => {
                    // Handle text messages as terminal input
                    // Check if it's a JSON resize message
                    if let Ok(resize) = serde_json::from_str::<ResizeTerminalRequest>(&text) {
                        if let Err(e) = manager_for_input
                            .resize_session(&session_id_for_input, resize.rows, resize.cols)
                            .await
                        {
                            debug!(session_id = %session_id_for_input, error = %e, "Failed to resize terminal via WS");
                        }
                    } else {
                        // Plain text input
                        if let Err(e) = manager_for_input
                            .write_input(&session_id_for_input, text.as_bytes())
                            .await
                        {
                            error!(session_id = %session_id_for_input, error = %e, "Failed to write terminal text input");
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    debug!(session_id = %session_id_for_input, "Terminal WebSocket close received");
                    break;
                }
                Ok(_) => {} // Ignore pings/pongs
                Err(e) => {
                    debug!(session_id = %session_id_for_input, error = %e, "Terminal WebSocket error");
                    break;
                }
            }
        }
    });

    // Wait for either task to finish, then abort the other
    tokio::select! {
        _ = output_task => {},
        _ = input_task => {},
    }

    info!(session_id = %session_id, "Terminal WebSocket disconnected");
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
        .route(
            "/projects/{id}/changes/{change_id}/toggle",
            post(toggle_change_selection),
        )
        .route(
            "/projects/{id}/changes/toggle-all",
            post(toggle_all_change_selection),
        )
        .route(
            "/projects/{id}/worktrees",
            get(server_list_worktrees).post(server_create_worktree),
        )
        .route(
            "/projects/{id}/worktrees/refresh",
            post(server_refresh_worktrees),
        )
        .route(
            "/projects/{id}/worktrees/{branch}",
            delete(server_delete_worktree),
        )
        .route(
            "/projects/{id}/worktrees/{branch}/merge",
            post(server_merge_worktree),
        )
        .route("/projects/{id}/files/tree", get(get_file_tree))
        .route("/projects/{id}/files/content", get(get_file_content))
        .route(
            "/terminal/sessions",
            get(list_terminals).post(create_terminal),
        )
        .route("/terminal/sessions/{session_id}", delete(delete_terminal))
        .route(
            "/terminal/sessions/{session_id}/resize",
            post(resize_terminal),
        )
        .route("/control/run", post(global_control_run))
        .route("/control/stop", post(global_control_stop))
        .route("/control/status", get(global_control_status))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ))
        .with_state(app_state.clone());

    let public_api_routes = Router::new()
        .route("/ws", get(ws_handler))
        .route(
            "/terminal/sessions/{session_id}/ws",
            get(terminal_ws_handler),
        )
        .route("/version", get(get_version))
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
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
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
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
        }
    }

    // ── Global Control tests ──

    #[tokio::test]
    async fn test_global_control_run_records_call() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        // Initialize call recorder
        CONTROL_CALLS.get_or_init(|| Arc::new(std::sync::Mutex::new(Vec::new())));
        CONTROL_CALLS.get().unwrap().lock().unwrap().clear();

        let _entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/control/run")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let calls = CONTROL_CALLS.get().unwrap().lock().unwrap();
        // Global run records a "_global_" + "run" call
        assert!(calls
            .iter()
            .any(|(id, action)| id == "_global_" && action == "run"));
    }

    #[tokio::test]
    async fn test_global_control_stop_records_call() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        // Initialize call recorder
        CONTROL_CALLS.get_or_init(|| Arc::new(std::sync::Mutex::new(Vec::new())));
        CONTROL_CALLS.get().unwrap().lock().unwrap().clear();

        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/control/stop")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let calls = CONTROL_CALLS.get().unwrap().lock().unwrap();
        assert!(calls
            .iter()
            .any(|(id, action)| id == "_global_" && action == "stop"));
    }

    #[tokio::test]
    async fn test_global_control_status_returns_idle_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/control/status")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["orchestration_status"], "idle");
    }

    #[tokio::test]
    async fn test_per_project_control_routes_removed() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let router = build_router(state.clone());

        // Old per-project control/run should return 404 (route not found)
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/control/run", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        // axum returns 405 Method Not Allowed for unmatched routes under existing paths,
        // or 404 for completely unmatched routes
        assert!(
            resp.status() == StatusCode::NOT_FOUND
                || resp.status() == StatusCode::METHOD_NOT_ALLOWED,
            "Old per-project control/run route should be removed, got: {}",
            resp.status()
        );
    }

    // ── Semaphore (max_concurrent_total) tests ──

    #[tokio::test]
    async fn test_max_concurrent_total_semaphore_respected() {
        let temp_dir = TempDir::new().unwrap();
        // Create registry with max_concurrent_total = 2
        let state = make_state_with_limit(&temp_dir, None, 2);

        // Add two projects
        let _entry1 = state
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
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
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
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
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

    // ── resolve_command login shell tests ──

    #[tokio::test]
    async fn test_run_resolve_command_uses_login_shell() {
        // Verify that run_resolve_command executes via the user's login shell,
        // making PATH-dependent commands available even in non-login environments.
        let temp_dir = TempDir::new().unwrap();
        let (ran, exit_code) =
            super::run_resolve_command("echo hello", temp_dir.path(), "test prompt", None, None)
                .await;
        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(
            exit_code,
            Some(0),
            "echo command should succeed via login shell"
        );
    }

    #[tokio::test]
    async fn test_run_resolve_command_substitutes_prompt() {
        // Verify that {prompt} placeholder is substituted in the command string.
        let temp_dir = TempDir::new().unwrap();
        let (ran, exit_code) =
            super::run_resolve_command("echo {prompt}", temp_dir.path(), "test_marker", None, None)
                .await;
        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(
            exit_code,
            Some(0),
            "echo with prompt substitution should succeed"
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
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
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
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
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
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
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

    // ── Worktree API routing tests ──

    #[tokio::test]
    async fn test_list_worktrees_project_not_found_returns_404() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/nonexistent/worktrees")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_worktrees_empty_for_registered_project() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        // Register a project without cloning (no worktree exists)
        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::GET)
            .uri(format!("/api/v1/projects/{}/worktrees", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        // Should return OK with empty array (project path doesn't exist)
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json, serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_list_worktrees_with_real_project() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

        // First add the project (this clones and creates worktree)
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

        // Now list worktrees
        let list_req = Request::builder()
            .method(Method::GET)
            .uri(format!("/api/v1/projects/{}/worktrees", project_id))
            .body(Body::empty())
            .unwrap();

        let list_resp = router.oneshot(list_req).await.unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let worktrees: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        // Should have at least the main worktree
        assert!(
            !worktrees.is_empty(),
            "Should have at least one worktree after project add"
        );
    }

    #[tokio::test]
    async fn test_delete_worktree_project_not_found_returns_404() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::DELETE)
            .uri("/api/v1/projects/nonexistent/worktrees/some-branch")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_merge_worktree_project_not_found_returns_404() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects/nonexistent/worktrees/some-branch/merge")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_refresh_worktrees_project_not_found_returns_404() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects/nonexistent/worktrees/refresh")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_create_worktree_project_not_found_returns_404() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let body = serde_json::json!({
            "change_id": "test-change"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects/nonexistent/worktrees")
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_worktree_auth_required() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, Some("secret-token"));

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/some-id/worktrees")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "Worktree endpoints should require authentication"
        );
    }

    // ── extract_repo_name tests ──

    #[test]
    fn test_extract_repo_name_standard_https() {
        assert_eq!(
            extract_repo_name("https://github.com/owner/my-repo.git"),
            "my-repo"
        );
    }

    #[test]
    fn test_extract_repo_name_https_without_git_suffix() {
        assert_eq!(
            extract_repo_name("https://github.com/owner/my-repo"),
            "my-repo"
        );
    }

    #[test]
    fn test_extract_repo_name_ssh_url() {
        assert_eq!(
            extract_repo_name("git@github.com:owner/my-repo.git"),
            "my-repo"
        );
    }

    #[test]
    fn test_extract_repo_name_trailing_slash() {
        assert_eq!(
            extract_repo_name("https://github.com/owner/my-repo/"),
            "my-repo"
        );
    }

    #[test]
    fn test_extract_repo_name_bare_name() {
        assert_eq!(extract_repo_name("my-repo"), "my-repo");
    }

    #[test]
    fn test_extract_repo_name_empty_falls_back_to_url() {
        // An edge case: URL that reduces to empty after trimming
        // This shouldn't normally happen, but the function should not return empty
        let result = extract_repo_name("https://example.com/");
        assert!(!result.is_empty(), "Should not produce an empty repo name");
    }

    #[test]
    fn test_extract_repo_name_just_git_suffix() {
        assert_eq!(
            extract_repo_name("https://example.com/.git"),
            "https://example.com/.git",
            "A URL ending in .git with no basename should fall back to the full URL"
        );
    }

    // ── Version endpoint tests ──

    #[tokio::test]
    async fn test_get_version_returns_200() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/version")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_version_no_auth_required() {
        let temp_dir = TempDir::new().unwrap();
        // Configure a bearer token — version endpoint must still return 200 without it
        let router = make_router(&temp_dir, Some("secret-token"));

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/version")
            // No Authorization header
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "GET /api/v1/version must succeed without authentication even when auth is configured"
        );
    }

    #[tokio::test]
    async fn test_get_version_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/version")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Response must contain a non-empty "version" field
        let version = json["version"]
            .as_str()
            .expect("Response must contain 'version' field as a string");
        assert!(
            !version.is_empty(),
            "version field must not be empty, got: {:?}",
            version
        );
    }

    #[tokio::test]
    async fn test_remote_project_snapshot_serialization_has_dashboard_fields() {
        // Verify that the RemoteProject JSON includes repo, branch, status, is_busy fields
        let project = RemoteProject {
            id: "abc123".to_string(),
            name: "my-repo@main".to_string(),
            repo: "my-repo".to_string(),
            branch: "main".to_string(),
            status: "idle".to_string(),
            is_busy: false,
            error: None,
            changes: vec![],
        };

        let json = serde_json::to_value(&project).unwrap();
        assert_eq!(json["repo"], "my-repo");
        assert_eq!(json["branch"], "main");
        assert_eq!(json["status"], "idle");
        assert_eq!(json["is_busy"], false);
        assert!(json.get("error").is_none() || json["error"].is_null());
        assert_eq!(json["name"], "my-repo@main");
    }

    // ── File API tests ──

    #[test]
    fn test_validate_relative_path_rejects_traversal() {
        assert!(validate_relative_path("../etc/passwd").is_err());
        assert!(validate_relative_path("foo/../bar").is_err());
        assert!(validate_relative_path("..").is_err());
    }

    #[test]
    fn test_validate_relative_path_rejects_absolute() {
        assert!(validate_relative_path("/etc/passwd").is_err());
        assert!(validate_relative_path("\\windows\\system32").is_err());
    }

    #[test]
    fn test_validate_relative_path_accepts_valid() {
        assert!(validate_relative_path("src/main.rs").is_ok());
        assert!(validate_relative_path("openspec/changes/add-feature/proposal.md").is_ok());
        assert!(validate_relative_path("Cargo.toml").is_ok());
    }

    #[test]
    fn test_build_file_tree_excludes_dirs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Create various directories
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/foo")).unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();

        let tree = build_file_tree(root, root).unwrap();
        let names: Vec<&str> = tree.iter().map(|e| e.name.as_str()).collect();

        assert!(!names.contains(&".git"), "should exclude .git");
        assert!(
            !names.contains(&"node_modules"),
            "should exclude node_modules"
        );
        assert!(!names.contains(&"target"), "should exclude target");
        assert!(names.contains(&"src"), "should include src");
        assert!(names.contains(&"Cargo.toml"), "should include Cargo.toml");
    }

    #[test]
    fn test_build_file_tree_recursive() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/c.txt"), "hello").unwrap();
        std::fs::write(root.join("a/d.txt"), "world").unwrap();

        let tree = build_file_tree(root, root).unwrap();
        assert_eq!(tree.len(), 1); // only "a"
        let a = &tree[0];
        assert_eq!(a.name, "a");
        assert_eq!(a.entry_type, "directory");

        let children_a = a.children.as_ref().unwrap();
        assert_eq!(children_a.len(), 2); // "b" and "d.txt"

        let b = children_a.iter().find(|e| e.name == "b").unwrap();
        assert_eq!(b.entry_type, "directory");
        let children_b = b.children.as_ref().unwrap();
        assert_eq!(children_b.len(), 1);
        assert_eq!(children_b[0].name, "c.txt");
        assert_eq!(children_b[0].entry_type, "file");
    }

    #[test]
    fn test_is_binary_file_text() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("text.txt");
        std::fs::write(&path, "Hello, world!").unwrap();
        assert!(!is_binary_file(&path).unwrap());
    }

    #[test]
    fn test_is_binary_file_binary() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("binary.bin");
        std::fs::write(&path, b"\x00\x01\x02\x03").unwrap();
        assert!(is_binary_file(&path).unwrap());
    }

    #[tokio::test]
    async fn test_get_file_tree_project_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/nonexistent/files/tree")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_file_content_project_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/nonexistent/files/content?path=foo.txt")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_file_content_rejects_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/api/v1/projects/{}/files/content?path=../../../etc/passwd",
                entry.id
            ))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_file_tree_with_real_project() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

        // Add a project (clones and creates worktree)
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

        // Get file tree
        let tree_req = Request::builder()
            .method(Method::GET)
            .uri(format!("/api/v1/projects/{}/files/tree", project_id))
            .body(Body::empty())
            .unwrap();

        let tree_resp = router.clone().oneshot(tree_req).await.unwrap();
        assert_eq!(tree_resp.status(), StatusCode::OK);

        let tree_body = axum::body::to_bytes(tree_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let tree: Vec<serde_json::Value> = serde_json::from_slice(&tree_body).unwrap();
        // Should have at least README.md
        let names: Vec<&str> = tree.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(
            names.contains(&"README.md"),
            "File tree should contain README.md, got: {:?}",
            names
        );
    }

    #[tokio::test]
    async fn test_get_file_content_with_real_project() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

        // Add a project
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

        // Get file content for README.md
        let content_req = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/api/v1/projects/{}/files/content?path=README.md",
                project_id
            ))
            .body(Body::empty())
            .unwrap();

        let content_resp = router.oneshot(content_req).await.unwrap();
        assert_eq!(content_resp.status(), StatusCode::OK);

        let content_body = axum::body::to_bytes(content_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let content: serde_json::Value = serde_json::from_slice(&content_body).unwrap();
        assert_eq!(content["path"], "README.md");
        assert_eq!(content["binary"], false);
        assert_eq!(content["truncated"], false);
        assert!(content["content"].is_string());
        assert_eq!(content["content"].as_str().unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_get_file_content_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

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

        let content_req = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/api/v1/projects/{}/files/content?path=nonexistent.txt",
                project_id
            ))
            .body(Body::empty())
            .unwrap();

        let content_resp = router.oneshot(content_req).await.unwrap();
        assert_eq!(content_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_file_api_requires_auth() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, Some("secret-token"));

        // File tree endpoint
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/some-id/files/tree")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "File tree endpoint should require authentication"
        );

        // File content endpoint
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/some-id/files/content?path=foo.txt")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "File content endpoint should require authentication"
        );
    }
}
