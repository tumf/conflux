//! `/api/v2/proposals/{change_id}/subscription` — notification, not command.
//!
//! # Why a proposal is the key
//!
//! An execution ID exists only once the owner has admitted work. An agent that
//! wants to be told when its proposal finishes has to say so *before* that — it
//! marks the proposal, asks for a Start, and then has nothing to name until the
//! owner acts. Keying the subscription by the proposal closes that gap: the
//! owner binds each new execution episode of that proposal to whatever
//! subscription is current, so re-admission after a retry produces a distinct
//! notification instead of a lost one.
//!
//! # Why it is not a command
//!
//! Like the execution-scoped sink, this sits outside the closed workflow command
//! registry. A subscription creates no command record, carries no
//! `expected_revision` and no idempotency key, does not advance `state_revision`,
//! and cannot move a proposal. It is observability: registering one promises
//! nothing about admission, and delivering one authorizes nothing.
//!
//! # Why mutation is Unix-socket only
//!
//! A subscription stores an argv this process will execute. Reaching the mode
//! `0600` socket derived from the repository's own Git common directory already
//! means local filesystem access to the owner's repository; a bearer token over
//! TCP is a weaker claim than that, and it is exactly the claim an exposed
//! reverse proxy hands out. Reads work on either transport, but the registered
//! *argv* comes back only over that socket: a channel that may not register a
//! command may not read one back.
//!
//! Every request, inspection included, carries the complete
//! `(instance_id, change_id)` binding. That is a coherence check rather than
//! access control — both identifiers are readable through other authenticated
//! resources — and what it proves is that the caller and the owner mean the same
//! *incarnation*. Subscriptions are process-local, so a caller holding one from
//! a previous owner must be told the owner was replaced rather than silently
//! registering against a process that never saw its work.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use super::auth::CorrelationId;
use super::dto::{
    ApiError, ErrorCode, ExecutionSinkSpec, ProposalSubscriptionParams,
    ProposalSubscriptionRequest, ProposalSubscriptionResponse,
};
use super::{ApiTransport, RemoteControlState};
use crate::web::completion_sink::{ProposalSubscriptionView, SinkRefusal};

/// Read one proposal's subscription.
#[utoipa::path(
    get,
    path = "/api/v2/proposals/{change_id}/subscription",
    tag = "remote-control",
    params(
        ("change_id" = String, Path, description = "Proposal the subscription is keyed by"),
        ("instance_id" = String, Query, description = "Owner incarnation the caller believes it is addressing. Required: inspection asserts the same complete binding a registration does"),
    ),
    responses(
        (status = 200, description = "Current subscription for this proposal. The registered argv is returned only to a request that arrived on the owner Unix socket; `subscribed` reports presence on either transport", body = ProposalSubscriptionResponse),
        (status = 409, description = "The presented instance binding is not this owner incarnation", body = ApiError),
        (status = 422, description = "The request omitted part of the binding", body = ApiError),
    )
)]
pub async fn get_subscription(
    State(state): State<RemoteControlState>,
    Extension(CorrelationId(correlation)): Extension<CorrelationId>,
    Path(change_id): Path<String>,
    Query(params): Query<ProposalSubscriptionParams>,
    transport: Option<Extension<ApiTransport>>,
) -> Response {
    let Some(registry) = state.completion_sinks.get() else {
        return unsupported(&correlation);
    };
    let Some(instance_id) = params.instance_id else {
        return incomplete_binding("reading", &correlation);
    };
    match registry.view_proposal_subscription(&change_id, &instance_id) {
        Ok(view) => body(&state, view, discloses_argv(&transport)).into_response(),
        Err(refusal) => refusal_response(refusal, &change_id, &correlation),
    }
}

/// Register or replace one proposal's subscription.
#[utoipa::path(
    put,
    path = "/api/v2/proposals/{change_id}/subscription",
    tag = "remote-control",
    params(("change_id" = String, Path, description = "Proposal the subscription is keyed by")),
    request_body = ProposalSubscriptionRequest,
    responses(
        (status = 200, description = "Subscription registered or replaced", body = ProposalSubscriptionResponse),
        (status = 403, description = "This transport may not register an executable argv", body = ApiError),
        (status = 409, description = "The presented instance binding is not this owner incarnation", body = ApiError),
        (status = 422, description = "The argv is not an acceptable bounded command", body = ApiError),
    )
)]
pub async fn put_subscription(
    State(state): State<RemoteControlState>,
    Extension(CorrelationId(correlation)): Extension<CorrelationId>,
    Path(change_id): Path<String>,
    transport: Option<Extension<ApiTransport>>,
    Json(request): Json<ProposalSubscriptionRequest>,
) -> Response {
    let Some(registry) = state.completion_sinks.get() else {
        return unsupported(&correlation);
    };
    if let Some(refusal) = require_owner_socket(transport, &correlation) {
        return refusal;
    }
    match registry.set_proposal_subscription(
        &change_id,
        &request.instance_id,
        ExecutionSinkSpec {
            command: request.command,
            notify_blocked: request.notify_blocked,
        },
    ) {
        // Mutation already proved it arrived on the owner socket, so the argv it
        // just registered is echoed back in full.
        Ok(view) => body(&state, view, true).into_response(),
        Err(refusal) => refusal_response(refusal, &change_id, &correlation),
    }
}

/// Clear one proposal's subscription.
#[utoipa::path(
    delete,
    path = "/api/v2/proposals/{change_id}/subscription",
    tag = "remote-control",
    params(
        ("change_id" = String, Path, description = "Proposal the subscription is keyed by"),
        ("instance_id" = String, Query, description = "Owner incarnation the caller believes it is addressing. Required"),
    ),
    responses(
        (status = 200, description = "Subscription cleared", body = ProposalSubscriptionResponse),
        (status = 403, description = "This transport may not mutate a subscription", body = ApiError),
        (status = 409, description = "The presented instance binding is not this owner incarnation", body = ApiError),
        (status = 422, description = "The request omitted part of the binding", body = ApiError),
    )
)]
pub async fn delete_subscription(
    State(state): State<RemoteControlState>,
    Extension(CorrelationId(correlation)): Extension<CorrelationId>,
    Path(change_id): Path<String>,
    Query(params): Query<ProposalSubscriptionParams>,
    transport: Option<Extension<ApiTransport>>,
) -> Response {
    let Some(registry) = state.completion_sinks.get() else {
        return unsupported(&correlation);
    };
    if let Some(refusal) = require_owner_socket(transport, &correlation) {
        return refusal;
    }
    let Some(instance_id) = params.instance_id else {
        return incomplete_binding("clearing", &correlation);
    };
    match registry.clear_proposal_subscription(&change_id, &instance_id) {
        Ok(view) => body(&state, view, true).into_response(),
        Err(refusal) => refusal_response(refusal, &change_id, &correlation),
    }
}

/// Refuse a request that presented only part of the binding.
fn incomplete_binding(operation: &str, correlation: &str) -> Response {
    ApiError::new(
        ErrorCode::ValidationFailed,
        format!("{operation} a proposal subscription requires the instance_id it is bound to"),
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
fn require_owner_socket(
    transport: Option<Extension<ApiTransport>>,
    correlation: &str,
) -> Option<Response> {
    match transport.map(|Extension(transport)| transport) {
        Some(ApiTransport::Unix) => None,
        _ => Some(
            ApiError::new(
                ErrorCode::TransportNotPermitted,
                "a proposal subscription stores an argv this owner will execute, so it may be \
                 set or cleared only over the owner's Unix socket",
                correlation,
            )
            .into_response(),
        ),
    }
}

/// Render one subscription, disclosing the argv only where that is permitted.
fn body(
    state: &RemoteControlState,
    view: ProposalSubscriptionView,
    disclose_argv: bool,
) -> Json<ProposalSubscriptionResponse> {
    let execution_state = state
        .completion_sinks
        .get()
        .map(|registry| registry.execution_state(&view.change_id))
        .unwrap_or(super::dto::ChangeExecutionState::Unknown);
    let subscribed = view.sink.is_some();
    Json(ProposalSubscriptionResponse {
        instance_id: state.projection.instance_id().to_string(),
        change_id: view.change_id,
        sink: match disclose_argv {
            true => view.sink,
            false => None,
        },
        subscribed,
        execution_id: view.execution_id,
        execution_state,
        terminal_dispatched: view.terminal_dispatched,
        delivered_events: view.delivered_events,
    })
}

fn refusal_response(refusal: SinkRefusal, change_id: &str, correlation: &str) -> Response {
    match refusal {
        SinkRefusal::InstanceMismatch => ApiError::new(
            ErrorCode::ExecutionBindingMismatch,
            "the presented instance_id is not this owner incarnation, so a subscription for \
             '{change_id}' would belong to a process this caller never observed"
                .replace("{change_id}", change_id),
            correlation,
        )
        .into_response(),
        SinkRefusal::InvalidCommand(detail) => {
            ApiError::new(ErrorCode::ValidationFailed, detail, correlation).into_response()
        }
        // Neither is reachable: a proposal subscription is keyed by the change
        // in the path, so there is no execution to be unknown or mis-bound.
        // Answering with the same typed code the binding checks use keeps that
        // an honest refusal rather than an internal error a client cannot act on.
        SinkRefusal::UnknownExecution | SinkRefusal::BindingMismatch { .. } => ApiError::new(
            ErrorCode::ExecutionBindingMismatch,
            format!("the subscription binding presented for '{change_id}' is not this owner's"),
            correlation,
        )
        .into_response(),
    }
}

/// The refusal of a build that serves the route but binds no registry.
fn unsupported(correlation: &str) -> Response {
    ApiError::new(
        ErrorCode::CommandExecutorUnbound,
        "this process has no completion-sink registry bound, so no proposal subscription can be \
         held",
        correlation,
    )
    .into_response()
}
