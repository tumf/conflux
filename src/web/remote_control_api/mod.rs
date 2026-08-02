//! Single-instance remote-control API (`/api/v2`).
//!
//! This is a versioned contract for controlling *one running cflx process*
//! remotely: discover what it accepts, take a coherent snapshot, submit a
//! command from a closed set with optimistic concurrency and idempotency, and
//! follow an ordered, resumable event stream.
//!
//! It is deliberately separate from the legacy single-instance `/api/*` routes
//! and `/ws`, which the browser monitoring UI still uses and which keep working
//! unchanged. `/api/v2` is the only versioned remote-control namespace; the
//! multi-project `/api/v1` namespace was removed with server mode.
//!
//! Everything v2 tracks — `instance_id`, `state_revision`, `event_sequence`, the
//! command registry — is scoped to one process incarnation and is gone after a
//! restart. That is a feature: nothing here can become durable workflow state.

pub mod auth;
pub mod commands;
pub mod dto;
pub mod executor;
pub mod projection;
pub mod reads;
pub mod registry;
pub mod stream;
pub mod worktrees;

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;

use async_trait::async_trait;
use utoipa_swagger_ui::SwaggerUi;

use auth::{
    cors_headers, is_preflight, preflight_response, resolve_correlation_id, CorrelationId,
    RemoteControlAuth,
};
use dto::{CommandSpec, ErrorCode};
use executor::{CommandFailure, ExecutionSummary, RemoteControlExecutor};
use projection::Projection;
use worktrees::{UnboundWorktreeOperations, WorktreeListing, WorktreeOperations};

/// The only v2 path that is served without authentication.
pub const HEALTH_PATH: &str = "/api/v2/health";

async fn openapi_yaml() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/yaml")],
        serde_yaml::to_string(&crate::web::openapi::document())
            .expect("OpenAPI document must serialize as YAML"),
    )
}

/// The projection owner plus a late-bound delegation target.
///
/// The web server can start before an orchestration runtime exists (that is
/// already true for the legacy control channel), so the projection is created
/// eagerly — it is what gives the process its `instance_id` — while the executor
/// is bound once the shared operator command service is available. Until then,
/// commands are refused rather than queued: a controller must not be told a
/// command was accepted by a process that cannot act on it.
pub struct RemoteControlRuntime {
    projection: Arc<Projection>,
    executor: tokio::sync::RwLock<Option<Arc<dyn RemoteControlExecutor>>>,
    /// Worktree reads are bound separately from commands because the read routes
    /// are mounted before any orchestration runtime exists.
    worktrees: tokio::sync::RwLock<Option<Arc<dyn WorktreeOperations>>>,
}

impl Default for RemoteControlRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteControlRuntime {
    /// Start a new incarnation with no executor bound yet.
    pub fn new() -> Self {
        Self {
            projection: Arc::new(Projection::new()),
            executor: tokio::sync::RwLock::new(None),
            worktrees: tokio::sync::RwLock::new(None),
        }
    }

    /// The projection owner for this incarnation.
    pub fn projection(&self) -> Arc<Projection> {
        self.projection.clone()
    }

    /// Bind the delegation target once an orchestration runtime exists.
    pub async fn bind(&self, executor: Arc<dyn RemoteControlExecutor>) {
        *self.executor.write().await = Some(executor);
    }

    /// Bind the worktree port once a repository-backed service exists.
    pub async fn bind_worktrees(&self, worktrees: Arc<dyn WorktreeOperations>) {
        *self.worktrees.write().await = Some(worktrees);
    }

    /// True once commands can actually be delegated.
    // Observed by tests; the binary crate recompiles this tree and sees no caller.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn is_bound(&self) -> bool {
        self.executor.read().await.is_some()
    }
}

#[async_trait]
impl RemoteControlExecutor for RemoteControlRuntime {
    async fn execute(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure> {
        let bound = self.executor.read().await.clone();
        match bound {
            Some(executor) => executor.execute(command).await,
            None => Err(CommandFailure::new(
                ErrorCode::LifecycleConflict,
                "this instance has no orchestration runtime bound yet",
            )),
        }
    }
}

/// Delegate worktree reads to the bound port, refusing before one exists.
#[async_trait]
impl WorktreeOperations for RemoteControlRuntime {
    async fn list(&self) -> Result<WorktreeListing, CommandFailure> {
        match self.worktrees.read().await.clone() {
            Some(port) => port.list().await,
            None => UnboundWorktreeOperations.list().await,
        }
    }

    async fn create(&self, change_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        match self.worktrees.read().await.clone() {
            Some(port) => port.create(change_id).await,
            None => UnboundWorktreeOperations.create(change_id).await,
        }
    }

    async fn delete(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        match self.worktrees.read().await.clone() {
            Some(port) => port.delete(worktree_id).await,
            None => UnboundWorktreeOperations.delete(worktree_id).await,
        }
    }

    async fn merge(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        match self.worktrees.read().await.clone() {
            Some(port) => port.merge(worktree_id).await,
            None => UnboundWorktreeOperations.merge(worktree_id).await,
        }
    }
}

/// Shared state for the v2 router.
#[derive(Clone)]
pub struct RemoteControlState {
    /// The single projection owner.
    pub projection: Arc<Projection>,
    /// Bearer and origin policy.
    pub auth: Arc<RemoteControlAuth>,
    /// Delegation target for admitted commands.
    pub executor: Arc<dyn RemoteControlExecutor>,
    /// Worktree reads. Defaults to the unbound port, which refuses.
    pub worktrees: Arc<dyn WorktreeOperations>,
}

impl RemoteControlState {
    /// Assemble the router state.
    pub fn new(
        projection: Arc<Projection>,
        auth: Arc<RemoteControlAuth>,
        executor: Arc<dyn RemoteControlExecutor>,
    ) -> Self {
        Self {
            projection,
            auth,
            executor,
            worktrees: Arc::new(UnboundWorktreeOperations),
        }
    }

    /// Attach the worktree read port.
    pub fn with_worktrees(mut self, worktrees: Arc<dyn WorktreeOperations>) -> Self {
        self.worktrees = worktrees;
        self
    }
}

/// Build the `/api/v2` router.
///
/// Mounted only by single-instance web monitoring. Server-mode project routing
/// does not merge it: the two namespaces describe different things and sharing
/// them would make `instance_id` meaningless.
pub fn router(state: RemoteControlState) -> Router {
    let protected = Router::new()
        .route(HEALTH_PATH, get(reads::health))
        .route("/api/v2/capabilities", get(reads::capabilities))
        .route("/api/v2/instance", get(reads::instance))
        .route("/api/v2/state", get(reads::state))
        .route("/api/v2/changes", get(reads::list_changes))
        .route("/api/v2/changes/{change_id}", get(reads::get_change))
        .route("/api/v2/logs", get(reads::logs))
        .route("/api/v2/worktrees", get(reads::list_worktrees))
        .route("/api/v2/worktrees/{worktree_id}", get(reads::get_worktree))
        .route("/api/v2/commands", post(commands::submit_command))
        .route("/api/v2/commands/{command_id}", get(commands::get_command))
        .route("/api/v2/events", get(stream::events))
        .route("/api/v2/ws", get(stream::ws))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), gate))
        .with_state(state);

    protected
        .route("/api/v2/openapi.yaml", get(openapi_yaml))
        .merge(
            SwaggerUi::new("/api/v2/docs")
                .url("/api/v2/openapi.json", crate::web::openapi::document()),
        )
}

/// Origin, credential-transport, and bearer enforcement for every v2 request.
///
/// Runs as one gate rather than three layers so the ordering is visible: an
/// unacceptable origin is refused before credentials are even looked at, and a
/// preflight never reaches a handler.
async fn gate(State(state): State<RemoteControlState>, request: Request, next: Next) -> Response {
    let correlation = match resolve_correlation_id(request.headers()) {
        Ok(id) => id,
        Err(error) => return error.into_response(),
    };

    let allowed_origin = match state.auth.check_origin(request.headers(), &correlation) {
        Ok(origin) => origin,
        Err(error) => return error.into_response(),
    };

    if is_preflight(request.method()) {
        return preflight_response(allowed_origin.as_deref());
    }

    // Credentials never travel outside the Authorization header on v2, on any
    // path — including the unauthenticated health probe, so a client cannot
    // learn the habit on one route and leak a token on another.
    if let Err(error) = state.auth.reject_out_of_band_credentials(
        request.uri().query(),
        request.headers(),
        &correlation,
    ) {
        return error.into_response();
    }

    if request.uri().path() != HEALTH_PATH {
        if let Err(error) = state.auth.check_bearer(request.headers(), &correlation) {
            return error.into_response();
        }
    }

    let mut request = request;
    request
        .extensions_mut()
        .insert(CorrelationId(correlation.clone()));

    let mut response = next.run(request).await;
    for (name, value) in cors_headers(allowed_origin.as_deref()) {
        response.headers_mut().insert(name, value);
    }
    if let Ok(value) = axum::http::HeaderValue::from_str(&correlation) {
        response.headers_mut().insert("x-correlation-id", value);
    }
    response
}

#[cfg(test)]
mod tests;
