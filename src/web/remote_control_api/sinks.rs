//! `/api/v2/executions/{execution_id}/sink` — subscription, not command.
//!
//! These three resources sit deliberately *outside* the closed workflow command
//! registry. A sink registration is not a command: it creates no command record,
//! carries no `expected_revision` and no idempotency key, does not advance
//! `state_revision`, and cannot move a change. Routing it through `POST
//! /api/v2/commands` would have made every one of those statements false, and
//! would have put an executable argv inside the one surface whose whole job is
//! to mutate orchestration state.
//!
//! # Why mutation is Unix-socket only
//!
//! A sink is an argv this process will execute. The Unix socket is mode `0600`
//! and derived from the repository's own Git common directory, so reaching it
//! already means local filesystem access to the owner's repository. A bearer
//! token over TCP is a weaker claim than that, and it is exactly the claim an
//! exposed reverse proxy hands out. Reads are fine over either transport;
//! `PUT` and `DELETE` are refused on anything but the socket, with their own
//! error code so a client can tell "wrong channel" from "wrong credentials".

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use super::auth::CorrelationId;
use super::dto::{
    ApiError, ErrorCode, ExecutionSinkParams, ExecutionSinkRequest, ExecutionSinkResponse,
    ExecutionSinkSpec,
};
use super::{ApiTransport, RemoteControlState};
use crate::web::completion_sink::{SinkRefusal, SinkView};

/// Read the sink attached to one execution.
#[utoipa::path(
    get,
    path = "/api/v2/executions/{execution_id}/sink",
    tag = "remote-control",
    params(
        ("execution_id" = String, Path, description = "Process-local execution episode ID"),
        ("instance_id" = Option<String>, Query, description = "Owner incarnation the caller believes it is addressing"),
        ("change_id" = Option<String>, Query, description = "Change the caller believes the execution belongs to"),
    ),
    responses(
        (status = 200, description = "Current subscription for this execution", body = ExecutionSinkResponse),
        (status = 404, description = "No such execution in this incarnation", body = ApiError),
        (status = 409, description = "The presented instance/change binding is not this execution's", body = ApiError),
    )
)]
pub async fn get_sink(
    State(state): State<RemoteControlState>,
    Extension(CorrelationId(correlation)): Extension<CorrelationId>,
    Path(execution_id): Path<String>,
    Query(params): Query<ExecutionSinkParams>,
) -> Response {
    let Some(registry) = state.completion_sinks.get() else {
        return unsupported(&correlation);
    };
    match registry.view(
        &execution_id,
        params.instance_id.as_deref(),
        params.change_id.as_deref(),
    ) {
        Ok(view) => body(&state, &execution_id, view).into_response(),
        Err(refusal) => refusal_response(refusal, &execution_id, &correlation),
    }
}

/// Attach or replace the sink for one execution.
#[utoipa::path(
    put,
    path = "/api/v2/executions/{execution_id}/sink",
    tag = "remote-control",
    params(("execution_id" = String, Path, description = "Process-local execution episode ID")),
    request_body = ExecutionSinkRequest,
    responses(
        (status = 200, description = "Sink attached or replaced", body = ExecutionSinkResponse),
        (status = 403, description = "This transport may not register an executable argv", body = ApiError),
        (status = 404, description = "No such execution in this incarnation", body = ApiError),
        (status = 409, description = "The presented instance/change binding is not this execution's", body = ApiError),
        (status = 422, description = "The argv is not an acceptable bounded command", body = ApiError),
    )
)]
pub async fn put_sink(
    State(state): State<RemoteControlState>,
    Extension(CorrelationId(correlation)): Extension<CorrelationId>,
    Path(execution_id): Path<String>,
    transport: Option<Extension<ApiTransport>>,
    Json(request): Json<ExecutionSinkRequest>,
) -> Response {
    let Some(registry) = state.completion_sinks.get() else {
        return unsupported(&correlation);
    };
    if let Some(refusal) = require_owner_socket(transport, &correlation) {
        return refusal;
    }
    match registry.set_sink(
        &execution_id,
        &request.instance_id,
        &request.change_id,
        ExecutionSinkSpec {
            command: request.command,
            notify_blocked: request.notify_blocked,
        },
    ) {
        Ok(view) => body(&state, &execution_id, view).into_response(),
        Err(refusal) => refusal_response(refusal, &execution_id, &correlation),
    }
}

/// Detach the sink for one execution.
#[utoipa::path(
    delete,
    path = "/api/v2/executions/{execution_id}/sink",
    tag = "remote-control",
    params(
        ("execution_id" = String, Path, description = "Process-local execution episode ID"),
        ("instance_id" = Option<String>, Query, description = "Owner incarnation the caller believes it is addressing"),
        ("change_id" = Option<String>, Query, description = "Change the caller believes the execution belongs to"),
    ),
    responses(
        (status = 200, description = "Sink detached", body = ExecutionSinkResponse),
        (status = 403, description = "This transport may not mutate a sink", body = ApiError),
        (status = 404, description = "No such execution in this incarnation", body = ApiError),
        (status = 409, description = "The presented instance/change binding is not this execution's", body = ApiError),
    )
)]
pub async fn delete_sink(
    State(state): State<RemoteControlState>,
    Extension(CorrelationId(correlation)): Extension<CorrelationId>,
    Path(execution_id): Path<String>,
    Query(params): Query<ExecutionSinkParams>,
    transport: Option<Extension<ApiTransport>>,
) -> Response {
    let Some(registry) = state.completion_sinks.get() else {
        return unsupported(&correlation);
    };
    if let Some(refusal) = require_owner_socket(transport, &correlation) {
        return refusal;
    }
    let (Some(instance_id), Some(change_id)) = (params.instance_id, params.change_id) else {
        return ApiError::new(
            ErrorCode::ValidationFailed,
            "clearing a sink requires the instance_id and change_id it is bound to",
            &correlation,
        )
        .into_response();
    };
    match registry.clear_sink(&execution_id, &instance_id, &change_id) {
        Ok(view) => body(&state, &execution_id, view).into_response(),
        Err(refusal) => refusal_response(refusal, &execution_id, &correlation),
    }
}

/// Refuse a mutation that did not arrive on the owner's Unix socket.
///
/// Absence of the marker is treated as "not the socket": a router assembled
/// without the per-listener extension has no proof of transport, and failing
/// closed is the only safe reading of no evidence.
fn require_owner_socket(
    transport: Option<Extension<ApiTransport>>,
    correlation: &str,
) -> Option<Response> {
    match transport.map(|Extension(transport)| transport) {
        Some(ApiTransport::Unix) => None,
        _ => Some(
            ApiError::new(
                ErrorCode::TransportNotPermitted,
                "an execution sink stores an argv this owner will execute, so it may be set or \
                 cleared only over the owner's Unix socket",
                correlation,
            )
            .into_response(),
        ),
    }
}

fn body(
    state: &RemoteControlState,
    execution_id: &str,
    view: SinkView,
) -> Json<ExecutionSinkResponse> {
    let execution_state = state
        .completion_sinks
        .get()
        .map(|registry| registry.execution_state(&view.change_id))
        .unwrap_or(super::dto::ChangeExecutionState::Unknown);
    Json(ExecutionSinkResponse {
        instance_id: state.projection.instance_id().to_string(),
        execution_id: execution_id.to_string(),
        change_id: view.change_id,
        sink: view.sink,
        execution_state,
        terminal_dispatched: view.terminal_dispatched,
        delivered_events: view.delivered_events,
    })
}

fn refusal_response(refusal: SinkRefusal, execution_id: &str, correlation: &str) -> Response {
    match refusal {
        SinkRefusal::UnknownExecution => ApiError::new(
            ErrorCode::NotFound,
            format!("this incarnation has no execution '{execution_id}'"),
            correlation,
        )
        .into_response(),
        SinkRefusal::BindingMismatch { .. } => ApiError::new(
            ErrorCode::ExecutionBindingMismatch,
            format!(
                "execution '{execution_id}' does not belong to the instance and change presented \
                 with this request"
            ),
            correlation,
        )
        .into_response(),
        SinkRefusal::InvalidCommand(detail) => {
            ApiError::new(ErrorCode::ValidationFailed, detail, correlation).into_response()
        }
    }
}

/// The refusal of a build that serves the route but binds no registry.
fn unsupported(correlation: &str) -> Response {
    ApiError::new(
        ErrorCode::CommandExecutorUnbound,
        "this process has no execution-sink registry bound, so no subscription can be held",
        correlation,
    )
    .into_response()
}
