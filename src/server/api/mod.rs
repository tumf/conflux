//! API v1 handlers for the server daemon.
//!
//! Provides REST endpoints for project management and execution control.
//!
//! NOTE: This module deliberately does NOT reference or execute `~/.wt/setup`.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{ws::Message, ws::WebSocket, ws::WebSocketUpgrade, Path, Query, State},
    http::{header, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::execution::state::{detect_workspace_state, WorkspaceState};
use crate::remote::types::{RemoteChange, RemoteLogEntry, RemoteProject, RemoteStateUpdate};
use crate::server::active_commands::{
    ActiveCommandGuard, RootKind, SharedActiveCommands, WorktreeRootKey,
};
use crate::server::db::ServerDb;
use crate::server::proposal_session::{
    ProposalSessionError, ProposalSessionMessageRecord, SharedProposalSessionManager,
};
use crate::server::registry::{
    server_worktree_branch, OrchestrationStatus, ProjectEntry, ProjectStatus, ProjectSyncMetadata,
    ProjectSyncState, SharedRegistry,
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

/// Poll interval for background remote sync-state monitoring.
const REMOTE_SYNC_MONITOR_INTERVAL: Duration = Duration::from_secs(30);

/// Shared application state passed to axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub registry: SharedRegistry,
    pub runners: SharedRunners,
    pub db: Option<Arc<ServerDb>>,
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
    /// Proposal session manager for interactive proposal creation
    pub proposal_session_manager: SharedProposalSessionManager,
}

mod control;
mod dashboard;
mod files;
mod git_sync;
mod helpers;
mod projects;
mod proposals;
mod terminals;
mod worktrees;
mod ws;

#[cfg(test)]
pub(crate) mod test_support;

use helpers::{
    error_response, now_rfc3339, ProjectsStateResponse, StatsOverviewResponse,
    StatsOverviewSummaryResponse, StatsProjectResponse, StatsRecentEventResponse,
};

use control::{
    get_logs, get_project_history, get_stats_overview, global_control_run, global_control_status,
    global_control_stop, toggle_all_change_selection, toggle_change_selection,
};
use dashboard::{dashboard_assets, dashboard_favicon, dashboard_icons, dashboard_index};
use files::{get_file_content, get_file_tree};
use git_sync::{git_pull, git_push, git_sync};
use projects::{add_project, delete_project, get_version, list_projects, projects_state};
use proposals::{
    close_proposal_session, create_proposal_session, get_proposal_session_messages,
    list_proposal_session_changes, list_proposal_sessions, merge_proposal_session,
    proposal_session_ws_handler,
};
use terminals::{
    create_terminal, delete_terminal, list_terminals, resize_terminal, terminal_ws_handler,
};
use worktrees::{
    server_create_worktree, server_delete_worktree, server_list_worktrees, server_merge_worktree,
    server_refresh_worktrees,
};
use ws::ws_handler;

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

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    #[serde(default = "default_history_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    #[serde(default = "default_logs_limit")]
    limit: usize,
    before: Option<String>,
    project_id: Option<String>,
}

fn default_history_limit() -> usize {
    100
}

fn default_logs_limit() -> usize {
    100
}

/// GET /api/v1/ui-state - return all persisted dashboard UI state.
async fn get_ui_state(State(state): State<AppState>) -> Response {
    match &state.db {
        Some(db) => match db.get_all_ui_state() {
            Ok(ui_state) => (StatusCode::OK, Json(serde_json::json!(ui_state))).into_response(),
            Err(e) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load ui-state: {}", e),
            ),
        },
        None => (StatusCode::OK, Json(serde_json::json!({}))).into_response(),
    }
}

/// PUT /api/v1/ui-state/{key} - upsert single UI state key/value.
async fn put_ui_state(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let value = match payload.get("value").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => {
            return error_response(StatusCode::BAD_REQUEST, "Missing string field 'value'");
        }
    };

    match &state.db {
        Some(db) => match db.set_ui_state(&key, value) {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save ui-state: {}", e),
            ),
        },
        None => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server database is not configured",
        ),
    }
}

/// DELETE /api/v1/ui-state/{key} - remove single UI state entry.
async fn delete_ui_state(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    match &state.db {
        Some(db) => match db.delete_ui_state(&key) {
            Ok(()) => (StatusCode::NO_CONTENT, Json(serde_json::Value::Null)).into_response(),
            Err(e) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete ui-state: {}", e),
            ),
        },
        None => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server database is not configured",
        ),
    }
}

fn sync_metadata_unknown(reason: impl Into<String>) -> ProjectSyncMetadata {
    ProjectSyncMetadata {
        sync_state: ProjectSyncState::Unknown,
        ahead_count: 0,
        behind_count: 0,
        sync_required: false,
        local_sha: None,
        remote_sha: None,
        last_remote_check_at: Some(now_rfc3339()),
        remote_check_error: Some(reason.into()),
    }
}

fn parse_remote_head_sha(ls_remote_stdout: &str) -> Option<String> {
    ls_remote_stdout
        .split_whitespace()
        .next()
        .map(|sha| sha.to_string())
}

fn parse_left_right_count(stdout: &str) -> Option<(u32, u32)> {
    let mut parts = stdout.split_whitespace();
    let ahead = parts.next()?.parse::<u32>().ok()?;
    let behind = parts.next()?.parse::<u32>().ok()?;
    Some((ahead, behind))
}

fn classify_sync_state(ahead_count: u32, behind_count: u32) -> ProjectSyncState {
    match (ahead_count, behind_count) {
        (0, 0) => ProjectSyncState::UpToDate,
        (a, 0) if a > 0 => ProjectSyncState::Ahead,
        (0, b) if b > 0 => ProjectSyncState::Behind,
        _ => ProjectSyncState::Diverged,
    }
}

async fn compute_project_sync_metadata(
    project_id: &str,
    remote_url: &str,
    branch: &str,
    local_repo_path: &std::path::Path,
) -> ProjectSyncMetadata {
    debug!(
        project_id,
        branch,
        local_repo = %local_repo_path.display(),
        "Starting remote sync-state check"
    );

    let ls_remote = tokio::process::Command::new("git")
        .args(["ls-remote", "--heads", remote_url, branch])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    let remote_sha = match ls_remote {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match parse_remote_head_sha(&stdout) {
                Some(sha) => sha,
                None => {
                    warn!(
                        project_id,
                        branch, "Remote branch head not found during sync-state check"
                    );
                    return sync_metadata_unknown(format!(
                        "Branch '{}' not found on remote '{}'",
                        branch, remote_url
                    ));
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            warn!(project_id, branch, error = %stderr, "git ls-remote failed during sync-state check");
            return sync_metadata_unknown(format!("git ls-remote failed: {}", stderr));
        }
        Err(error) => {
            warn!(project_id, branch, error = %error, "Failed to execute git ls-remote during sync-state check");
            return sync_metadata_unknown(format!("Failed to run git ls-remote: {}", error));
        }
    };

    let fetch_refspec = format!("refs/heads/{}:refs/remotes/origin/{}", branch, branch);
    let fetch_remote = tokio::process::Command::new("git")
        .args(["fetch", remote_url, &fetch_refspec])
        .current_dir(local_repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match fetch_remote {
        Ok(output) if output.status.success() => {
            debug!(
                project_id,
                branch, "Fetched remote head for sync-state computation"
            );
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            warn!(project_id, branch, error = %stderr, "git fetch failed during sync-state check");
            return sync_metadata_unknown(format!("git fetch failed: {}", stderr));
        }
        Err(error) => {
            warn!(project_id, branch, error = %error, "Failed to execute git fetch during sync-state check");
            return sync_metadata_unknown(format!("Failed to run git fetch: {}", error));
        }
    }

    let local_ref = format!("refs/heads/{}", branch);
    let local_rev = tokio::process::Command::new("git")
        .args(["rev-parse", &local_ref])
        .current_dir(local_repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    let local_sha = match local_rev {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            warn!(project_id, branch, error = %stderr, "git rev-parse failed during sync-state check");
            return sync_metadata_unknown(format!("git rev-parse failed: {}", stderr));
        }
        Err(error) => {
            warn!(project_id, branch, error = %error, "Failed to execute git rev-parse during sync-state check");
            return sync_metadata_unknown(format!("Failed to run git rev-parse: {}", error));
        }
    };

    let origin_ref = format!("refs/remotes/origin/{}", branch);
    let range = format!("{}...{}", local_ref, origin_ref);
    let rev_list = tokio::process::Command::new("git")
        .args(["rev-list", "--left-right", "--count", &range])
        .current_dir(local_repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    let (ahead_count, behind_count) = match rev_list {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match parse_left_right_count(&stdout) {
                Some(counts) => counts,
                None => {
                    warn!(project_id, branch, output = %stdout.trim(), "Unexpected rev-list count output during sync-state check");
                    return sync_metadata_unknown(
                        "Failed to parse git rev-list ahead/behind counts",
                    );
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            warn!(project_id, branch, error = %stderr, "git rev-list failed during sync-state check");
            return sync_metadata_unknown(format!("git rev-list failed: {}", stderr));
        }
        Err(error) => {
            warn!(project_id, branch, error = %error, "Failed to execute git rev-list during sync-state check");
            return sync_metadata_unknown(format!("Failed to run git rev-list: {}", error));
        }
    };

    let sync_state = classify_sync_state(ahead_count, behind_count);
    let sync_required = !matches!(sync_state, ProjectSyncState::UpToDate);
    let checked_at = now_rfc3339();

    info!(
        project_id,
        branch,
        sync_state = sync_state.as_str(),
        ahead_count,
        behind_count,
        "Completed remote sync-state check"
    );

    ProjectSyncMetadata {
        sync_state,
        ahead_count,
        behind_count,
        sync_required,
        local_sha: Some(local_sha),
        remote_sha: Some(remote_sha),
        last_remote_check_at: Some(checked_at),
        remote_check_error: None,
    }
}

async fn refresh_project_sync_states_once(registry: &SharedRegistry) {
    let (entries, data_dir) = {
        let reg = registry.read().await;
        (reg.list(), reg.data_dir().to_path_buf())
    };

    if entries.is_empty() {
        debug!("Skipping remote sync-state refresh because no projects are registered");
        return;
    }

    for entry in entries {
        let local_repo_path = data_dir.join(&entry.id);
        let metadata = compute_project_sync_metadata(
            &entry.id,
            &entry.remote_url,
            &entry.branch,
            &local_repo_path,
        )
        .await;

        let mut reg = registry.write().await;
        if let Err(error) = reg.set_sync_metadata(&entry.id, metadata) {
            warn!(
                project_id = %entry.id,
                error = %error,
                "Failed to persist sync metadata after refresh"
            );
        }
    }
}

/// Run periodic remote sync-state monitoring for all registered server projects.
pub async fn run_remote_sync_state_monitor(registry: SharedRegistry) {
    info!(
        interval_seconds = REMOTE_SYNC_MONITOR_INTERVAL.as_secs(),
        "Starting remote sync-state monitor loop"
    );
    let mut interval = tokio::time::interval(REMOTE_SYNC_MONITOR_INTERVAL);
    loop {
        interval.tick().await;
        refresh_project_sync_states_once(&registry).await;
    }
}

// POST /api/v1/projects - add a new project
//
// Performs the following steps atomically (with rollback on failure):
// 1. Register the project in the registry (persisted to disk).
// 2. Acquire the global semaphore and per-project lock.
// 3. Verify the branch exists on the remote (git ls-remote).
// 4. Clone the repository as a bare clone into `data_dir/<project_id>`.
// 5. Create a git worktree at `data_dir/worktrees/<project_id>/<branch>`.
//
// If any step after registry insertion fails, the project is removed from the
// registry so no inconsistent state is persisted.
// ─────────────────────────────── Router builder ────────────────────────────────

/// Build the API v1 router with authentication middleware.
pub fn build_router(app_state: AppState) -> Router {
    let authenticated_routes = Router::new()
        .route("/projects", get(list_projects).post(add_project))
        .route("/projects/state", get(projects_state))
        .route("/ui-state", get(get_ui_state))
        .route("/ui-state/{key}", put(put_ui_state).delete(delete_ui_state))
        .route("/stats/overview", get(get_stats_overview))
        .route("/stats/projects/{id}/history", get(get_project_history))
        .route("/logs", get(get_logs))
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
        .route(
            "/projects/{id}/proposal-sessions",
            get(list_proposal_sessions).post(create_proposal_session),
        )
        .route(
            "/projects/{id}/proposal-sessions/{session_id}",
            delete(close_proposal_session),
        )
        .route(
            "/projects/{id}/proposal-sessions/{session_id}/merge",
            post(merge_proposal_session),
        )
        .route(
            "/projects/{id}/proposal-sessions/{session_id}/changes",
            get(list_proposal_session_changes),
        )
        .route(
            "/projects/{id}/proposal-sessions/{session_id}/messages",
            get(get_proposal_session_messages),
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
        .route(
            "/proposal-sessions/{session_id}/ws",
            get(proposal_session_ws_handler),
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

    #[test]
    fn test_classify_sync_state_variants() {
        assert!(matches!(
            classify_sync_state(0, 0),
            ProjectSyncState::UpToDate
        ));
        assert!(matches!(classify_sync_state(2, 0), ProjectSyncState::Ahead));
        assert!(matches!(
            classify_sync_state(0, 3),
            ProjectSyncState::Behind
        ));
        assert!(matches!(
            classify_sync_state(4, 1),
            ProjectSyncState::Diverged
        ));
    }
}

