//! Repository-local tests for the `/api/v2` remote-control contract.
//!
//! Split by concern so a failure names the property that broke rather than just
//! "the API". Shared fixtures live here.

mod auth_tests;
mod command_tests;
mod compatibility_tests;
mod dto_tests;
mod projection_tests;
mod read_tests;
mod registry_tests;
mod stream_tests;
mod worktree_tests;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode};
use tower::ServiceExt;

use crate::web::remote_control_api::auth::RemoteControlAuth;
use crate::web::remote_control_api::dto::{
    ChangeResource, CommandSpec, InstanceSnapshot, SnapshotTotals,
};
use crate::web::remote_control_api::executor::{
    CommandFailure, ExecutionSummary, RemoteControlExecutor,
};
use crate::web::remote_control_api::projection::Projection;
use crate::web::remote_control_api::worktrees::WorktreeOperations;
use crate::web::remote_control_api::{router, RemoteControlState};

/// Executor double that records what the API delegated and answers on cue.
///
/// It exists to prove the *API's* obligations — that a side effect happens
/// exactly once, or not at all — without dragging a real orchestration runtime
/// into a unit test.
pub(crate) struct RecordingExecutor {
    calls: Mutex<Vec<CommandSpec>>,
    call_count: AtomicUsize,
    outcome: Mutex<Result<ExecutionSummary, CommandFailure>>,
    /// When set, `execute` blocks until the gate is released, which keeps a
    /// command in the in-progress state for capacity and 202 tests.
    gate: Mutex<Option<Arc<tokio::sync::Notify>>>,
    /// Worktree commands delegate here, mirroring production wiring where the
    /// executor and the read routes share one worktree port.
    worktrees: Mutex<Option<Arc<dyn WorktreeOperations>>>,
}

impl RecordingExecutor {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            call_count: AtomicUsize::new(0),
            outcome: Mutex::new(Ok(ExecutionSummary::changed("done"))),
            gate: Mutex::new(None),
            worktrees: Mutex::new(None),
        })
    }

    pub(crate) fn bind_worktrees(&self, port: Arc<dyn WorktreeOperations>) {
        *self.worktrees.lock().unwrap() = Some(port);
    }

    pub(crate) fn set_outcome(&self, outcome: Result<ExecutionSummary, CommandFailure>) {
        *self.outcome.lock().unwrap() = outcome;
    }

    /// Make every subsequent execution block until the returned gate is notified.
    pub(crate) fn block_until_released(&self) -> Arc<tokio::sync::Notify> {
        let gate = Arc::new(tokio::sync::Notify::new());
        *self.gate.lock().unwrap() = Some(gate.clone());
        gate
    }

    pub(crate) fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    pub(crate) fn calls(&self) -> Vec<CommandSpec> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl RemoteControlExecutor for RecordingExecutor {
    async fn execute(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure> {
        self.calls.lock().unwrap().push(command.clone());
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let gate = self.gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.notified().await;
        }

        let worktrees = self.worktrees.lock().unwrap().clone();
        if let Some(port) = worktrees {
            match command {
                CommandSpec::CreateWorktree { target, .. } => {
                    return port.create(&target.change_id).await
                }
                CommandSpec::DeleteWorktree { target, .. } => {
                    return port.delete(&target.worktree_id).await
                }
                CommandSpec::MergeWorktree { target, .. } => {
                    return port.merge(&target.worktree_id).await
                }
                _ => {}
            }
        }
        self.outcome.lock().unwrap().clone()
    }
}

/// A router plus the pieces a test needs to drive and inspect it.
pub(crate) struct Harness {
    pub(crate) router: axum::Router,
    pub(crate) projection: Arc<Projection>,
    pub(crate) executor: Arc<RecordingExecutor>,
    pub(crate) worktrees: Arc<worktree_tests::FakeWorktreePort>,
}

/// Build a harness with the given bearer token and exact allowed origins.
pub(crate) fn harness(token: Option<&str>, origins: &[&str]) -> Harness {
    harness_with_projection(Arc::new(Projection::new()), token, origins)
}

/// Build a harness over an existing projection (for registry-bound tests).
pub(crate) fn harness_with_projection(
    projection: Arc<Projection>,
    token: Option<&str>,
    origins: &[&str],
) -> Harness {
    let owned: Vec<String> = origins.iter().map(|o| o.to_string()).collect();
    let auth = RemoteControlAuth::new(token.map(str::to_string), &owned)
        .expect("test origins must be valid");
    let executor = RecordingExecutor::new();
    let worktrees = Arc::new(worktree_tests::FakeWorktreePort::default());
    executor.bind_worktrees(worktrees.clone());
    let router = router(
        RemoteControlState::new(projection.clone(), Arc::new(auth), executor.clone())
            .with_worktrees(worktrees.clone()),
    );
    Harness {
        router,
        projection,
        executor,
        worktrees,
    }
}

/// A snapshot with one change, used to move the revision deterministically.
pub(crate) fn snapshot_with(change_id: &str, display_status: &str) -> InstanceSnapshot {
    InstanceSnapshot {
        app_mode: "running".to_string(),
        is_resolving: false,
        changes: vec![ChangeResource {
            id: change_id.to_string(),
            display_status: display_status.to_string(),
            progress_status: "in_progress".to_string(),
            completed_tasks: 1,
            total_tasks: 3,
            progress_percent: 33.3,
            dependencies: Vec::new(),
            iteration_number: Some(1),
        }],
        totals: SnapshotTotals {
            total: 1,
            completed: 0,
            in_progress: 1,
            pending: 0,
        },
    }
}

/// Issue a request against a harness router.
pub(crate) async fn send(router: &axum::Router, request: Request<Body>) -> Response<Body> {
    router.clone().oneshot(request).await.unwrap()
}

/// GET with optional bearer credentials.
pub(crate) fn get(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("host", "127.0.0.1:8080");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).unwrap()
}

/// POST a JSON body with optional bearer credentials.
pub(crate) fn post_json(uri: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("host", "127.0.0.1:8080")
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

/// Decode a response body as JSON.
pub(crate) async fn json_body(response: Response<Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

/// Status plus decoded JSON body, the pair almost every test wants.
pub(crate) async fn status_and_json(response: Response<Body>) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    (status, json_body(response).await)
}
