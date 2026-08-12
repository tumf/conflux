//! `cflx client wait` — an observer, not a workflow engine.
//!
//! # The only interesting question
//!
//! "Is it done?" has one honest answer and several tempting wrong ones. The
//! wrong ones are all forms of trusting the owner's presentation: the change's
//! `display_status` reached `merged`; the change disappeared from the snapshot;
//! a command record settled successfully. Each of those describes what the owner
//! believes or did, and none of them survives the owner being killed halfway
//! through, restarted, or simply wrong.
//!
//! The honest answer comes from the repository, and *which* repository evidence
//! counts depends on how this owner finishes work — merge to base, publish base,
//! or push the change branch. That is exactly what the owner execution contract
//! publishes, which is why this command reads it before it starts observing and
//! refuses to wait at all if the owner never published one. Waiting for a
//! completion you cannot recognize is not waiting; it is timing out slowly.
//!
//! # What it never does
//!
//! No start, retry, queue, resolve, archive, merge, cleanup, or worktree
//! command — not on error, not on a blocked change, not on timeout. If the work
//! needs a push, that is an operator's decision, and a waiter that quietly made
//! it would be an owner wearing a client's name.

use std::time::Duration;

use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::repo::{self, Verdict};
use crate::client::session::{observe, Connection, Observation};
use crate::client::transport::Wake;
use crate::web::remote_control_api::dto::OwnerExecutionContract;

/// Longest gap between authoritative rehydrations.
///
/// The event stream normally wakes the loop sooner; this is the recovery
/// cadence for a missed frame, a dropped stream, or an owner that changes
/// repository state without publishing an event this client recognizes.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Delay before retrying after a transient failure to observe.
const RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Display statuses that claim the change reached a terminal success.
///
/// They trigger *verification*, never a success claim of their own.
const CLAIMED_SUCCESS_STATUSES: [&str; 3] = ["archived", "merged", "pushed"];

/// Observe one change until verified completion, a typed failure, or timeout.
pub async fn run(connection: &Connection, change_id: &str, timeout: Duration) -> ResultEnvelope {
    let deadline = tokio::time::Instant::now() + timeout;

    let initial = match observe(connection, Some(change_id)).await {
        Ok(initial) => initial,
        Err(error) => return error.into_envelope(Operation::Wait).with_change(change_id),
    };
    let instance_id = initial.instance_id.clone();

    let Some(contract) = initial.contract.contract.clone() else {
        return ResultEnvelope::new(Operation::Wait, Outcome::UnsupportedTerminalMode)
            .with_change(change_id)
            .with_instance(Some(instance_id))
            .with_message(
                "the owner published no execution contract, so this client cannot tell what would \
                 prove the change finished. Waiting without a terminal mode could only end in a \
                 timeout",
            );
    };

    let Some(repo_root) = connection.repo_root().map(|root| root.to_path_buf()) else {
        return ResultEnvelope::new(Operation::Wait, Outcome::NotInRepository)
            .with_change(change_id)
            .with_instance(Some(instance_id))
            .with_message(
                "completion is certified from repository evidence, so `wait` must run inside the \
                 owner's Git repository",
            );
    };

    let mut observation = initial;
    loop {
        match evaluate(&observation, &instance_id, change_id, &repo_root, &contract).await {
            Step::Settled(envelope) => return *envelope,
            Step::KeepObserving { detail } => {
                if tokio::time::Instant::now() >= deadline {
                    return timeout_envelope(change_id, &instance_id, timeout, detail);
                }
            }
        }

        // Wake on published activity, and fall back to the poll cadence. The
        // budget never outlives the caller's deadline, so a quiet owner cannot
        // stretch the wait past what was asked for.
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return timeout_envelope(change_id, &instance_id, timeout, None);
        }
        let budget = POLL_INTERVAL.min(remaining);
        let wake = connection
            .client()
            .wake_on_activity(observation.event_sequence, &instance_id, budget)
            .await;
        // Every wake reason leads to the same action — rehydrate every
        // authoritative resource — because a gap and an ordinary advance are
        // indistinguishable once the snapshot is re-read in full.
        debug_assert!(matches!(wake, Wake::Activity | Wake::Gap | Wake::Idle));

        observation = loop {
            match observe(connection, Some(change_id)).await {
                Ok(next) => break next,
                Err(error) if error.is_transient() => {
                    if tokio::time::Instant::now() >= deadline {
                        return timeout_envelope(
                            change_id,
                            &instance_id,
                            timeout,
                            Some(error.message().to_string()),
                        );
                    }
                    tokio::time::sleep(RETRY_INTERVAL).await;
                }
                // A socket that stopped answering mid-wait is an owner that is
                // gone. It is never completion: the repository has to say so.
                Err(error) => {
                    let verdict = repo::verify(change_id, &repo_root, &contract).await;
                    if let Verdict::Completed { evidence } = verdict {
                        return completed_envelope(change_id, &instance_id, &contract, evidence);
                    }
                    return error.into_envelope(Operation::Wait).with_change(change_id);
                }
            }
        };
    }
}

/// One evaluation of the current observation.
enum Step {
    /// A terminal answer was reached.
    Settled(Box<ResultEnvelope>),
    /// Nothing terminal yet; `detail` explains what is still missing.
    KeepObserving { detail: Option<String> },
}

impl Step {
    fn settled(envelope: ResultEnvelope) -> Self {
        Self::Settled(Box::new(envelope))
    }
}

async fn evaluate(
    observation: &Observation,
    instance_id: &str,
    change_id: &str,
    repo_root: &std::path::Path,
    contract: &OwnerExecutionContract,
) -> Step {
    // Owner replacement first: everything below would otherwise be read from a
    // process that never saw the work this wait is about.
    if observation.instance_id != instance_id {
        let verdict = repo::verify(change_id, repo_root, contract).await;
        return match verdict {
            // Repository evidence alone is enough, and it is the *only* thing
            // that is: the new incarnation cannot vouch for the old one's
            // in-flight commands.
            Verdict::Completed { evidence } => Step::settled(completed_envelope(
                change_id,
                instance_id,
                contract,
                evidence,
            )),
            _ => Step::settled(
                ResultEnvelope::new(Operation::Wait, Outcome::OwnerRestarted)
                    .with_change(change_id)
                    .with_instance(Some(observation.instance_id.clone()))
                    .with_message(
                        "the socket began serving a different owner incarnation and current \
                         repository evidence does not prove completion on its own",
                    )
                    .with_detail(serde_json::json!({
                        "expected_instance_id": instance_id,
                        "observed_instance_id": observation.instance_id,
                    })),
            ),
        };
    }

    if let Some(process_error) = observation.state.snapshot.process_error.clone() {
        return Step::settled(
            ResultEnvelope::new(Operation::Wait, Outcome::ProcessFailed)
                .with_change(change_id)
                .with_instance(Some(instance_id.to_string()))
                .with_message(format!("the owner reported a fatal error: {process_error}")),
        );
    }

    let change = observation.change(change_id);

    if change.is_some_and(|change| change.display_status == "rejected") {
        let detail = change
            .and_then(|change| change.error_detail.clone())
            .unwrap_or_else(|| "the change was rejected".to_string());
        return Step::settled(
            ResultEnvelope::new(Operation::Wait, Outcome::ChangeRejected)
                .with_change(change_id)
                .with_instance(Some(instance_id.to_string()))
                .with_message(detail),
        );
    }

    // Verification is not free, so it runs when the owner claims a terminal
    // success or when the change stopped being tracked at all. Disappearance is
    // the case that matters: it proves nothing on its own, and treating it as
    // success is the exact bug this contract exists to prevent.
    let claims_success = match change {
        Some(change) => CLAIMED_SUCCESS_STATUSES.contains(&change.display_status.as_str()),
        None => true,
    };
    if !claims_success {
        return Step::KeepObserving {
            detail: change.map(|change| {
                format!(
                    "'{change_id}' is at status '{}' with no terminal evidence yet",
                    change.display_status
                )
            }),
        };
    }

    match repo::verify(change_id, repo_root, contract).await {
        Verdict::Completed { evidence } => Step::settled(completed_envelope(
            change_id,
            instance_id,
            contract,
            evidence,
        )),
        Verdict::NotCompleted { detail } => Step::KeepObserving {
            detail: Some(detail),
        },
        Verdict::Broken { detail } => Step::settled(
            ResultEnvelope::new(Operation::Wait, Outcome::EvidenceError)
                .with_change(change_id)
                .with_instance(Some(instance_id.to_string()))
                .with_message(format!(
                    "repository completion evidence for '{change_id}' is unusable: {detail}"
                ))
                .with_detail(contract_detail(contract)),
        ),
        Verdict::Unsupported { detail } => Step::settled(
            ResultEnvelope::new(Operation::Wait, Outcome::UnsupportedTerminalMode)
                .with_change(change_id)
                .with_instance(Some(instance_id.to_string()))
                .with_message(detail)
                .with_detail(contract_detail(contract)),
        ),
    }
}

fn completed_envelope(
    change_id: &str,
    instance_id: &str,
    contract: &OwnerExecutionContract,
    evidence: String,
) -> ResultEnvelope {
    let mut detail = contract_detail(contract);
    if let Some(object) = detail.as_object_mut() {
        object.insert(
            "evidence".to_string(),
            serde_json::Value::String(evidence.clone()),
        );
    }
    ResultEnvelope::new(Operation::Wait, Outcome::Completed)
        .with_change(change_id)
        .with_instance(Some(instance_id.to_string()))
        .with_message(evidence)
        .with_detail(detail)
}

fn contract_detail(contract: &OwnerExecutionContract) -> serde_json::Value {
    serde_json::json!({
        "terminal_mode": contract.terminal_mode,
        "base_branch": contract.base_branch,
        "remote": contract.remote,
        "pushed_branch": contract.pushed_branch,
        "commands_submitted": 0,
    })
}

fn timeout_envelope(
    change_id: &str,
    instance_id: &str,
    timeout: Duration,
    detail: Option<String>,
) -> ResultEnvelope {
    let mut message = format!(
        "no verified terminal outcome for '{change_id}' within {}ms",
        timeout.as_millis()
    );
    if let Some(detail) = detail {
        message.push_str(&format!("; {detail}"));
    }
    ResultEnvelope::new(Operation::Wait, Outcome::Timeout)
        .with_change(change_id)
        .with_instance(Some(instance_id.to_string()))
        .with_message(message)
        .with_detail(serde_json::json!({ "commands_submitted": 0 }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::remote_control_api::dto::TerminalMode;

    fn contract() -> OwnerExecutionContract {
        OwnerExecutionContract {
            base_branch: "main".to_string(),
            terminal_mode: TerminalMode::Merged,
            remote: None,
            pushed_branch: None,
        }
    }

    #[test]
    fn a_completion_envelope_carries_its_evidence_and_no_command_count() {
        let envelope = completed_envelope("alpha", "i-1", &contract(), "proof".to_string());
        assert_eq!(envelope.outcome, Outcome::Completed);
        assert!(envelope.ok);
        assert_eq!(envelope.detail["evidence"], "proof");
        assert_eq!(envelope.detail["terminal_mode"], "merged");
        assert_eq!(envelope.detail["commands_submitted"], 0);
    }

    #[test]
    fn a_timeout_reports_the_budget_and_the_missing_evidence() {
        let envelope = timeout_envelope(
            "alpha",
            "i-1",
            Duration::from_secs(90),
            Some("no archive entry".to_string()),
        );
        assert_eq!(envelope.outcome, Outcome::Timeout);
        assert!(!envelope.ok);
        let message = envelope.message.unwrap();
        assert!(message.contains("within 90000ms"), "{message}");
        assert!(message.contains("no archive entry"), "{message}");
        assert_eq!(envelope.detail["commands_submitted"], 0);
    }

    #[test]
    fn only_post_archive_statuses_claim_a_success_worth_verifying() {
        assert_eq!(CLAIMED_SUCCESS_STATUSES, ["archived", "merged", "pushed"]);
        // `rejected` is terminal but is not a success claim, so it must never
        // reach repository verification.
        assert!(!CLAIMED_SUCCESS_STATUSES.contains(&"rejected"));
    }
}
