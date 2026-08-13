//! Attaching, reading, and clearing one execution's completion sink.
//!
//! # Why the client checks the incarnation itself
//!
//! The owner checks `(instance_id, execution_id, change_id)` and refuses a
//! mismatch — but a *restarted* owner has never heard of the execution at all,
//! so it answers the same "no such execution" a genuinely stale ID gets. Those
//! two need different reactions, so a caller holding the incarnation an enqueue
//! reported passes it here and the mismatch is reported as `owner_restarted`
//! before anything is submitted. The instance the owner is *sent* still comes
//! from this call's own observation, never from the caller.
//!
//! # Why capability comes first
//!
//! An owner that predates execution sinks has no such route. Discovering that
//! from a 404 would be indistinguishable from "that execution is gone", so the
//! published capability is read first and an unsupporting owner is reported as
//! `incompatible_owner` before anything is submitted.
//!
//! Nothing here submits a workflow command. A sink registration creates no
//! command record, advances no revision, and cannot move a change — which is
//! exactly why it is not routed through the command endpoint.

use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::session::{
    describe_api_error, observe_bounded, Connection, MAX_RECONCILE_ATTEMPTS,
};
use crate::client::transport::{encode_query_value, HttpResponse, TransportError};
use crate::web::remote_control_api::dto::{
    ApiError, ErrorCode, ExecutionSinkRequest, ExecutionSinkResponse,
};

/// What the caller asked to do with one execution's sink.
#[derive(Debug, Clone)]
pub enum Intent {
    /// Attach or replace the sink.
    Set {
        /// Callback argv, executed directly and never as shell source.
        command: Vec<String>,
        /// Opt in to the non-terminal blocked attention edge.
        notify_blocked: bool,
    },
    /// Read the current sink.
    Get,
    /// Detach the sink.
    Clear,
}

impl Intent {
    fn operation(&self) -> Operation {
        match self {
            Self::Set { .. } => Operation::NotifySet,
            Self::Get => Operation::NotifyGet,
            Self::Clear => Operation::NotifyClear,
        }
    }
}

/// Run one notify intent against an existing owner.
///
/// `expected_instance` is the incarnation the caller believes the execution
/// belongs to, when it is holding one from an earlier enqueue. Checking it here
/// is what turns an owner restart into the typed `owner_restarted` the contract
/// promises, instead of the `execution_not_found` a new incarnation would
/// otherwise answer with — the same refusal a genuinely stale execution ID gets,
/// and a caller that conflated them would retry forever against a process that
/// never saw the work.
pub async fn run(
    connection: &Connection,
    change_id: &str,
    execution_id: &str,
    expected_instance: Option<&str>,
    intent: Intent,
) -> ResultEnvelope {
    let operation = intent.operation();

    let observation =
        match observe_bounded(connection, Some(change_id), MAX_RECONCILE_ATTEMPTS).await {
            Ok(observation) => observation,
            Err(error) => {
                return error
                    .into_envelope(operation)
                    .with_change(change_id)
                    .with_execution(Some(execution_id.to_string()))
            }
        };
    let instance_id = observation.instance_id.clone();

    if let Some(expected) = expected_instance {
        if expected != instance_id {
            return ResultEnvelope::new(operation, Outcome::OwnerRestarted)
                .with_change(change_id)
                .with_instance(Some(instance_id.clone()))
                .with_execution(Some(execution_id.to_string()))
                .with_message(
                    "the socket is serving a different owner incarnation, so this \
                     execution binding no longer exists. It is not completion: nothing \
                     about the change was observed",
                )
                .with_detail(serde_json::json!({
                    "expected_instance_id": expected,
                    "observed_instance_id": instance_id,
                }));
        }
    }

    if !observation.execution_sinks_available() {
        return ResultEnvelope::new(operation, Outcome::IncompatibleOwner)
            .with_change(change_id)
            .with_instance(Some(instance_id))
            .with_execution(Some(execution_id.to_string()))
            .with_message(
                "this owner does not serve execution-scoped completion sinks, so no callback \
                 can be registered with it. Observe completion with `wait` instead",
            );
    }

    let path = format!(
        "/api/v2/executions/{}/sink",
        encode_query_value(execution_id)
    );
    let binding = format!(
        "instance_id={}&change_id={}",
        encode_query_value(&instance_id),
        encode_query_value(change_id)
    );

    let response = match &intent {
        Intent::Set {
            command,
            notify_blocked,
        } => {
            let request = ExecutionSinkRequest {
                instance_id: instance_id.clone(),
                change_id: change_id.to_string(),
                command: command.clone(),
                notify_blocked: *notify_blocked,
            };
            match serde_json::to_string(&request) {
                Ok(body) => connection.client().put_json(&path, &body).await,
                Err(error) => {
                    return ResultEnvelope::new(operation, Outcome::UsageError)
                        .with_change(change_id)
                        .with_instance(Some(instance_id))
                        .with_execution(Some(execution_id.to_string()))
                        .with_message(format!("the sink request is not encodable: {error}"))
                }
            }
        }
        Intent::Get => connection.client().get(&format!("{path}?{binding}")).await,
        Intent::Clear => {
            connection
                .client()
                .delete(&format!("{path}?{binding}"))
                .await
        }
    };

    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return transport_envelope(error, operation, change_id, execution_id, &instance_id)
        }
    };

    classify(response, operation, change_id, execution_id, &instance_id)
}

fn transport_envelope(
    error: TransportError,
    operation: Operation,
    change_id: &str,
    execution_id: &str,
    instance_id: &str,
) -> ResultEnvelope {
    let outcome = match error {
        TransportError::NotListening { .. } => Outcome::OwnerNotRunning,
        _ => Outcome::TransportError,
    };
    ResultEnvelope::new(operation, outcome)
        .with_change(change_id)
        .with_instance(Some(instance_id.to_string()))
        .with_execution(Some(execution_id.to_string()))
        .with_message(error.to_string())
}

/// Map one owner response onto the stable client outcome vocabulary.
///
/// Branching on the typed `error_code` rather than on the HTTP status, because
/// two different refusals share status 409 and a caller that keyed on the status
/// would conflate "not your execution" with an ordinary lifecycle conflict.
fn classify(
    response: HttpResponse,
    operation: Operation,
    change_id: &str,
    execution_id: &str,
    instance_id: &str,
) -> ResultEnvelope {
    let envelope = |outcome: Outcome| {
        ResultEnvelope::new(operation, outcome)
            .with_change(change_id)
            .with_instance(Some(instance_id.to_string()))
            .with_execution(Some(execution_id.to_string()))
    };

    if response.status == 200 {
        return match response.json::<ExecutionSinkResponse>() {
            Ok(body) => {
                // The owner answers with its own identity. A different one means
                // the socket started serving another incarnation between the
                // observation and this request, and the registration it just
                // accepted belongs to a process this caller never observed.
                if body.instance_id != instance_id {
                    return envelope(Outcome::OwnerRestarted).with_message(
                        "the socket began serving a different owner incarnation while the \
                         subscription was being registered",
                    );
                }
                let detail = serde_json::to_value(&body).unwrap_or_else(|_| serde_json::json!({}));
                envelope(Outcome::Subscribed)
                    .with_message(match &body.sink {
                        Some(_) => "the owner holds a completion sink for this execution",
                        None => "this execution has no completion sink",
                    })
                    .with_detail(detail)
            }
            Err(error) => envelope(Outcome::IncompatibleOwner).with_message(format!(
                "the owner's sink response did not match this build's contract: {error}"
            )),
        };
    }

    let code = serde_json::from_slice::<ApiError>(&response.body).map(|error| error.error_code);
    let message = describe_api_error(&response.body, "the owner refused the sink operation");

    let outcome = match (response.status, code) {
        (401 | 403, Ok(ErrorCode::TransportNotPermitted)) => Outcome::TransportNotPermitted,
        (401 | 403, _) => Outcome::AuthenticationFailed,
        (404, Ok(ErrorCode::NotFound)) => Outcome::ExecutionNotFound,
        // A 404 with no typed body is a route this owner does not serve, which
        // is an owner-compatibility fact rather than a missing execution.
        (404, _) => Outcome::IncompatibleOwner,
        (409, Ok(ErrorCode::ExecutionBindingMismatch)) => Outcome::ExecutionBindingMismatch,
        (409, _) => Outcome::TargetIneligible,
        (422, _) => Outcome::UsageError,
        (503, _) => Outcome::OwnerNotCommandCapable,
        _ => Outcome::TransportError,
    };
    envelope(outcome).with_message(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, body: serde_json::Value) -> HttpResponse {
        HttpResponse {
            status,
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    fn api_error(code: &str) -> serde_json::Value {
        serde_json::json!({
            "error_code": code,
            "message": "refused",
            "correlation_id": "abc",
        })
    }

    #[test]
    fn a_typed_refusal_maps_onto_its_own_stable_outcome() {
        let cases = [
            (
                403,
                "transport_not_permitted",
                Outcome::TransportNotPermitted,
            ),
            (403, "forbidden", Outcome::AuthenticationFailed),
            (401, "unauthorized", Outcome::AuthenticationFailed),
            (404, "not_found", Outcome::ExecutionNotFound),
            (
                409,
                "execution_binding_mismatch",
                Outcome::ExecutionBindingMismatch,
            ),
            (422, "validation_failed", Outcome::UsageError),
        ];
        for (status, code, expected) in cases {
            let envelope = classify(
                response(status, api_error(code)),
                Operation::NotifySet,
                "alpha",
                "e-1",
                "i-1",
            );
            assert_eq!(envelope.outcome, expected, "{status} {code}");
            assert!(!envelope.ok);
            assert_eq!(envelope.change_id.as_deref(), Some("alpha"));
            assert_eq!(envelope.execution_id.as_deref(), Some("e-1"));
        }
    }

    /// A 404 without a typed body is an owner that has no such route at all.
    #[test]
    fn an_untyped_not_found_is_an_incompatible_owner_rather_than_a_lost_execution() {
        let envelope = classify(
            HttpResponse {
                status: 404,
                body: b"<html>nope</html>".to_vec(),
            },
            Operation::NotifyGet,
            "alpha",
            "e-1",
            "i-1",
        );
        assert_eq!(envelope.outcome, Outcome::IncompatibleOwner);
    }

    #[test]
    fn a_successful_registration_reports_one_success_token_and_carries_the_binding() {
        let body = serde_json::json!({
            "instance_id": "i-1",
            "execution_id": "e-1",
            "change_id": "alpha",
            "sink": {"command": ["/bin/true"], "notify_blocked": false},
            "execution_state": "active",
            "terminal_dispatched": false,
            "delivered_events": [],
        });
        let envelope = classify(
            response(200, body),
            Operation::NotifySet,
            "alpha",
            "e-1",
            "i-1",
        );
        assert_eq!(envelope.outcome, Outcome::Subscribed);
        assert!(envelope.ok);
        assert_eq!(envelope.exit_code(), 0);
        assert_eq!(envelope.detail["sink"]["command"][0], "/bin/true");
        assert_eq!(envelope.detail["terminal_dispatched"], false);
    }

    /// The owner answering as a different incarnation is a restart, not a
    /// successful subscription: the registration it accepted belongs to a
    /// process this caller never observed.
    #[test]
    fn an_answer_from_another_incarnation_is_reported_as_a_restart() {
        let body = serde_json::json!({
            "instance_id": "i-2",
            "execution_id": "e-1",
            "change_id": "alpha",
            "sink": serde_json::Value::Null,
            "execution_state": "unknown",
            "terminal_dispatched": false,
            "delivered_events": [],
        });
        let envelope = classify(
            response(200, body),
            Operation::NotifySet,
            "alpha",
            "e-1",
            "i-1",
        );
        assert_eq!(envelope.outcome, Outcome::OwnerRestarted);
        assert!(!envelope.ok);
    }
}
