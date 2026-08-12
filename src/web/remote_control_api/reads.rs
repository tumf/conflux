//! `/api/v2` read resources.
//!
//! Every read is a pure projection read. Nothing here touches the disk or the
//! reducer, so the `state_revision` a client gets back is exactly the revision
//! the projection owner published — a read can never invent a state that no
//! event ever described. Keeping the projection fed from disk is the refresh
//! task's job, not the reader's.

use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;

use super::auth::CorrelationId;

use super::dto::{
    ApiError, CapabilitiesResponse, CapabilityLimits, ChangeExecutionState, ChangeExecutionStatus,
    ChangeResponse, ChangesResponse, CommandExecutionCapability, ErrorCode,
    ExecutionContractResponse, ExecutionPhase, ExecutionStatusResponse, HealthResponse,
    InstanceResponse, LatestLogProjection, LogsResponse, ParallelCapabilities,
    ProcessExecutionStatus, StateResponse, TransportDescriptor, ALL_ERROR_CODES,
    ALL_PARALLEL_BLOCKED_REASONS, API_VERSION, COMMAND_RECORD_TTL_SECS, MAX_COMMAND_RECORDS,
    MAX_CORRELATION_ID_LEN, MAX_EVENTS, MAX_LOGS, SUPPORTED_COMMANDS,
};
use super::worktrees::{WorktreeCapabilities, WorktreeResponse, WorktreesResponse};
use super::RemoteControlState;

/// Responses describe a live process, so nothing may be cached anywhere.
fn no_store<T: serde::Serialize>(body: T) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

/// Liveness probe. Always unauthenticated so an operator can tell "down" apart
/// from "misconfigured credentials" without holding a token.
#[utoipa::path(
    get,
    path = "/api/v2/health",
    tag = "remote-control",
    security(),
    responses((status = 200, description = "Process is serving requests", body = HealthResponse))
)]
pub async fn health() -> Response {
    no_store(HealthResponse {
        status: "ok".to_string(),
        api_version: API_VERSION.to_string(),
        version: format!("v{} ({})", env!("CARGO_PKG_VERSION"), env!("BUILD_NUMBER")),
    })
}

/// Complete description of what this instance accepts and guarantees.
#[utoipa::path(
    get,
    path = "/api/v2/capabilities",
    tag = "remote-control",
    responses((status = 200, description = "Supported commands, transports, and limits", body = CapabilitiesResponse))
)]
pub async fn capabilities(State(state): State<RemoteControlState>) -> Response {
    // Read from the published snapshot rather than from a second source: a
    // client that compares capabilities with `/api/v2/state` must never see two
    // different answers about whether parallel execution is available.
    let parallel = state.projection.snapshot().0.parallel;
    no_store(CapabilitiesResponse {
        api_version: API_VERSION.to_string(),
        instance_id: state.projection.instance_id().to_string(),
        commands: SUPPORTED_COMMANDS.iter().map(|c| c.to_string()).collect(),
        transports: vec![
            TransportDescriptor {
                name: "sse".to_string(),
                path: "/api/v2/events".to_string(),
                // `EventSource` cannot send an Authorization header, so an
                // authenticated browser client must use fetch() streaming.
                client: "fetch-response-streaming".to_string(),
                browser_native_supported: false,
            },
            TransportDescriptor {
                name: "websocket".to_string(),
                path: "/api/v2/ws".to_string(),
                // Browsers cannot set request headers on a WebSocket handshake.
                client: "non-browser".to_string(),
                browser_native_supported: false,
            },
        ],
        error_codes: ALL_ERROR_CODES
            .iter()
            .map(|code| code.as_str().to_string())
            .collect(),
        limits: CapabilityLimits {
            max_events: MAX_EVENTS,
            max_logs: MAX_LOGS,
            max_commands: MAX_COMMAND_RECORDS,
            max_idempotency_records: MAX_COMMAND_RECORDS,
            command_record_ttl_secs: COMMAND_RECORD_TTL_SECS,
            max_correlation_id_len: MAX_CORRELATION_ID_LEN,
        },
        authentication_required: state.auth.is_enforced(),
        command_execution: CommandExecutionCapability {
            available: state.executor.is_command_capable().await,
        },
        worktrees: WorktreeCapabilities::default(),
        parallel: ParallelCapabilities {
            max_concurrent: parallel.max_concurrent,
            vcs_backend: parallel.vcs_backend,
            blocked_reasons: ALL_PARALLEL_BLOCKED_REASONS
                .iter()
                .map(|reason| (*reason).to_string())
                .collect(),
        },
    })
}

/// Process incarnation identity. Clients compare `instance_id` before reusing a cursor.
#[utoipa::path(
    get,
    path = "/api/v2/instance",
    tag = "remote-control",
    responses((status = 200, description = "Process incarnation identity", body = InstanceResponse))
)]
pub async fn instance(State(state): State<RemoteControlState>) -> Response {
    no_store(InstanceResponse {
        instance_id: state.projection.instance_id().to_string(),
        started_at: state.projection.started_at().to_string(),
        pid: std::process::id(),
        version: format!("v{} ({})", env!("CARGO_PKG_VERSION"), env!("BUILD_NUMBER")),
        api_version: API_VERSION.to_string(),
    })
}

/// Coherent full snapshot plus the cursor to resume streaming from.
#[utoipa::path(
    get,
    path = "/api/v2/state",
    tag = "remote-control",
    responses((status = 200, description = "Coherent snapshot with its revision and cursor", body = StateResponse))
)]
pub async fn state(State(state): State<RemoteControlState>) -> Response {
    let (snapshot, state_revision, event_sequence) = state.projection.snapshot();
    no_store(StateResponse {
        instance_id: state.projection.instance_id().to_string(),
        state_revision,
        event_sequence,
        snapshot,
    })
}

/// Projected changes with their reducer-derived display statuses.
#[utoipa::path(
    get,
    path = "/api/v2/changes",
    tag = "remote-control",
    responses((status = 200, description = "Projected changes", body = ChangesResponse))
)]
pub async fn list_changes(State(state): State<RemoteControlState>) -> Response {
    let (snapshot, state_revision, _) = state.projection.snapshot();
    no_store(ChangesResponse {
        instance_id: state.projection.instance_id().to_string(),
        state_revision,
        changes: snapshot.changes,
    })
}

/// One projected change.
#[utoipa::path(
    get,
    path = "/api/v2/changes/{change_id}",
    tag = "remote-control",
    params(("change_id" = String, Path, description = "Change ID")),
    responses(
        (status = 200, description = "Projected change", body = ChangeResponse),
        (status = 404, description = "No such change in this incarnation", body = ApiError)
    )
)]
pub async fn get_change(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
    Path(change_id): Path<String>,
) -> Response {
    let (snapshot, state_revision, _) = state.projection.snapshot();
    match snapshot.changes.into_iter().find(|c| c.id == change_id) {
        Some(change) => no_store(ChangeResponse {
            instance_id: state.projection.instance_id().to_string(),
            state_revision,
            change,
        }),
        None => ApiError::new(
            ErrorCode::NotFound,
            format!("change '{change_id}' is not present in this instance"),
            &correlation.0,
        )
        .with_revision(state_revision)
        .into_response(),
    }
}

/// Current worktrees, each with the opaque ID that addresses it.
///
/// Unlike the other reads this one observes the repository rather than the
/// projection, because worktree existence is a filesystem/Git fact and a stale
/// projection of it would hand a client an ID that no longer names anything.
#[utoipa::path(
    get,
    path = "/api/v2/worktrees",
    tag = "remote-control",
    responses(
        (status = 200, description = "Current worktrees with opaque IDs", body = WorktreesResponse),
        (status = 409, description = "No worktree runtime is bound yet", body = ApiError),
        (status = 500, description = "The repository could not be observed", body = ApiError)
    )
)]
pub async fn list_worktrees(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
) -> Response {
    let state_revision = state.projection.revision();
    match state.worktrees.list().await {
        Ok(listing) => no_store(WorktreesResponse {
            instance_id: state.projection.instance_id().to_string(),
            state_revision,
            repository_id: listing.repository_id,
            worktrees: listing.worktrees,
        }),
        Err(failure) => ApiError::new(failure.error_code, failure.message, &correlation.0)
            .with_revision(state_revision)
            .into_response(),
    }
}

/// One worktree, addressed only by its opaque ID.
#[utoipa::path(
    get,
    path = "/api/v2/worktrees/{worktree_id}",
    tag = "remote-control",
    params(("worktree_id" = String, Path, description = "Opaque process-local worktree ID")),
    responses(
        (status = 200, description = "The requested worktree", body = WorktreeResponse),
        (status = 404, description = "The ID is unknown or was retired", body = ApiError),
        (status = 409, description = "No worktree runtime is bound yet", body = ApiError)
    )
)]
pub async fn get_worktree(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
    Path(worktree_id): Path<String>,
) -> Response {
    let state_revision = state.projection.revision();
    let listing = match state.worktrees.list().await {
        Ok(listing) => listing,
        Err(failure) => {
            return ApiError::new(failure.error_code, failure.message, &correlation.0)
                .with_revision(state_revision)
                .into_response()
        }
    };

    match listing
        .worktrees
        .into_iter()
        .find(|worktree| worktree.worktree_id == worktree_id)
    {
        Some(worktree) => no_store(WorktreeResponse {
            instance_id: state.projection.instance_id().to_string(),
            state_revision,
            worktree,
        }),
        None => ApiError::new(
            ErrorCode::WorktreeNotFound,
            format!("worktree '{worktree_id}' is not a current resource in this instance"),
            &correlation.0,
        )
        .with_revision(state_revision)
        .into_response(),
    }
}

/// Coherent machine-readable answer to "what is actually happening right now".
///
/// This is the resource an agent reads *before* intervening. `/state` says what
/// may be commanded and `/logs` says what was printed; neither closes the gap
/// between "the scheduler is alive" and "lifecycle work is running", and neither
/// says which phase most recently completed. Getting that wrong is how an agent
/// creates a duplicate manual commit while Conflux is still settling.
///
/// The snapshot and the retained log ring are read under one hold of the
/// projection lock, so the phase, the revision, the cursor, and the log line all
/// describe the same instant. Every timestamp is an absolute UTC RFC 3339
/// instant, including `observed_at`, which is what lets a client render relative
/// time without trusting its own clock — the server returns no elapsed counter
/// and no relative-time text at all.
#[utoipa::path(
    get,
    path = "/api/v2/execution-status",
    tag = "remote-control",
    responses((status = 200, description = "Coherent execution observation with absolute timestamps", body = ExecutionStatusResponse))
)]
pub async fn execution_status(State(state): State<RemoteControlState>) -> Response {
    let observation = state.projection.execution_observation();
    let facts = state.execution_facts.snapshot();

    let changes = observation
        .snapshot
        .changes
        .iter()
        .map(|change| {
            let change_facts = facts.change(&change.id);
            ChangeExecutionStatus {
                id: change.id.clone(),
                execution_state: ChangeExecutionState::from_shared(change_facts.execution_state),
                current_phase: ExecutionPhase::from_shared(change_facts.current_phase),
                last_completed_phase: change_facts
                    .last_completed_phase
                    .map(ExecutionPhase::from_shared),
                iteration: change.iteration_number,
                phase_started_at: change_facts.phase_started_at.map(|at| at.to_rfc3339()),
                last_completed_at: change_facts.last_completed_at.map(|at| at.to_rfc3339()),
                run_started_at: change.timing.started_at.clone(),
                run_completed_at: change.timing.completed_at.clone(),
                latest_activity: change.latest_activity.clone(),
                latest_log: observation
                    .change_logs
                    .get(&change.id)
                    .map(LatestLogProjection::from_entry),
            }
        })
        .collect();

    no_store(ExecutionStatusResponse {
        instance_id: state.projection.instance_id().to_string(),
        state_revision: observation.state_revision,
        event_sequence: observation.event_sequence,
        observed_at: chrono::Utc::now().to_rfc3339(),
        process: ProcessExecutionStatus {
            app_mode: observation.snapshot.app_mode.clone(),
            scheduler_running: state.scheduler_running(),
            has_active_work: facts.has_active_work(),
            active_activities: facts
                .activities
                .iter()
                .map(|activity| activity.as_str().to_string())
                .collect(),
            latest_log: observation
                .process_log
                .as_ref()
                .map(LatestLogProjection::from_entry),
        },
        changes,
    })
}

/// Change selector accepted by the execution-contract resource.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContractParams {
    /// Resolve the change-scoped branch identity for this change.
    #[serde(default)]
    pub change_id: Option<String>,
}

/// The minimal typed contract that says what would *prove* a change finished.
///
/// A client cannot derive this from the snapshot: `display_status: merged` is a
/// presentation of what the owner believes, while completion has to be certified
/// from repository evidence, and which evidence counts depends on whether this
/// owner merges to base, publishes base, or pushes the change branch. This
/// resource names the base branch and that mode, and nothing else — it holds no
/// per-change commit, and it is never authoritative for routing.
///
/// The revision is read *after* the contract so a client joining this response
/// with `/api/v2/state` cannot be handed a contract older than the revision it
/// is reconciling at.
#[utoipa::path(
    get,
    path = "/api/v2/execution-contract",
    tag = "remote-control",
    params(("change_id" = Option<String>, Query, description = "Resolve change-scoped branch identity for this change")),
    responses((status = 200, description = "Owner execution contract at an instance and revision", body = ExecutionContractResponse))
)]
pub async fn execution_contract(
    State(state): State<RemoteControlState>,
    Query(params): Query<ContractParams>,
) -> Response {
    let contract = state
        .execution_contract
        .resolve(params.change_id.as_deref());
    no_store(ExecutionContractResponse {
        instance_id: state.projection.instance_id().to_string(),
        state_revision: state.projection.revision(),
        contract,
    })
}

/// Bounded observational log ring.
#[utoipa::path(
    get,
    path = "/api/v2/logs",
    tag = "remote-control",
    responses((status = 200, description = "Retained log entries, oldest first", body = LogsResponse))
)]
pub async fn logs(State(state): State<RemoteControlState>) -> Response {
    let (logs, state_revision, event_sequence) = state.projection.logs();
    no_store(LogsResponse {
        instance_id: state.projection.instance_id().to_string(),
        state_revision,
        event_sequence,
        logs,
    })
}
