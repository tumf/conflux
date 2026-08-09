//! `POST /api/v2/commands` and `GET /api/v2/commands/{command_id}`.
//!
//! The admission order is the whole safety argument, so it is worth stating
//! plainly:
//!
//! 1. typed schema validation — an unknown command type dies here, before any
//!    service call exists;
//! 2. idempotency lookup — an exact replay resolves even after the revision
//!    moved on, and without waiting for the gate, which is what makes a
//!    client-side retry safe *and* non-blocking;
//! 3. acquisition of the process-local application gate;
//! 4. revision validation — only for a *new* key, so a stale command cannot
//!    sneak in behind a reused key;
//! 5. atomic reservation of both records — capacity pressure fails here, before
//!    any effect;
//! 6. delegation to the shared application transaction, which revalidates
//!    lifecycle and target immediately before it acts;
//! 7. settlement at the exact revision that command's outcome dispatch produced.
//!
//! Steps 3 through 7 are one critical section, and that is the difference from
//! the previous shape. Optimistic revisions alone were not enough: admission was
//! atomic for the *record*, but the effect that consumes the revision landed
//! after the lock was released, so two new commands carrying the same
//! `expected_revision` could both pass validation and both execute. Holding the
//! gate until the record is settled makes the second one observe the revision
//! the first consumed, and fail stale without ever reaching a service.
//!
//! `stop_and_dequeue` is the one exception, and it is explicit: it holds the
//! gate only for admission and cancellation issuance, then releases it and
//! settles from a spawned continuation. Its confirmation wait must not
//! monopolize operator admission, block force stop, or stall event fan-out.

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
use super::executor::{Applied, CommandFailure, ExecutionSummary, PendingCommand};
use super::projection::{Admission, Projection};
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

    // 2. Exact replay resolves without the gate. A retry of a command that is
    //    still executing must return the original record rather than queue
    //    behind it, and a mismatched key must be refused without waiting at all.
    match state.projection.resolve_replay(&request, Utc::now()) {
        Some(Admission::Replay(record)) => return record_response(&record),
        Some(Admission::IdempotencyMismatch) => {
            return ApiError::new(
                ErrorCode::IdempotencyMismatch,
                "idempotency_key is already bound to a different command identity",
                &correlation_id,
            )
            .with_revision(state.projection.revision())
            .into_response()
        }
        _ => {}
    }

    // 3. From here to settlement is one critical section.
    let gate = state.gate.hold().await;

    // 4-5. Revision validation and reservation happen atomically inside the
    //      projection owner, and now under the gate, so the revision a command
    //      is validated against is one no other command can still be about to
    //      consume.
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

    // 6-7. Delegate under the gate and settle at the revision the outcome
    //      dispatch produced.
    let command_id = record.command_id.clone();
    let projection = state.projection.clone();
    // The guard is *moved* into the executor: an ordinary command holds it
    // through settlement, while a two-phase one drops it before waiting. Keeping
    // a copy here and re-entering the coordinator would deadlock on a lock this
    // request already owns.
    let settlement: Option<PendingCommand> =
        match state.executor.begin(&request.command, gate).await {
            // The gate travels into the spawned task, so an ordinary command is
            // serialized through settlement while still reporting through its record
            // rather than pinning the connection.
            Applied::Ordinary(gate) => {
                let executor = state.executor.clone();
                let spec = request.command.clone();
                Some(Box::pin(
                    async move { executor.execute_held(&spec, gate).await },
                ))
            }
            // A two-phase command keeps its record Running while confirmation is
            // pending; the executor already released the gate.
            Applied::Pending(pending) => Some(pending),
            // Refused before any effect: nothing to run.
            Applied::Settled(result) => {
                settle(&projection, &command_id, result);
                None
            }
        };

    if let Some(pending) = settlement {
        let settle_id = command_id.clone();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let result = pending.await;
            settle(&projection, &settle_id, result);
            let _ = done_tx.send(());
        });
        let _ = tokio::time::timeout(SYNCHRONOUS_GRACE, done_rx).await;
    }

    match state.projection.command(&command_id) {
        Some(settled) => record_response(&settled),
        // The record was reserved, so this cannot normally happen; report it as
        // still running rather than inventing an outcome.
        None => record_response(&record),
    }
}

/// Settle one command record from the actual outcome of its execution.
///
/// `result_revision` travels with the outcome rather than being read here: a
/// command's recorded revision must identify the snapshot containing *its*
/// decision fields, and by the time this runs, later progress may already have
/// advanced the projection.
fn settle(
    projection: &Projection,
    command_id: &str,
    result: Result<ExecutionSummary, CommandFailure>,
) {
    let (command_state, detail, error_code, revision, typed_result) = match result {
        Ok(summary) if summary.changed => (
            CommandState::Succeeded,
            summary.detail,
            None,
            summary.result_revision,
            summary.result,
        ),
        Ok(summary) => (
            CommandState::NoOp,
            summary.detail,
            None,
            summary.result_revision,
            summary.result,
        ),
        // A refusal settled no evidence: it cancelled nothing and dequeued
        // nothing, so it must not carry a settlement result that would read as
        // one.
        Err(failure) => (
            CommandState::Failed,
            Some(failure.message),
            Some(failure.error_code),
            failure.result_revision,
            None,
        ),
    };
    projection.complete_command(
        command_id,
        command_state,
        detail,
        error_code,
        revision,
        typed_result,
    );
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
