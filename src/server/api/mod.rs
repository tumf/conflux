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

#[cfg(test)]
use control::CONTROL_CALLS;

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
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::server::api::test_support::{
        create_local_git_repo, create_local_git_repo_with_setup, make_router, make_state,
        run_sync_monitor_once_for_tests,
    };
    use crate::server::registry::create_shared_registry;

    fn make_router_with_db(temp_dir: &TempDir, auth_token: Option<&str>) -> Router {
        let mut state = make_state(temp_dir, auth_token);
        state.db = Some(crate::server::db::ServerDb::new(temp_dir.path()).unwrap());
        build_router(state)
    }

    async fn rev_parse(repo: &std::path::Path, rev: &str) -> Option<String> {
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", rev])
            .current_dir(repo)
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn append_commit_to_remote(
        remote_url: &str,
        branch: &str,
        file_name: &str,
        contents: &str,
    ) -> Option<String> {
        let work_dir = tempfile::TempDir::new().ok()?;
        let work_path = work_dir.path();

        let clone = tokio::process::Command::new("git")
            .args(["clone", remote_url, work_path.to_str()?])
            .status()
            .await
            .ok()?;
        if !clone.success() {
            return None;
        }

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

        let checkout = tokio::process::Command::new("git")
            .args(["checkout", branch])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !checkout.success() {
            return None;
        }

        std::fs::write(work_path.join(file_name), contents).ok()?;
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        let commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "advance remote"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !commit.success() {
            return None;
        }

        let push = tokio::process::Command::new("git")
            .args(["push", "origin", branch])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !push.success() {
            return None;
        }

        rev_parse(work_path, "HEAD").await
    }

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
    async fn test_get_proposal_session_messages_not_found_returns_404() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/proj-1/proposal-sessions/missing/messages")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
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
            db: None,
            auth_token: auth_token.map(|s| s.to_string()),
            max_concurrent_total: max_concurrent,
            resolve_command: None,
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        }
    }

    // ── Global Control tests ──

    #[tokio::test]
    async fn test_global_control_run_skips_unremarked_error_changes() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        CONTROL_CALLS.get_or_init(|| Arc::new(std::sync::Mutex::new(Vec::new())));
        CONTROL_CALLS.get().unwrap().lock().unwrap().clear();

        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let worktree_path = temp_dir
            .path()
            .join("worktrees")
            .join(&entry.id)
            .join(&entry.branch)
            .join("openspec/changes/fix-a");
        std::fs::create_dir_all(&worktree_path).unwrap();
        std::fs::write(worktree_path.join("proposal.md"), "# proposal\n").unwrap();

        {
            let mut registry = state.registry.write().await;
            registry.toggle_change_selected(&entry.id, "fix-a");
        }

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/control/run")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["started"], 0);
        assert_eq!(json["skipped"], 1);
    }

    #[tokio::test]
    async fn test_sync_monitor_is_non_invasive_and_never_runs_sync_or_resolve() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_string_lossy());

        let mut state = make_state(&temp_dir, None);
        state.resolve_command = Some("false".to_string());
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

        let add_body = axum::body::to_bytes(add_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let add_json: serde_json::Value = serde_json::from_slice(&add_body).unwrap();
        let project_id = add_json["id"].as_str().unwrap();

        let local_repo_path = temp_dir.path().join(project_id);
        let local_sha_before = rev_parse(&local_repo_path, "refs/heads/main")
            .await
            .unwrap();

        let _remote_sha_after = append_commit_to_remote(
            &format!("file://{}", origin.to_string_lossy()),
            "main",
            "remote-only.txt",
            "remote advance",
        )
        .await
        .unwrap();

        run_sync_monitor_once_for_tests(&state).await;

        let local_sha_after = rev_parse(&local_repo_path, "refs/heads/main")
            .await
            .unwrap();
        assert_eq!(
            local_sha_before, local_sha_after,
            "monitoring must not mutate local branch tip via git/sync"
        );

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/state")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let project = &json["projects"][0];

        assert_eq!(project["sync_state"], "behind");
        assert_eq!(project["ahead_count"], 0);
        assert!(project["behind_count"].as_u64().unwrap_or(0) > 0);
        assert_eq!(project["sync_required"], true);
    }

    #[tokio::test]
    async fn test_projects_state_preserves_selection_visibility_for_non_error_changes() {
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

        let router = build_router(state.clone());
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

        {
            let mut registry = state.registry.write().await;
            registry.toggle_change_selected(&entry.id, "fix-a");
        }

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/state")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["projects"][0]["changes"][0]["selected"], false);
    }

    #[tokio::test]
    async fn test_projects_state_reports_error_changes_as_unselected_until_remarked() {
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
        assert_eq!(json["projects"][0]["changes"][0]["status"], "error");
        assert_eq!(json["projects"][0]["changes"][0]["selected"], false);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!(
                "/api/v1/projects/{}/changes/fix-a/toggle",
                entry.id
            ))
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
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["projects"][0]["changes"][0]["selected"], true);
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
}
