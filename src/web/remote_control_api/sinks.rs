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
//!
//! # Why reading is not simply "reads are fine"
//!
//! The *argv* is disclosed only over that same socket. A TCP reader learns that
//! a subscription exists, what state the execution is in, and which events have
//! been delivered — everything it needs to decide whether it still owes a
//! resume — but not the command line the owner will run. A registration it may
//! not create is not one it may read back either.
//!
//! And every request carries the complete `(instance_id, execution_id,
//! change_id)` binding, inspection included. That is a coherence check rather
//! than access control: all three identifiers are readable through other
//! authenticated resources, so presenting them proves nothing about who you are.
//! What it does prove is that the caller and the owner mean the same execution —
//! which is exactly what an execution ID reused across a restart, or a retry
//! that opened a new episode, would otherwise silently get wrong.

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
        ("instance_id" = String, Query, description = "Owner incarnation the caller believes it is addressing. Required: inspection asserts the same complete binding a registration does"),
        ("change_id" = String, Query, description = "Change the caller believes the execution belongs to. Required: inspection asserts the same complete binding a registration does"),
    ),
    responses(
        (status = 200, description = "Current subscription for this execution. The registered argv is returned only to a request that arrived on the owner Unix socket; `sink_registered` reports presence on either transport", body = ExecutionSinkResponse),
        (status = 404, description = "No such execution in this incarnation", body = ApiError),
        (status = 409, description = "The presented instance/change binding is not this execution's", body = ApiError),
        (status = 422, description = "The request omitted part of the execution binding", body = ApiError),
    )
)]
pub async fn get_sink(
    State(state): State<RemoteControlState>,
    Extension(CorrelationId(correlation)): Extension<CorrelationId>,
    Path(execution_id): Path<String>,
    Query(params): Query<ExecutionSinkParams>,
    transport: Option<Extension<ApiTransport>>,
) -> Response {
    let Some(registry) = state.completion_sinks.get() else {
        return unsupported(&correlation);
    };
    let Some((instance_id, change_id)) = complete_binding(params) else {
        return incomplete_binding("reading", &correlation);
    };
    match registry.view(&execution_id, &instance_id, &change_id) {
        Ok(view) => body(&state, &execution_id, view, discloses_argv(&transport)).into_response(),
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
        // Mutation already proved it arrived on the owner socket, so the argv it
        // just registered is echoed back in full.
        Ok(view) => body(&state, &execution_id, view, true).into_response(),
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
        ("instance_id" = String, Query, description = "Owner incarnation the caller believes it is addressing. Required"),
        ("change_id" = String, Query, description = "Change the caller believes the execution belongs to. Required"),
    ),
    responses(
        (status = 200, description = "Sink detached", body = ExecutionSinkResponse),
        (status = 403, description = "This transport may not mutate a sink", body = ApiError),
        (status = 404, description = "No such execution in this incarnation", body = ApiError),
        (status = 409, description = "The presented instance/change binding is not this execution's", body = ApiError),
        (status = 422, description = "The request omitted part of the execution binding", body = ApiError),
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
    let Some((instance_id, change_id)) = complete_binding(params) else {
        return incomplete_binding("clearing", &correlation);
    };
    match registry.clear_sink(&execution_id, &instance_id, &change_id) {
        Ok(view) => body(&state, &execution_id, view, true).into_response(),
        Err(refusal) => refusal_response(refusal, &execution_id, &correlation),
    }
}

/// Require the complete binding every sink request asserts.
fn complete_binding(params: ExecutionSinkParams) -> Option<(String, String)> {
    match (params.instance_id, params.change_id) {
        (Some(instance_id), Some(change_id)) => Some((instance_id, change_id)),
        _ => None,
    }
}

/// Refuse a request that presented only part of the execution binding.
fn incomplete_binding(operation: &str, correlation: &str) -> Response {
    ApiError::new(
        ErrorCode::ValidationFailed,
        format!(
            "{operation} an execution sink requires the instance_id and change_id it is bound to"
        ),
        correlation,
    )
    .into_response()
}

/// Whether this transport may be told the registered argv.
///
/// Absence of the marker is read as "not the socket", the same way mutation
/// reads it: a router assembled without the per-listener extension has no proof
/// of transport, and no evidence is not evidence of the safer channel.
fn discloses_argv(transport: &Option<Extension<ApiTransport>>) -> bool {
    matches!(transport, Some(Extension(ApiTransport::Unix)))
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

/// Render one subscription, disclosing the argv only where that is permitted.
fn body(
    state: &RemoteControlState,
    execution_id: &str,
    view: SinkView,
    disclose_argv: bool,
) -> Json<ExecutionSinkResponse> {
    let execution_state = state
        .completion_sinks
        .get()
        .map(|registry| registry.execution_state(&view.change_id))
        .unwrap_or(super::dto::ChangeExecutionState::Unknown);
    let sink_registered = view.sink.is_some();
    Json(ExecutionSinkResponse {
        instance_id: state.projection.instance_id().to_string(),
        execution_id: execution_id.to_string(),
        change_id: view.change_id,
        sink: match disclose_argv {
            true => view.sink,
            false => None,
        },
        sink_registered,
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
