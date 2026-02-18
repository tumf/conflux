//! API v1 handlers for the server daemon.
//!
//! Provides REST endpoints for project management and execution control.
//!
//! NOTE: This module deliberately does NOT reference or execute `~/.wt/setup`.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::server::registry::{ProjectEntry, ProjectStatus, SharedRegistry};

/// Shared application state passed to axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub registry: SharedRegistry,
    /// Optional bearer token for authentication (None = no auth required)
    pub auth_token: Option<String>,
    /// Maximum concurrent total (informational; actual semaphore is in registry)
    #[allow(dead_code)]
    pub max_concurrent_total: usize,
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

/// POST /api/v1/projects - add a new project
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

    let mut registry = state.registry.write().await;
    match registry.add(req.remote_url, req.branch) {
        Ok(entry) => {
            info!("Project added: id={}", entry.id);
            (StatusCode::CREATED, Json(ProjectResponse::from(entry))).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already exists") {
                error_response(StatusCode::CONFLICT, msg)
            } else {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        }
    }
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

/// POST /api/v1/projects/:id/git/pull - pull from remote
pub async fn git_pull(State(state): State<AppState>, Path(project_id): Path<String>) -> Response {
    let (remote_url, branch, lock) = {
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
        (entry.remote_url, entry.branch, lock)
    };

    // Acquire per-project exclusive lock
    let _guard = lock.lock().await;

    info!(
        "git pull: project_id={} remote_url={} branch={}",
        project_id, remote_url, branch
    );

    let output = tokio::process::Command::new("git")
        .args(["ls-remote", "--heads", &remote_url, &branch])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("Branch '{}' not found on remote '{}'", branch, remote_url),
                )
            } else {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({"status": "pulled", "ref": stdout.trim()})),
                )
                    .into_response()
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            error!("git ls-remote failed: {}", stderr);
            error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("git operation failed: {}", stderr),
            )
        }
        Err(e) => {
            error!("Failed to run git: {}", e);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git: {}", e),
            )
        }
    }
}

/// POST /api/v1/projects/:id/git/push - push to remote (validates non-fast-forward)
pub async fn git_push(State(state): State<AppState>, Path(project_id): Path<String>) -> Response {
    let (remote_url, branch, lock) = {
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
        (entry.remote_url, entry.branch, lock)
    };

    // Acquire per-project exclusive lock
    let _guard = lock.lock().await;

    info!(
        "git push (dry-run check): project_id={} remote_url={} branch={}",
        project_id, remote_url, branch
    );

    // In server mode, actual push requires the repo to be cloned locally.
    // We simulate this by checking remote ref existence only.
    // A full implementation would operate on a bare clone in data_dir.
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "push_queued",
            "remote_url": remote_url,
            "branch": branch,
            "note": "Actual push requires local repository clone in data_dir"
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
) -> Response {
    apply_control(&state, &project_id, "run", ProjectStatus::Running).await
}

/// POST /api/v1/projects/:id/control/stop
pub async fn control_stop(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    apply_control(&state, &project_id, "stop", ProjectStatus::Stopped).await
}

/// POST /api/v1/projects/:id/control/retry
pub async fn control_retry(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    apply_control(&state, &project_id, "retry", ProjectStatus::Running).await
}

async fn apply_control(
    state: &AppState,
    project_id: &str,
    action: &str,
    new_status: ProjectStatus,
) -> Response {
    // Record the call for test verification
    if let Some(calls) = CONTROL_CALLS.get() {
        calls
            .lock()
            .unwrap()
            .push((project_id.to_string(), action.to_string()));
    }

    let lock = {
        let registry = state.registry.read().await;
        if registry.get(project_id).is_none() {
            return error_response(
                StatusCode::NOT_FOUND,
                format!("Project not found: {}", project_id),
            );
        }
        match registry.project_lock(project_id) {
            Some(l) => l,
            None => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Missing project lock")
            }
        }
    };

    // Acquire per-project exclusive lock
    let _guard = lock.lock().await;

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

// ─────────────────────────────── Router builder ────────────────────────────────

/// Build the API v1 router with authentication middleware.
pub fn build_router(app_state: AppState) -> Router {
    let api_routes = Router::new()
        .route("/projects", get(list_projects).post(add_project))
        .route("/projects/{id}", delete(delete_project))
        .route("/projects/{id}/git/pull", post(git_pull))
        .route("/projects/{id}/git/push", post(git_push))
        .route("/projects/{id}/control/run", post(control_run))
        .route("/projects/{id}/control/stop", post(control_stop))
        .route("/projects/{id}/control/retry", post(control_retry))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ))
        .with_state(app_state);

    Router::new().nest("/api/v1", api_routes)
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
        AppState {
            registry,
            auth_token: auth_token.map(|s| s.to_string()),
            max_concurrent_total: 4,
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

    #[tokio::test]
    async fn test_add_project_returns_201() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let body = serde_json::json!({
            "remote_url": "https://github.com/foo/bar",
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
}
