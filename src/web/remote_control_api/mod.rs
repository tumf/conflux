//! Single-instance remote-control API (`/api/v2`).
//!
//! This is a versioned contract for controlling *one running cflx process*
//! remotely: discover what it accepts, take a coherent snapshot, submit a
//! command from a closed set with optimistic concurrency and idempotency, and
//! follow an ordered, resumable event stream.
//!
//! `/api/v2` is the *only* API namespace this process serves. The legacy
//! unversioned `/api/*` and `/ws` surface and the multi-project `/api/v1`
//! namespace are both gone: the embedded operator console is a v2 client now, and
//! a second unauthenticated contract would only be a way around v2's bearer
//! policy, revision checks, and typed errors.
//!
//! The contract itself is generated from [`crate::web::openapi`] and never
//! tracked as a file. A consumer reads it from this endpoint or exports it with
//! `cflx openapi`; both return the same generated document.
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

/// This build's canonical contract, byte-for-byte the same document that
/// `cflx openapi` writes to stdout.
///
/// Unauthenticated on purpose: it describes the API and reads no instance state,
/// so requiring a token here would only stop a client from discovering how to
/// present one.
#[utoipa::path(
    get,
    path = "/api/v2/openapi.yaml",
    tag = "contract",
    security(),
    responses((status = 200, description = "OpenAPI document (YAML), content type `application/yaml`"))
)]
pub async fn openapi_yaml() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/yaml")],
        crate::web::openapi::document_yaml(),
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
    /// The process-local application gate, bound with the executor.
    gate: Arc<CommandGate>,
    /// Execution facts and scheduler liveness, bound with the orchestration
    /// runtime that produces them.
    execution_facts: Arc<ExecutionFactsHandle>,
}

/// Late-bound handle to the shared execution-facts store and the scheduler
/// liveness authority the execution-status resource reads.
///
/// Bound together because they answer the two halves of one question: a live
/// scheduler with no admitted work and a dead scheduler with stale facts are
/// different situations, and a client must be able to tell them apart. Unbound
/// is a process that has observed no lifecycle work and has no scheduler, which
/// reports exactly that rather than inventing either half.
///
/// Both bindings sit behind synchronous locks: every operation is a clone or a
/// boolean read, so an async lock would only add an await point where a request
/// could be reordered against a binding.
#[derive(Default)]
pub struct ExecutionFactsHandle {
    facts:
        std::sync::RwLock<Option<Arc<crate::orchestration::execution_facts::ExecutionFactsStore>>>,
    boundary: std::sync::RwLock<
        Option<Arc<dyn crate::orchestration::operator_command::RunBoundaryLiveness>>,
    >,
}

impl ExecutionFactsHandle {
    /// Bind the shared store.
    pub fn bind(&self, facts: Arc<crate::orchestration::execution_facts::ExecutionFactsStore>) {
        *self
            .facts
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(facts);
    }

    /// Bind the scheduler-task liveness authority.
    pub fn bind_boundary(
        &self,
        boundary: Arc<dyn crate::orchestration::operator_command::RunBoundaryLiveness>,
    ) {
        *self
            .boundary
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(boundary);
    }

    /// A coherent read of the store, empty when nothing is bound.
    ///
    /// An unbound process has observed no lifecycle work, and the empty snapshot
    /// says exactly that: no phases, no episodes, no active work.
    pub fn snapshot(&self) -> crate::orchestration::execution_facts::ExecutionFactsSnapshot {
        match self
            .facts
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
        {
            Some(facts) => facts.snapshot(),
            None => Default::default(),
        }
    }

    /// Whether the scheduler task owning the current run state is alive.
    pub fn scheduler_running(&self) -> bool {
        match self
            .boundary
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
        {
            Some(boundary) => boundary.boundary_running(),
            None => false,
        }
    }
}

/// Late-bound handle to the process-local operator application gate.
///
/// The endpoint holds this gate from final admission through settlement, which
/// is the whole fix for the concurrency hole optimistic revisions had on their
/// own: admission was atomic for the *record*, but the effect that consumes the
/// revision landed after the lock was already released, so two new commands
/// could each pass the revision check.
///
/// Unbound until an orchestration runtime exists — a process that cannot execute
/// a command has no transaction to serialize.
#[derive(Default)]
pub struct CommandGate {
    inner: tokio::sync::RwLock<Option<Arc<tokio::sync::Mutex<()>>>>,
}

impl CommandGate {
    /// Bind the coordinator's gate.
    pub async fn bind(&self, gate: Arc<tokio::sync::Mutex<()>>) {
        *self.inner.write().await = Some(gate);
    }

    /// Hold the gate for one submission, if a coordinator is bound.
    pub async fn hold(&self) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        let gate = self.inner.read().await.clone()?;
        Some(gate.lock_owned().await)
    }
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
            gate: Arc::new(CommandGate::default()),
            execution_facts: Arc::new(ExecutionFactsHandle::default()),
        }
    }

    /// The projection owner for this incarnation.
    pub fn projection(&self) -> Arc<Projection> {
        self.projection.clone()
    }

    /// The application gate the command endpoint serializes submissions with.
    pub fn gate(&self) -> Arc<CommandGate> {
        self.gate.clone()
    }

    /// The execution-facts handle the status resource reads.
    pub fn execution_facts(&self) -> Arc<ExecutionFactsHandle> {
        self.execution_facts.clone()
    }

    /// Bind the shared execution-facts store once an orchestration runtime exists.
    pub fn bind_execution_facts(
        &self,
        facts: Arc<crate::orchestration::execution_facts::ExecutionFactsStore>,
    ) {
        self.execution_facts.bind(facts);
    }

    /// Bind the scheduler-task liveness authority the status resource reports.
    pub fn bind_run_boundary(
        &self,
        boundary: Arc<dyn crate::orchestration::operator_command::RunBoundaryLiveness>,
    ) {
        self.execution_facts.bind_boundary(boundary);
    }

    /// Bind the delegation target once an orchestration runtime exists.
    pub async fn bind(&self, executor: Arc<dyn RemoteControlExecutor>) {
        *self.executor.write().await = Some(executor);
    }

    /// Bind the process-local application gate once a coordinator exists.
    pub async fn bind_gate(&self, gate: Arc<tokio::sync::Mutex<()>>) {
        self.gate.bind(gate).await;
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
            None => Err(unbound_runtime()),
        }
    }

    async fn begin(
        &self,
        command: &CommandSpec,
        gate: Option<executor::GateGuard>,
    ) -> executor::Applied {
        let bound = self.executor.read().await.clone();
        match bound {
            Some(executor) => executor.begin(command, gate).await,
            None => executor::Applied::Settled(Err(unbound_runtime())),
        }
    }

    async fn execute_held(
        &self,
        command: &CommandSpec,
        gate: Option<executor::GateGuard>,
    ) -> Result<ExecutionSummary, CommandFailure> {
        let bound = self.executor.read().await.clone();
        match bound {
            Some(executor) => executor.execute_held(command, gate).await,
            None => Err(unbound_runtime()),
        }
    }
}

/// The refusal a process with no orchestration runtime returns.
///
/// A refusal rather than a queue: a controller must not be told a command was
/// accepted by a process that cannot act on it.
fn unbound_runtime() -> CommandFailure {
    CommandFailure::new(
        ErrorCode::LifecycleConflict,
        "this instance has no orchestration runtime bound yet",
    )
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
    /// The application gate one submission is serialized by.
    ///
    /// Defaults to unbound, which is a process with no coordinator and therefore
    /// no transaction to serialize.
    pub gate: Arc<CommandGate>,
    /// Execution facts and scheduler liveness for `/api/v2/execution-status`.
    pub execution_facts: Arc<ExecutionFactsHandle>,
}

impl RemoteControlState {
    /// Whether the scheduler task owning the current run state is alive.
    pub fn scheduler_running(&self) -> bool {
        self.execution_facts.scheduler_running()
    }
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
            gate: Arc::new(CommandGate::default()),
            execution_facts: Arc::new(ExecutionFactsHandle::default()),
        }
    }

    /// Attach the application gate this router serializes submissions with.
    pub fn with_gate(mut self, gate: Arc<CommandGate>) -> Self {
        self.gate = gate;
        self
    }

    /// Attach the execution-facts handle the status resource reads.
    pub fn with_execution_facts(mut self, facts: Arc<ExecutionFactsHandle>) -> Self {
        self.execution_facts = facts;
        self
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
///
/// Every route — including the contract-discovery routes, which need no bearer
/// token — is registered before the gate layer, so origin policy, preflight
/// handling, and the refusal of out-of-band credentials cover the whole
/// namespace. Exempting a route from the *bearer* check is [`gate`]'s decision,
/// not a reason to mount it outside the gate.
pub fn router(state: RemoteControlState) -> Router {
    Router::new()
        .route(HEALTH_PATH, get(reads::health))
        .route("/api/v2/capabilities", get(reads::capabilities))
        .route("/api/v2/instance", get(reads::instance))
        .route("/api/v2/state", get(reads::state))
        .route("/api/v2/execution-status", get(reads::execution_status))
        .route("/api/v2/changes", get(reads::list_changes))
        .route("/api/v2/changes/{change_id}", get(reads::get_change))
        .route("/api/v2/logs", get(reads::logs))
        .route("/api/v2/worktrees", get(reads::list_worktrees))
        .route("/api/v2/worktrees/{worktree_id}", get(reads::get_worktree))
        .route("/api/v2/commands", post(commands::submit_command))
        .route("/api/v2/commands/{command_id}", get(commands::get_command))
        .route("/api/v2/events", get(stream::events))
        .route("/api/v2/ws", get(stream::ws))
        .route("/api/v2/openapi.yaml", get(openapi_yaml))
        .merge(
            SwaggerUi::new("/api/v2/docs")
                .url("/api/v2/openapi.json", crate::web::openapi::document()),
        )
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), gate))
        .with_state(state)
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

    // The gate and the published contract read the same list, so a route cannot
    // be documented as authenticated while being served without credentials.
    if !crate::web::openapi::is_unauthenticated_v2_path(request.uri().path()) {
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
