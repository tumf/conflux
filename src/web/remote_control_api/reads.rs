//! `/api/v2` read resources.
//!
//! Every read is a pure projection read. Nothing here touches the disk or the
//! reducer, so the `state_revision` a client gets back is exactly the revision
//! the projection owner published — a read can never invent a state that no
//! event ever described. Keeping the projection fed from disk is the refresh
//! task's job, not the reader's.

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use super::auth::CorrelationId;

use super::dto::{
    ApiError, CapabilitiesResponse, CapabilityLimits, ChangeResponse, ChangesResponse, ErrorCode,
    HealthResponse, InstanceResponse, LogsResponse, StateResponse, TransportDescriptor,
    ALL_ERROR_CODES, API_VERSION, COMMAND_RECORD_TTL_SECS, MAX_COMMAND_RECORDS,
    MAX_CORRELATION_ID_LEN, MAX_EVENTS, MAX_LOGS, SUPPORTED_COMMANDS,
};
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
