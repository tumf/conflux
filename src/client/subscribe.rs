//! Registering, reading, and clearing proposal-scoped completion subscriptions.
//!
//! # Why the key is a proposal rather than an execution
//!
//! The execution-scoped sink needed an `execution_id`, and an execution ID does
//! not exist until the owner admits work. An agent that marks proposals and asks
//! for a Start therefore had nothing to name at the moment it wanted to say "tell
//! me when this finishes" — the gap the previous design closed by inferring a
//! registration from an admission result, which is exactly the implicit behavior
//! this change removes. A subscription keyed by the proposal can be registered
//! before, during, or after admission, and the owner binds each new execution
//! episode of that proposal to it.
//!
//! # Why the client checks the incarnation itself
//!
//! Subscriptions are process-local, so a caller holding one from a previous owner
//! is holding nothing. The owner checks the binding too, but checking it here —
//! against this call's own observation, before any mutation — is what turns an
//! owner restart into the typed `owner_restarted` a caller can act on rather
//! than a successful registration against a process that never saw its work.
//!
//! # Why capability comes first
//!
//! An owner that predates proposal subscriptions has no such route. Discovering
//! that from a 404 would be indistinguishable from an ordinary refusal, so the
//! published capability is read first and an owner without it is reported as
//! `unsupported_owner` before anything is submitted.
//!
//! # All or nothing, and what that can honestly mean
//!
//! Every check that can refuse the request — target count, distinctness, callback
//! argv shape, owner reachability, owner incarnation, owner capability — is
//! completed against one coherent observation *before* the first proposal is
//! touched. So a validation failure really does leave every named proposal
//! untouched. What cannot be undone is a transport fault partway through: the
//! registrations that settled are real, and this reports `partial_intent` naming
//! exactly them rather than claiming a rollback that never happened.
//!
//! Nothing here submits a workflow command. A subscription creates no command
//! record, advances no revision, and cannot move a proposal — which is exactly
//! why it is not routed through the command endpoint.

use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::session::{
    describe_api_error, observe_bounded, Connection, MAX_RECONCILE_ATTEMPTS,
};
use crate::client::transport::{encode_query_value, HttpResponse, TransportError};
use crate::web::remote_control_api::dto::{
    ApiError, ErrorCode, ProposalSubscriptionRequest, ProposalSubscriptionResponse,
};

/// What the caller asked to do with the named proposals' subscriptions.
#[derive(Debug, Clone)]
pub enum Intent {
    /// Register or replace the subscription.
    Set {
        /// Callback argv, executed directly and never as shell source.
        command: Vec<String>,
        /// Opt in to the non-terminal blocked attention edge.
        notify_blocked: bool,
    },
    /// Read the current subscription.
    Get,
    /// Remove the subscription.
    Clear,
}

impl Intent {
    /// The operation an envelope reports for this intent.
    pub fn operation(&self) -> Operation {
        match self {
            Self::Set { .. } => Operation::SubscribeSet,
            Self::Get => Operation::SubscribeGet,
            Self::Clear => Operation::SubscribeClear,
        }
    }

    /// Wire spelling accepted by `cflx_subscribe`.
    pub fn action(&self) -> &'static str {
        match self {
            Self::Set { .. } => "set",
            Self::Get => "get",
            Self::Clear => "clear",
        }
    }

    /// The success token this intent reports.
    ///
    /// `clear` gets its own because "there is a subscription now" and "there is
    /// not" are the two answers a caller acts on differently, and one shared
    /// token would make them read the same.
    fn success(&self) -> Outcome {
        match self {
            Self::Clear => Outcome::Cleared,
            _ => Outcome::Subscribed,
        }
    }
}

/// Reject one whole subscription request from its arguments alone.
///
/// Pure: no filesystem, no socket, no owner. That is what makes "a validation
/// failure for any requested proposal or callback causes no requested mutation"
/// a property of the code rather than a promise about ordering.
pub fn validate_request(change_ids: &[String], intent: &Intent) -> Result<(), String> {
    crate::client::control::validate_targets(change_ids)?;
    match intent {
        Intent::Set { command, .. } => {
            crate::web::completion_sink::validate_command(command).map_err(
                |refusal| match refusal {
                    crate::web::completion_sink::SinkRefusal::InvalidCommand(detail) => detail,
                    // The other variants describe an execution binding, which a
                    // pure argv check never inspects.
                    other => format!("the callback argv is not acceptable: {other:?}"),
                },
            )
        }
        // A read and a removal carry no argv at all; accepting one would be
        // storing a command for an operation that never runs it.
        Intent::Get | Intent::Clear => Ok(()),
    }
}

/// Run one subscription intent against an existing owner.
///
/// `expected_instance` is the incarnation the caller believes it is addressing,
/// when it kept one. Supplying it is what turns an owner restart into the typed
/// `owner_restarted` a caller can act on; omitting it means the caller had
/// nothing to remember, and the observed incarnation is used, which is honest
/// rather than convenient — there is no earlier owner for it to disagree with.
/// The `cflx client subscribe` verbs require it, because a shell caller that
/// registered a callback did observe one.
pub async fn run(
    connection: &Connection,
    change_ids: &[String],
    expected_instance: Option<&str>,
    intent: Intent,
) -> ResultEnvelope {
    let operation = intent.operation();
    if let Err(message) = validate_request(change_ids, &intent) {
        return ResultEnvelope::new(operation, Outcome::UsageError).with_message(message);
    }

    let observation = match observe_bounded(connection, None, MAX_RECONCILE_ATTEMPTS).await {
        Ok(observation) => observation,
        Err(error) => return error.into_envelope(operation),
    };
    let instance_id = observation.instance_id.clone();

    if let Some(expected) = expected_instance {
        if expected != instance_id {
            return ResultEnvelope::new(operation, Outcome::OwnerRestarted)
                .with_instance(Some(instance_id.clone()))
                .with_message(
                    "the socket is serving a different owner incarnation, so subscriptions \
                     registered with the one you named no longer exist. It is not completion: \
                     nothing about any proposal was observed",
                )
                .with_detail(serde_json::json!({
                    "expected_instance_id": expected,
                    "observed_instance_id": instance_id,
                }));
        }
    }

    if !observation.proposal_subscriptions_available() {
        return ResultEnvelope::new(operation, Outcome::UnsupportedOwner)
            .with_instance(Some(instance_id))
            .with_message(
                "this owner does not serve proposal-scoped subscriptions, so no callback can be \
                 registered with it. Observe completion with `cflx client wait` instead",
            );
    }

    let mut settled: Vec<serde_json::Value> = Vec::new();
    // Only meaningful for a single-target request: with several proposals in
    // flight there is no one episode the envelope's scalar field could name.
    let mut episode: Option<String> = None;
    for change_id in change_ids {
        let response = request(connection, change_id, &instance_id, &intent).await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                let outcome = match error {
                    TransportError::NotListening { .. } => Outcome::OwnerNotRunning,
                    _ => Outcome::TransportError,
                };
                return stop(
                    operation,
                    &instance_id,
                    change_id,
                    &settled,
                    outcome,
                    error.to_string(),
                );
            }
        };
        match classify(response, &instance_id) {
            Ok(body) => {
                episode = body.execution_id.clone();
                settled.push(serde_json::to_value(&body).unwrap_or_default())
            }
            Err(refusal) => {
                return stop(
                    operation,
                    &instance_id,
                    change_id,
                    &settled,
                    refusal.outcome,
                    refusal.message,
                )
            }
        }
    }

    let envelope = ResultEnvelope::new(operation, intent.success())
        .with_instance(Some(instance_id))
        .with_message(match &intent {
            Intent::Set { .. } => format!(
                "{} proposal subscriptions are registered; delivery is notification only and \
                 resumes no agent",
                settled.len()
            ),
            Intent::Get => format!("{} proposal subscriptions were read", settled.len()),
            Intent::Clear => format!(
                "{} proposal subscriptions were removed; a callback already running keeps its \
                 own bounds",
                settled.len()
            ),
        })
        .with_detail(serde_json::json!({
            "action": intent.action(),
            "subscriptions": settled,
        }));
    match change_ids {
        [only] => envelope.with_change(only.clone()).with_execution(episode),
        _ => envelope,
    }
}

/// Stop a multi-proposal request, distinguishing "nothing happened" from
/// "some of it did".
///
/// A refusal on the *first* proposal is the whole request refused: no mutation
/// exists, so reporting the typed reason is the honest answer. A refusal after
/// one settled is partial intent — the registrations that exist are real, and
/// removing them to tidy up would be this client cancelling a subscription it
/// was never asked to cancel.
fn stop(
    operation: Operation,
    instance_id: &str,
    change_id: &str,
    settled: &[serde_json::Value],
    outcome: Outcome,
    message: String,
) -> ResultEnvelope {
    if settled.is_empty() {
        return ResultEnvelope::new(operation, outcome)
            .with_instance(Some(instance_id.to_string()))
            .with_change(change_id)
            .with_message(message)
            .with_detail(serde_json::json!({ "subscriptions": settled }));
    }
    ResultEnvelope::new(operation, Outcome::PartialIntent)
        .with_instance(Some(instance_id.to_string()))
        .with_change(change_id)
        .with_message(format!(
            "{} proposals settled before '{change_id}' stopped the request: {message}. Nothing \
             was rolled back",
            settled.len()
        ))
        .with_detail(serde_json::json!({
            "subscriptions": settled,
            "stopped_at": change_id,
            "rolled_back": false,
        }))
}

async fn request(
    connection: &Connection,
    change_id: &str,
    instance_id: &str,
    intent: &Intent,
) -> Result<HttpResponse, TransportError> {
    let path = format!(
        "/api/v2/proposals/{}/subscription",
        encode_query_value(change_id)
    );
    let binding = format!("instance_id={}", encode_query_value(instance_id));
    match intent {
        Intent::Set {
            command,
            notify_blocked,
        } => {
            let body = serde_json::to_string(&ProposalSubscriptionRequest {
                instance_id: instance_id.to_string(),
                command: command.clone(),
                notify_blocked: *notify_blocked,
            })
            .expect("a closed struct of owned values is encodable");
            connection.client().put_json(&path, &body).await
        }
        Intent::Get => connection.client().get(&format!("{path}?{binding}")).await,
        Intent::Clear => {
            connection
                .client()
                .delete(&format!("{path}?{binding}"))
                .await
        }
    }
}

/// Why one proposal's request did not succeed.
#[derive(Debug)]
struct Refusal {
    outcome: Outcome,
    message: String,
}

/// Map one owner response onto the stable client outcome vocabulary.
///
/// Branching on the typed `error_code` rather than on the HTTP status, because
/// two different refusals share status 409 and a caller that keyed on the status
/// would conflate "the owner was replaced" with an ordinary lifecycle conflict.
fn classify(
    response: HttpResponse,
    instance_id: &str,
) -> Result<ProposalSubscriptionResponse, Refusal> {
    if response.status == 200 {
        return match response.json::<ProposalSubscriptionResponse>() {
            Ok(body) => {
                // The owner answers with its own identity. A different one means
                // the socket started serving another incarnation between the
                // observation and this request, and whatever it just accepted
                // belongs to a process this caller never observed.
                if body.instance_id != instance_id {
                    return Err(Refusal {
                        outcome: Outcome::OwnerRestarted,
                        message: "the socket began serving a different owner incarnation while \
                                  the subscription was being registered"
                            .to_string(),
                    });
                }
                Ok(body)
            }
            Err(error) => Err(Refusal {
                outcome: Outcome::IncompatibleOwner,
                message: format!(
                    "the owner's subscription response did not match this build's contract: \
                     {error}"
                ),
            }),
        };
    }

    let code = serde_json::from_slice::<ApiError>(&response.body).map(|error| error.error_code);
    let message = describe_api_error(&response.body, "the owner refused the subscription request");
    let outcome = match (response.status, code) {
        (401 | 403, Ok(ErrorCode::TransportNotPermitted)) => Outcome::TransportNotPermitted,
        (401 | 403, _) => Outcome::AuthenticationFailed,
        // A 404 on this route is an owner that does not serve it at all: the
        // resource is keyed by a change ID the owner never has to know.
        (404, _) => Outcome::UnsupportedOwner,
        // The only binding a proposal subscription has is the owner incarnation,
        // so a binding mismatch here means the socket changed hands.
        (409, Ok(ErrorCode::ExecutionBindingMismatch)) => Outcome::OwnerRestarted,
        (409, _) => Outcome::TargetIneligible,
        (422, _) => Outcome::UsageError,
        (503, _) => Outcome::UnsupportedOwner,
        _ => Outcome::TransportError,
    };
    Err(Refusal { outcome, message })
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

    fn set(command: &[&str]) -> Intent {
        Intent::Set {
            command: command.iter().map(|part| part.to_string()).collect(),
            notify_blocked: false,
        }
    }

    #[test]
    fn each_intent_reports_its_own_operation_and_success_token() {
        assert_eq!(Intent::Get.operation(), Operation::SubscribeGet);
        assert_eq!(Intent::Get.success(), Outcome::Subscribed);
        assert_eq!(set(&["/bin/true"]).operation(), Operation::SubscribeSet);
        assert_eq!(set(&["/bin/true"]).success(), Outcome::Subscribed);
        assert_eq!(Intent::Clear.operation(), Operation::SubscribeClear);
        assert_eq!(Intent::Clear.success(), Outcome::Cleared);
        assert_eq!(Intent::Clear.action(), "clear");
    }

    /// Every refusal a request can earn from its own arguments happens before
    /// an owner exists, which is what "no requested mutation" means for a list
    /// of proposals that would otherwise be mutated one at a time.
    #[test]
    fn a_request_is_validated_completely_before_any_owner_is_contacted() {
        let targets = ["alpha".to_string(), "beta".to_string()];
        assert!(validate_request(&targets, &set(&["/absolute/callback", "--flag"])).is_ok());

        assert!(validate_request(&[], &Intent::Get).is_err());
        let duplicated = ["alpha", "alpha"].map(str::to_string).to_vec();
        assert!(validate_request(&duplicated, &Intent::Clear).is_err());

        let too_many: Vec<String> = (0..65).map(|n| format!("change-{n}")).collect();
        assert!(validate_request(&too_many, &Intent::Get).is_err());

        // An empty callback is refused here rather than stored and discovered
        // at delivery time, when nothing could be reported to anybody.
        let empty = validate_request(&targets, &set(&[])).expect_err("an empty argv is refused");
        assert!(empty.contains("program"), "{empty}");

        // Read and clear carry no argv, so nothing about one can be invalid.
        assert!(validate_request(&targets, &Intent::Get).is_ok());
        assert!(validate_request(&targets, &Intent::Clear).is_ok());
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
            (404, "not_found", Outcome::UnsupportedOwner),
            (409, "execution_binding_mismatch", Outcome::OwnerRestarted),
            (422, "validation_failed", Outcome::UsageError),
            (503, "command_executor_unbound", Outcome::UnsupportedOwner),
        ];
        for (status, code, expected) in cases {
            let refusal =
                classify(response(status, api_error(code)), "i-1").expect_err("{status} {code}");
            assert_eq!(refusal.outcome, expected, "{status} {code}");
        }
    }

    #[test]
    fn a_successful_registration_carries_the_owner_binding_and_the_episode() {
        let body = serde_json::json!({
            "instance_id": "i-1",
            "change_id": "alpha",
            "sink": {"command": ["/bin/true"], "notify_blocked": false},
            "subscribed": true,
            "execution_id": "e-1",
            "execution_state": "active",
            "terminal_dispatched": false,
            "delivered_events": [],
        });
        let parsed = classify(response(200, body), "i-1").expect("a registered subscription");
        assert_eq!(parsed.change_id, "alpha");
        assert!(parsed.subscribed);
        assert_eq!(parsed.execution_id.as_deref(), Some("e-1"));
        assert_eq!(parsed.sink.unwrap().command, vec!["/bin/true".to_string()]);
    }

    /// A subscription registered before any admission is the case the whole
    /// resource exists for, so an absent episode has to be an ordinary answer
    /// rather than a refusal.
    #[test]
    fn a_subscription_before_admission_reports_no_episode_and_still_succeeds() {
        let body = serde_json::json!({
            "instance_id": "i-1",
            "change_id": "alpha",
            "sink": {"command": ["/bin/true"], "notify_blocked": true},
            "subscribed": true,
            "execution_state": "unknown",
            "terminal_dispatched": false,
            "delivered_events": [],
        });
        let parsed = classify(response(200, body), "i-1").expect("a pre-admission subscription");
        assert!(parsed.subscribed);
        assert_eq!(parsed.execution_id, None);
        assert!(!parsed.terminal_dispatched);
    }

    /// The owner answering as a different incarnation is a restart, not a
    /// successful subscription.
    #[test]
    fn an_answer_from_another_incarnation_is_reported_as_a_restart() {
        let body = serde_json::json!({
            "instance_id": "i-2",
            "change_id": "alpha",
            "sink": serde_json::Value::Null,
            "subscribed": false,
            "execution_state": "unknown",
            "terminal_dispatched": false,
            "delivered_events": [],
        });
        let refusal = classify(response(200, body), "i-1").expect_err("a replaced owner");
        assert_eq!(refusal.outcome, Outcome::OwnerRestarted);
    }

    /// A TCP read is told a subscription exists without being told the argv, and
    /// that redaction must not read as an absent subscription.
    #[test]
    fn a_redacted_read_still_reports_presence() {
        let body = serde_json::json!({
            "instance_id": "i-1",
            "change_id": "alpha",
            "sink": serde_json::Value::Null,
            "subscribed": true,
            "execution_state": "queued",
            "terminal_dispatched": false,
            "delivered_events": [],
        });
        let parsed = classify(response(200, body), "i-1").expect("a redacted read");
        assert!(parsed.subscribed, "presence is not the secret; the argv is");
        assert!(parsed.sink.is_none());
    }

    #[test]
    fn a_refusal_before_the_first_mutation_is_not_reported_as_partial() {
        let envelope = stop(
            Operation::SubscribeSet,
            "i-1",
            "alpha",
            &[],
            Outcome::TransportNotPermitted,
            "TCP may not register an argv".to_string(),
        );
        assert_eq!(envelope.outcome, Outcome::TransportNotPermitted);
        assert_eq!(envelope.change_id.as_deref(), Some("alpha"));
        assert!(envelope.detail["subscriptions"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_refusal_after_a_settled_registration_is_partial_and_claims_no_rollback() {
        let settled = vec![serde_json::json!({"change_id": "alpha", "subscribed": true})];
        let envelope = stop(
            Operation::SubscribeSet,
            "i-1",
            "beta",
            &settled,
            Outcome::TransportError,
            "the socket closed".to_string(),
        );
        assert_eq!(envelope.outcome, Outcome::PartialIntent);
        assert_eq!(envelope.detail["rolled_back"], false);
        assert_eq!(envelope.detail["stopped_at"], "beta");
        assert_eq!(envelope.detail["subscriptions"][0]["change_id"], "alpha");
    }
}
