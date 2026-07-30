//! `POST /api/v2/commands` and `GET /api/v2/commands/{command_id}`.
//!
//! The admission order is the whole safety argument, so it is worth stating
//! plainly:
//!
//! 1. typed schema validation — an unknown command type dies here, before any
//!    service call exists;
//! 2. idempotency lookup — an exact replay resolves even after the revision
//!    moved on, which is what makes a client-side retry safe;
//! 3. revision validation — only for a *new* key, so a stale command cannot
//!    sneak in behind a reused key;
//! 4. atomic reservation of both records — capacity pressure fails here, before
//!    any effect;
//! 5. delegation to the shared service, which revalidates lifecycle and target
//!    immediately before it acts.

use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::Utc;

use super::auth::CorrelationId;
use super::dto::{
    is_valid_correlation_id, ApiError, CommandRecord, CommandRequest, CommandState, ErrorCode,
};
use super::projection::Admission;
use super::RemoteControlState;

/// How long the endpoint waits for a command to settle before answering `202`.
///
/// Most commands finish immediately; `stop_and_dequeue` waits on confirmed
/// process termination and can legitimately outlive a request, so the result is
/// reported through the command record instead of holding the connection.
const SYNCHRONOUS_GRACE: Duration = Duration::from_millis(250);

fn no_store(status: axum::http::StatusCode, body: impl serde::Serialize) -> Response {
    (status, [(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

fn record_response(record: &CommandRecord) -> Response {
    no_store(record.http_status(), record)
}

/// Submit a command from the closed v2 command set.
#[utoipa::path(
    post,
    path = "/api/v2/commands",
    tag = "remote-control",
    request_body = CommandRequest,
    responses(
        (status = 200, description = "Completed synchronously, replayed, or an explicit no-op", body = CommandRecord),
        (status = 202, description = "Accepted and still running", body = CommandRecord),
        (status = 409, description = "Stale revision, lifecycle, eligibility, or idempotency conflict", body = ApiError),
        (status = 422, description = "Schema or correlation-ID validation failed", body = ApiError),
        (status = 503, description = "No record slot could be reserved", body = ApiError)
    )
)]
pub async fn submit_command(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
    body: Bytes,
) -> Response {
    let correlation_id = correlation.0;

    // 1. Typed schema validation. Deserializing by hand keeps every parse
    //    failure — including an unknown command type — on the `validation_failed`
    //    path instead of axum's generic 400.
    let request: CommandRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(error) => {
            return ApiError::new(
                ErrorCode::ValidationFailed,
                format!("command envelope failed typed validation: {error}"),
                &correlation_id,
            )
            .into_response()
        }
    };

    if request.idempotency_key.is_empty() || request.idempotency_key.len() > 200 {
        return ApiError::new(
            ErrorCode::ValidationFailed,
            "idempotency_key must be 1-200 characters",
            &correlation_id,
        )
        .into_response();
    }
    if let Some(supplied) = request.correlation_id.as_deref() {
        if !is_valid_correlation_id(supplied) {
            return ApiError::new(
                ErrorCode::ValidationFailed,
                "correlation_id must be 1-64 characters matching [A-Za-z0-9._:-]",
                &correlation_id,
            )
            .into_response();
        }
    }
    let correlation_id = request.correlation_id.clone().unwrap_or(correlation_id);

    // 2-4. Lookup, revision validation, and reservation happen atomically inside
    //      the projection owner, so two concurrent submissions of the same key
    //      cannot both reserve.
    let record = match state
        .projection
        .admit(&request, &correlation_id, Utc::now())
    {
        Admission::Replay(record) => return record_response(&record),
        Admission::IdempotencyMismatch => {
            return ApiError::new(
                ErrorCode::IdempotencyMismatch,
                "idempotency_key is already bound to a different command identity",
                &correlation_id,
            )
            .with_revision(state.projection.revision())
            .into_response()
        }
        Admission::Stale(current) => {
            return ApiError::new(
                ErrorCode::StaleRevision,
                format!("expected_revision {} is stale", request.expected_revision),
                &correlation_id,
            )
            .with_revision(current)
            .into_response()
        }
        Admission::Capacity => {
            return ApiError::new(
                ErrorCode::RegistryCapacity,
                "no command slot could be reserved without evicting in-progress work",
                &correlation_id,
            )
            .with_revision(state.projection.revision())
            .into_response()
        }
        Admission::Admitted(record) => *record,
    };

    // 5. Delegate. The command runs on its own task so a long-running one (a
    //    stop that waits for confirmed termination) reports through its record
    //    instead of pinning the connection.
    let command_id = record.command_id.clone();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let executor = state.executor.clone();
    let projection = state.projection.clone();
    let spec = request.command.clone();
    let settle_id = command_id.clone();
    tokio::spawn(async move {
        let (command_state, detail, error_code) = match executor.execute(&spec).await {
            Ok(summary) if summary.changed => (CommandState::Succeeded, summary.detail, None),
            Ok(summary) => (CommandState::NoOp, summary.detail, None),
            Err(failure) => (
                CommandState::Failed,
                Some(failure.message),
                Some(failure.error_code),
            ),
        };
        projection.complete_command(&settle_id, command_state, detail, error_code);
        let _ = done_tx.send(());
    });

    let _ = tokio::time::timeout(SYNCHRONOUS_GRACE, done_rx).await;
    match state.projection.command(&command_id) {
        Some(settled) => record_response(&settled),
        // The record was reserved, so this cannot normally happen; report it as
        // still running rather than inventing an outcome.
        None => record_response(&record),
    }
}

/// Look up a previously submitted command.
#[utoipa::path(
    get,
    path = "/api/v2/commands/{command_id}",
    tag = "remote-control",
    params(("command_id" = String, Path, description = "Command ID")),
    responses(
        (status = 200, description = "Command record", body = CommandRecord),
        (status = 404, description = "No such command in this incarnation", body = ApiError)
    )
)]
pub async fn get_command(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
    Path(command_id): Path<String>,
) -> Response {
    match state.projection.command(&command_id) {
        Some(record) => no_store(axum::http::StatusCode::OK, record),
        None => ApiError::new(
            ErrorCode::NotFound,
            format!("command '{command_id}' is not known to this instance"),
            &correlation.0,
        )
        .into_response(),
    }
}
