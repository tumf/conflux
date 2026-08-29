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
//!
//! # Waiting for the owner, not for the operator
//!
//! Refusing to act on a parked change is not the same as refusing to *report*
//! one. `error`, `merge wait`, `stopped`, and `stalled` are rows this owner will
//! not advance by itself, so an observer sitting on them is waiting for a human
//! who does not know they are being waited for — the caller learns nothing, and
//! an unbounded wait learns nothing forever. Those release immediately with
//! `change_requires_action`, carrying the observed status so the caller can tell
//! a conflict from a failure. Everything the owner can still move on its own —
//! the active phases, `queued`, and any status a future owner adds — keeps
//! observing, because giving up on live work is the opposite mistake and the
//! more expensive one.
//!
//! `blocked` is on both sides of that line, and only its structured blocker says
//! which. A dependency wait clears when the owner archives what it is waiting
//! on, so it is live work. A validated *external* prerequisite is a hold the
//! owner already handed back to the operator, and the row will not move again
//! until someone retries it — so it releases with the same
//! `change_requires_action`, carrying the blocker's own prerequisite facts.
//!
//! # A target the owner never had
//!
//! Absence means two different things depending on when it happens. A change
//! that vanishes *mid-wait* may come back — the owner reprojects, an execution
//! is re-admitted — so the completion contract keeps observing it and refuses to
//! read the gap as either success or failure. A change that is missing from the
//! very first coherent observation has nothing to come back from: no row was
//! ever there to move, and an unbounded wait on a mistyped `change-id` would
//! otherwise never return at all. That absence releases as `change_not_found`,
//! the same typed refusal every other target-scoped client operation gives.
//!
//! The repository is still asked exactly once before the refusal, because the
//! most ordinary reason a proposal is missing from the snapshot is that it
//! already finished and was archived. Calling a completed change a typo would be
//! the worse of the two errors.
//!
//! # One deadline, not several budgets
//!
//! `--timeout D` is a promise about the whole operation, so exactly one
//! monotonic deadline is built when the wait begins and every step that can
//! block is bounded by what is left of it: the first observation, each reread,
//! the event-stream budget, repository classification, and every Git child.
//! Without that, the per-request transport valve and an unreachable Git remote
//! are each free to outlive `D` on their own, and the caller's timeout would
//! bound only the parts that were already fast. Expiry is an *operation*
//! outcome: whatever the inner step would have reported afterwards, the answer
//! is `timeout`, and nothing was submitted.
//!
//! # No deadline is the default
//!
//! Omitting `--timeout`, or spelling it `0`, means the caller wants the answer
//! rather than a report about the clock, so there is no operation deadline at
//! all and no `timeout` outcome to reach. That is a promise about the
//! *operation*, not about the processes it spawns: the transport keeps its
//! per-request valve, and every Git child gets a fresh finite budget of its own
//! ([`GIT_INVOCATION_BUDGET`]) — otherwise a single `git ls-remote` against an
//! unreachable remote would turn "wait as long as it takes" into "hang forever
//! on the first hop". A child that hits that budget is killed, reaped, and
//! retried on the next poll; it never borrows the `timeout` outcome that belongs
//! to callers who asked for a deadline.
//!
//! # What an expired deadline still owes the caller
//!
//! `timeout` is a statement about the clock, and on its own it is the least
//! useful answer this command can give: an agent that asked for 30 minutes and
//! got "no" cannot tell a change that is one phase from done from one the owner
//! never admitted, or from an owner that never answered at all. Each of those
//! needs a different next action, and re-reading the owner afterwards to find
//! out is both an extra round trip and a lie — it describes the world *after*
//! the deadline, not the wait that ran out.
//!
//! So the wait keeps the last thing it honestly knew. Every coherent
//! observation replaces a target-only projection of itself — the change's own
//! published row and its matching execution facts, at one reconciled revision —
//! and the timeout envelope reports that projection alongside the configured
//! budget, the measured elapsed time, and the [`TimeoutStage`] the deadline
//! landed in. Nothing is read after expiry to fill it in, and a wait that never
//! completed an observation says `null` rather than inventing an owner it never
//! reconciled with.

use std::future::Future;
use std::time::Duration;

use tokio::time::Instant;

use crate::bounded_git::GitDeadline;
use crate::client::completion::{
    certify, classify, is_settled_success_claim, CertificationStage, Disposition, Verdict,
};
use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::session::{observe, Connection, Observation};
use crate::client::transport::Wake;
use crate::web::remote_control_api::dto::{BlockerKind, ChangeBlocker, OwnerExecutionContract};

/// Longest gap between authoritative rehydrations.
///
/// The event stream normally wakes the loop sooner; this is the recovery
/// cadence for a missed frame, a dropped stream, or an owner that changes
/// repository state without publishing an event this client recognizes.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Delay before retrying after a transient failure to observe.
const RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Longest one Git child may run when there is no operation deadline above it.
///
/// Deliberately the same 30 seconds as the transport's per-request valve: both
/// answer "this single hop is not coming back", and an unbounded wait needs the
/// two to agree so a stalled remote and a stalled owner cost the same nothing.
/// Generous on purpose — expiry costs a respawn on the next poll, so the only
/// way this can hurt is by being *shorter* than a slow-but-healthy `ls-remote`.
const GIT_INVOCATION_BUDGET: Duration = Duration::from_secs(30);

/// How many coherent observations a settled success row gets before release.
///
/// Two: the one that found the row, and one more. A settled row is the owner's
/// last word, so the only thing a second look can add is evidence that landed
/// in the gap — a push completing just after the status moved. A third look
/// would be polling a verdict nobody is going to revise.
const UNCERTIFIED_CLAIM_ROUNDS: usize = 2;

/// Run one step under the operation deadline, if there is one.
///
/// `None` means the deadline passed. The inner future is dropped at that point,
/// which is safe precisely because everything reachable from here is read-only:
/// `wait` submits no command, so a cancelled step can never have half-applied
/// one. Git children are the exception that needs more than dropping, and they
/// carry the deadline down to the spawn site themselves.
///
/// Without a deadline the step simply runs: an unbounded wait has nothing to
/// compare against, and inventing a bound here would be the synthesized deadline
/// this default exists to remove.
async fn within<F: Future>(deadline: Option<Instant>, future: F) -> Option<F::Output> {
    match deadline {
        Some(deadline) => tokio::time::timeout_at(deadline, future).await.ok(),
        None => Some(future.await),
    }
}

/// Whether an operation deadline exists and has already passed.
fn reached(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

/// How the Git children of one verification round are bounded.
///
/// A positive `--timeout` hands its own deadline down, so expiry is the
/// operation's answer. Without one, each child gets a fresh finite budget whose
/// expiry is only that child giving up.
fn git_deadline(deadline: Option<Instant>) -> GitDeadline {
    match deadline {
        Some(deadline) => GitDeadline::Operation(deadline),
        None => GitDeadline::PerChild(GIT_INVOCATION_BUDGET),
    }
}

/// Observe one change until verified completion, a typed failure, or timeout.
///
/// `timeout` is `None` for the unbounded default that omission and `--timeout 0`
/// both select; `Some(d)` is an explicitly requested positive operation
/// deadline, and the only form that can end in `timeout`.
pub async fn run(
    connection: &Connection,
    change_id: &str,
    timeout: Option<Duration>,
) -> ResultEnvelope {
    // Started before the deadline is derived, so the measured elapsed time can
    // never read as shorter than the budget it overran.
    let mut diagnostics = Diagnostics::new(timeout);
    let deadline = timeout.map(|timeout| Instant::now() + timeout);

    // Bounded from the very first read: a socket that accepts a connection and
    // then stops talking must cost the caller its own timeout, not the
    // transport's much longer safety valve.
    let initial = match within(deadline, observe(connection, Some(change_id))).await {
        Some(Ok(initial)) => initial,
        Some(Err(error)) => return error.into_envelope(Operation::Wait).with_change(change_id),
        None => {
            return diagnostics.expire(change_id, None, TimeoutStage::InitialObservation, None);
        }
    };
    diagnostics.record(&initial, change_id);
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
    // How many consecutive rounds a settled success row went uncertified. The
    // second one is the last: the row cannot change, so a third look would read
    // the same two facts again.
    let mut uncertified_rounds = 0usize;
    // Only the first coherent observation can say the owner never tracked this
    // change. Every later one is looking at a change it has already seen.
    let mut first_observation = true;
    loop {
        match evaluate(
            &observation,
            &instance_id,
            change_id,
            &repo_root,
            &contract,
            deadline,
            first_observation,
        )
        .await
        {
            Step::Settled(envelope) => return *envelope,
            Step::Expired { stage, detail } => {
                return diagnostics.expire(change_id, Some(&instance_id), stage, detail)
            }
            Step::UncertifiedClaim { status, detail } => {
                uncertified_rounds += 1;
                if uncertified_rounds >= UNCERTIFIED_CLAIM_ROUNDS {
                    return requires_action_envelope(
                        change_id,
                        &instance_id,
                        &status,
                        Some(detail),
                        None,
                        format!(
                            "'{change_id}' is at settled status '{status}', but repository \
                             evidence still does not prove completion"
                        ),
                    );
                }
                if reached(deadline) {
                    return diagnostics.expire(
                        change_id,
                        Some(&instance_id),
                        TimeoutStage::ObservingOwner,
                        Some(detail),
                    );
                }
            }
            Step::KeepObserving { detail } => {
                // A row that moved back into live work starts the allowance
                // over: the next settled claim is a new claim, not the tail of
                // the previous one.
                uncertified_rounds = 0;
                if reached(deadline) {
                    return diagnostics.expire(
                        change_id,
                        Some(&instance_id),
                        TimeoutStage::ObservingOwner,
                        detail,
                    );
                }
            }
        }
        first_observation = false;

        // Wake on published activity, and fall back to the poll cadence. The
        // budget never outlives the caller's deadline, so a quiet owner cannot
        // stretch the wait past what was asked for.
        let budget = match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return diagnostics.expire(
                        change_id,
                        Some(&instance_id),
                        TimeoutStage::ObservingOwner,
                        None,
                    );
                }
                POLL_INTERVAL.min(remaining)
            }
            None => POLL_INTERVAL,
        };
        let wake = connection
            .client()
            .wake_on_activity(observation.event_sequence, &instance_id, budget)
            .await;
        // Every wake reason leads to the same action — rehydrate every
        // authoritative resource — because a gap and an ordinary advance are
        // indistinguishable once the snapshot is re-read in full.
        debug_assert!(matches!(wake, Wake::Activity | Wake::Gap | Wake::Idle));

        observation = loop {
            let Some(result) = within(deadline, observe(connection, Some(change_id))).await else {
                return diagnostics.expire(
                    change_id,
                    Some(&instance_id),
                    TimeoutStage::ObservingOwner,
                    None,
                );
            };
            match result {
                Ok(next) => break next,
                Err(error) if error.is_transient() => {
                    if reached(deadline) {
                        return diagnostics.expire(
                            change_id,
                            Some(&instance_id),
                            TimeoutStage::ObservingOwner,
                            Some(error.message().to_string()),
                        );
                    }
                    let backoff = match deadline {
                        Some(deadline) => {
                            RETRY_INTERVAL.min(deadline.saturating_duration_since(Instant::now()))
                        }
                        None => RETRY_INTERVAL,
                    };
                    tokio::time::sleep(backoff).await;
                }
                // A socket that stopped answering mid-wait is an owner that is
                // gone. It is never completion: the repository has to say so.
                Err(error) => {
                    match certify(change_id, &repo_root, &contract, git_deadline(deadline)).await {
                        Verdict::Completed { evidence } => {
                            return completed_envelope(change_id, &instance_id, &contract, evidence)
                        }
                        // The deadline passed while the last proof was being
                        // read. `timeout` is the operation's answer; the
                        // transport error that arrived first was already too
                        // late to replace it.
                        Verdict::DeadlineExpired { stage } => {
                            return diagnostics.expire(
                                change_id,
                                Some(&instance_id),
                                stage.into(),
                                None,
                            )
                        }
                        _ => {}
                    }
                    return error.into_envelope(Operation::Wait).with_change(change_id);
                }
            }
        };
        // Recorded before the next evaluation, so a deadline that expires inside
        // certification still reports the observation that certification began
        // from rather than a stale one.
        diagnostics.record(&observation, change_id);
    }
}

/// One evaluation of the current observation.
enum Step {
    /// A terminal answer was reached.
    Settled(Box<ResultEnvelope>),
    /// Nothing terminal yet; `detail` explains what is still missing.
    KeepObserving { detail: Option<String> },
    /// A settled success row the repository did not certify.
    ///
    /// Separate from [`Self::KeepObserving`] because the *row* will not move
    /// again: the loop grants exactly one more coherent observation and then
    /// releases the caller, instead of polling a verdict that is already final.
    UncertifiedClaim { status: String, detail: String },
    /// The operation deadline passed inside repository verification.
    ///
    /// `stage` distinguishes the local classification from the remote
    /// comparison, because "which one was still running" is the only part of
    /// this the caller can act on.
    Expired {
        stage: TimeoutStage,
        detail: Option<String>,
    },
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
    deadline: Option<Instant>,
    first_observation: bool,
) -> Step {
    // Owner replacement first: everything below would otherwise be read from a
    // process that never saw the work this wait is about.
    if observation.instance_id != instance_id {
        let verdict = certify(change_id, repo_root, contract, git_deadline(deadline)).await;
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
            // Owner replacement is only reportable once the repository has been
            // asked; a deadline that passed first leaves that question
            // unanswered, so the operation timed out rather than observing a
            // restart it could not qualify.
            Verdict::DeadlineExpired { stage } => Step::Expired {
                stage: stage.into(),
                detail: None,
            },
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

    // A target the owner never tracked, answered before anything is classified:
    // there is no status to read, no row to keep observing, and nothing that
    // would make one appear. Only the *first* observation can say this, which is
    // why later disappearance falls through to the certification path below and
    // keeps its existing meaning.
    if first_observation && change.is_none() {
        return match certify(change_id, repo_root, contract, git_deadline(deadline)).await {
            // Archived-and-gone is the ordinary reason a proposal is missing,
            // and it is completion, not a bad target.
            Verdict::Completed { evidence } => Step::settled(completed_envelope(
                change_id,
                instance_id,
                contract,
                evidence,
            )),
            // An expired deadline proves nothing about the repository, so the
            // caller who asked for one hears about the clock rather than a
            // refusal this operation never finished qualifying.
            Verdict::DeadlineExpired { stage } => Step::Expired {
                stage: stage.into(),
                detail: None,
            },
            // Missing, broken, and unverifiable evidence all fail to rescue an
            // untracked target; the oracle's own reason travels with the
            // refusal so a broken proof is still readable behind it.
            Verdict::NotCompleted { detail }
            | Verdict::Broken { detail }
            | Verdict::Unsupported { detail } => {
                Step::settled(unknown_change_envelope(change_id, instance_id, detail))
            }
        };
    }

    let status = change.map(|change| change.display_status.as_str());
    let blocker = change.and_then(|change| change.blocker.as_ref());
    let error_detail = change.and_then(|change| change.error_detail.clone());

    match classify(status, blocker.map(|blocker| blocker.kind)) {
        Disposition::Rejected => {
            let detail = error_detail.unwrap_or_else(|| "the change was rejected".to_string());
            return Step::settled(
                ResultEnvelope::new(Operation::Wait, Outcome::ChangeRejected)
                    .with_change(change_id)
                    .with_instance(Some(instance_id.to_string()))
                    .with_message(detail),
            );
        }
        // The owner has stopped here and will not resume on its own, so holding
        // on would be waiting for an operator rather than for the work. Nothing
        // is submitted on the way out: deciding what to do about a parked change
        // is exactly the decision a client must not make.
        Disposition::RequiresAction => {
            let status = status.unwrap_or_default();
            // An external hold names its own prerequisite, so the release says
            // what is being waited on rather than only which row it stopped at.
            // Every other manual-action row has no such fact to report.
            let external = blocker.filter(|blocker| blocker.kind == BlockerKind::External);
            let message = match external {
                Some(_) => format!(
                    "'{change_id}' is blocked on an external prerequisite the owner cannot clear \
                     by itself, so it will not advance without a new operator action"
                ),
                None => format!(
                    "'{change_id}' is at status '{status}', which the owner cannot advance without \
                     a new operator action"
                ),
            };
            return Step::settled(requires_action_envelope(
                change_id,
                instance_id,
                status,
                // The change-local error is the more specific fact when the
                // owner published one; the blocker's own line is what a
                // validated external hold has instead of an error.
                error_detail.or_else(|| external.and_then(|blocker| blocker.detail.clone())),
                external,
                message,
            ));
        }
        Disposition::KeepObserving => {
            return Step::KeepObserving {
                detail: status.map(|status| {
                    format!("'{change_id}' is at status '{status}' with no terminal evidence yet")
                }),
            }
        }
        // Verification is not free, so it runs when the owner claims a terminal
        // success or when the change stopped being tracked at all. Disappearance
        // is the case that matters: it proves nothing on its own, and treating
        // it as success is the exact bug this contract exists to prevent.
        Disposition::Certify => {}
    }

    match certify(change_id, repo_root, contract, git_deadline(deadline)).await {
        Verdict::Completed { evidence } => Step::settled(completed_envelope(
            change_id,
            instance_id,
            contract,
            evidence,
        )),
        // A settled success row that the repository will not back is the one
        // "not finished" that never finishes: the owner published its verdict
        // and has nothing left to publish. It gets one more coherent look — a
        // push landing a few milliseconds after the row moved is a real race —
        // and then it is released rather than held forever.
        Verdict::NotCompleted { detail } if is_settled_success_claim(status) => {
            Step::UncertifiedClaim {
                status: status.unwrap_or_default().to_string(),
                detail,
            }
        }
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
        Verdict::DeadlineExpired { stage } => Step::Expired {
            stage: stage.into(),
            detail: None,
        },
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

/// The refusal that says the owner never had this proposal to begin with.
///
/// It is the same `change_not_found` every other target-scoped client operation
/// returns, for the same reason: a target the owner does not track is a mistake
/// in the request, not a state the caller can wait out. The oracle's reason for
/// declining to certify it rides along in `evidence_detail`, because "absent
/// from the snapshot" and "the repository does not show it finished either" are
/// two separate facts and a caller looking at a surprise refusal needs both.
fn unknown_change_envelope(
    change_id: &str,
    instance_id: &str,
    evidence_detail: String,
) -> ResultEnvelope {
    ResultEnvelope::new(Operation::Wait, Outcome::ChangeNotFound)
        .with_change(change_id)
        .with_instance(Some(instance_id.to_string()))
        .with_message(format!(
            "the owner does not track a proposal named '{change_id}', and repository evidence does \
             not prove one finished"
        ))
        .with_detail(serde_json::json!({
            "commands_submitted": 0,
            "evidence_detail": evidence_detail,
        }))
}

/// The release that says "an operator has to look at this".
///
/// It reports the observed status rather than prose, because that is the fact a
/// caller branches on: `merge wait` needs a conflict resolved, `error` needs a
/// retry decision, and a settled success row with no evidence needs someone to
/// look at the repository. An external hold adds the structured blocker for the
/// same reason — `unblock_condition` and `prerequisite_owner` are what the
/// operator actually needs, and re-deriving them from a message would be
/// parsing prose. `commands_submitted: 0` is stated here for the same reason
/// every other wait result states it — the release is an observation, and
/// nothing about reaching it moved the change.
fn requires_action_envelope(
    change_id: &str,
    instance_id: &str,
    observed_status: &str,
    error_detail: Option<String>,
    blocker: Option<&ChangeBlocker>,
    message: String,
) -> ResultEnvelope {
    let mut detail = serde_json::json!({
        "observed_status": observed_status,
        "commands_submitted": 0,
    });
    if let (Some(object), Some(error_detail)) = (detail.as_object_mut(), error_detail) {
        object.insert(
            "error_detail".to_string(),
            serde_json::Value::String(error_detail),
        );
    }
    // Serialized whole rather than field by field: the wire shape a caller sees
    // here is the same one the snapshot published, so a new blocker field never
    // has to be remembered in two places.
    if let (Some(object), Some(blocker)) = (detail.as_object_mut(), blocker) {
        if let Ok(blocker) = serde_json::to_value(blocker) {
            object.insert("blocker".to_string(), blocker);
        }
    }
    ResultEnvelope::new(Operation::Wait, Outcome::ChangeRequiresAction)
        .with_change(change_id)
        .with_instance(Some(instance_id.to_string()))
        .with_message(message)
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

/// How the expired budget reads in a message.
///
/// `None` is unreachable from a timeout: without an operation deadline there is
/// nothing to expire. It stays representable rather than panicking, because the
/// wrong answer to "which budget ran out" is a worse failure than a vague one.
fn budget_phrase(timeout: Option<Duration>) -> String {
    match timeout {
        Some(timeout) => format!(" within {}ms", timeout.as_millis()),
        None => String::new(),
    }
}

/// Where the operation deadline landed.
///
/// Four stages rather than one flag, because each one names a different thing
/// to go and look at: an owner that never answered, an owner that answered and
/// kept working, a local repository read, and a remote lookup. The vocabulary is
/// closed and stable — a caller branches on it, so it must not grow a fifth
/// spelling for a stage that already has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeoutStage {
    /// The first coherent observation never completed.
    InitialObservation,
    /// The owner was being observed for progress it had not yet made.
    ObservingOwner,
    /// Local repository evidence for the terminal mode was being classified.
    RepositoryCertification,
    /// A remote ref was being compared against the locally verified tip.
    RemoteVerification,
}

impl TimeoutStage {
    /// The wire spelling. Stable: scripts branch on it.
    fn as_str(self) -> &'static str {
        match self {
            Self::InitialObservation => "initial_observation",
            Self::ObservingOwner => "observing_owner",
            Self::RepositoryCertification => "repository_certification",
            Self::RemoteVerification => "remote_verification",
        }
    }
}

/// The oracle names which half of the proof it was in; this is the same fact.
///
/// Mapped rather than shared so the wire vocabulary stays this command's own:
/// the oracle's stages describe Git work, and the client's describe where a
/// caller's deadline expired.
impl From<CertificationStage> for TimeoutStage {
    fn from(stage: CertificationStage) -> Self {
        match stage {
            CertificationStage::Repository => Self::RepositoryCertification,
            CertificationStage::Remote => Self::RemoteVerification,
        }
    }
}

/// Everything a timeout can honestly report, accumulated as the wait runs.
///
/// It exists because the useful facts are gone by the time they are needed: the
/// observation that would explain the timeout is the one *before* the deadline,
/// and reading the owner again afterwards would answer a different question.
/// Collecting as we go costs one projection per observation and makes the
/// post-deadline read unnecessary, which is what keeps the expiry
/// observation-only.
struct Diagnostics {
    /// The configured positive timeout, when the caller asked for one.
    timeout: Option<Duration>,
    /// When the operation began; the origin for `wait_elapsed_ms`.
    started: Instant,
    /// Latest completed coherent observation, projected onto the target alone.
    ///
    /// `None` until the first one completes, which is the difference between
    /// "the owner said nothing useful" and "the owner was never reached".
    last_observation: Option<serde_json::Value>,
}

impl Diagnostics {
    fn new(timeout: Option<Duration>) -> Self {
        Self {
            timeout,
            started: Instant::now(),
            last_observation: None,
        }
    }

    /// Retain one coherent observation, replacing whatever came before it.
    ///
    /// Only the requested change survives the projection. The snapshot carries
    /// every proposal the owner tracks, and a timeout diagnostic that shipped
    /// all of them would be publishing unrelated work to a caller who asked
    /// about one change — so the row and its matching execution facts are
    /// selected here, at the one revision they were reconciled at, rather than
    /// filtered later.
    fn record(&mut self, observation: &Observation, change_id: &str) {
        let change = observation.change(change_id);
        let execution = observation
            .execution
            .changes
            .iter()
            .find(|status| status.id == change_id);
        // Serialized whole rather than field by field, for the same reason the
        // blocker is: these are the owner's own sanitized projections, and a
        // field added to one of them must not need remembering in two places.
        self.last_observation = Some(serde_json::json!({
            "observed_at": observation.execution.observed_at,
            "state_revision": observation.state_revision,
            "event_sequence": observation.event_sequence,
            "change": change,
            "execution": execution,
        }));
    }

    /// Build the typed timeout envelope for an expiry at `stage`.
    ///
    /// `instance_id` is `None` only before the first coherent observation:
    /// naming an incarnation the operation never reconciled with would claim an
    /// observation it never made.
    fn expire(
        &self,
        change_id: &str,
        instance_id: Option<&str>,
        stage: TimeoutStage,
        detail: Option<String>,
    ) -> ResultEnvelope {
        let mut message = match stage {
            TimeoutStage::InitialObservation => format!(
                "the owner did not answer the first observation{}",
                budget_phrase(self.timeout)
            ),
            _ => format!(
                "no verified terminal outcome for '{change_id}'{}",
                budget_phrase(self.timeout)
            ),
        };
        if let Some(detail) = detail {
            message.push_str(&format!("; {detail}"));
        }
        let mut body = serde_json::json!({
            "commands_submitted": 0,
            "timeout_stage": stage.as_str(),
            // Measured, not assumed: a wait that overran its budget by a
            // scheduling delay must say so rather than echo what was configured.
            "wait_elapsed_ms": self.started.elapsed().as_millis() as u64,
            // Explicitly null when nothing was observed, because a caller has to
            // tell "no observation" from "an observation with nothing in it".
            "last_observation": self.last_observation,
        });
        // Omitted rather than nulled when there is no configured budget, the way
        // every other optional field here is. Unreachable in practice: an
        // unbounded wait has no deadline to expire.
        if let (Some(object), Some(timeout)) = (body.as_object_mut(), self.timeout) {
            object.insert(
                "timeout_ms".to_string(),
                serde_json::json!(timeout.as_millis() as u64),
            );
        }
        ResultEnvelope::new(Operation::Wait, Outcome::Timeout)
            .with_change(change_id)
            .with_instance(instance_id.map(str::to_string))
            .with_message(message)
            .with_detail(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::remote_control_api::dto::{
        AttentionState, ChangeActivity, ChangeExecutionState, ChangeExecutionStatus,
        ChangeResource, ChangeTiming, ExecutionPhase, LatestLogProjection, ParallelEligibility,
        QueueIntent, TerminalMode,
    };

    /// One projected row, at the status a caller would want a timeout to name.
    fn change(id: &str, display_status: &str) -> ChangeResource {
        ChangeResource {
            id: id.to_string(),
            display_status: display_status.to_string(),
            progress_status: "in_progress".to_string(),
            completed_tasks: 3,
            total_tasks: 7,
            progress_percent: 42.0,
            dependencies: Vec::new(),
            iteration_number: Some(2),
            execution_marked: true,
            queue_intent: QueueIntent::Queued,
            attention: AttentionState::None,
            blocker: None,
            error_detail: None,
            actions: crate::web::remote_control_api::projection::change_actions_for_test(
                "select",
                display_status,
                None,
            ),
            parallel: ParallelEligibility::default(),
            timing: ChangeTiming::default(),
            latest_activity: None,
            worktree: None,
        }
    }

    /// The execution facts that belong to one row.
    fn execution(id: &str, phase: ExecutionPhase) -> ChangeExecutionStatus {
        ChangeExecutionStatus {
            id: id.to_string(),
            execution_id: Some(format!("x-{id}")),
            execution_state: ChangeExecutionState::Active,
            current_phase: phase,
            last_completed_phase: Some(ExecutionPhase::Apply),
            iteration: Some(2),
            phase_started_at: Some("2026-01-01T00:00:00Z".to_string()),
            last_completed_at: Some("2026-01-01T00:00:01Z".to_string()),
            run_started_at: Some("2026-01-01T00:00:00Z".to_string()),
            run_completed_at: None,
            latest_activity: Some(ChangeActivity {
                event_type: "phase_started".to_string(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                detail: Some("acceptance started".to_string()),
            }),
            latest_log: Some(LatestLogProjection {
                message: "running acceptance".to_string(),
                level: crate::events::LogLevel::Info,
                operation: Some("acceptance".to_string()),
                iteration: Some(2),
                created_at: "2026-01-01T00:00:02Z".to_string(),
            }),
        }
    }

    /// One coherent observation at `revision`, carrying exactly these rows.
    fn observation(
        changes: Vec<ChangeResource>,
        executions: Vec<ChangeExecutionStatus>,
        revision: u64,
    ) -> Observation {
        let mut observation = crate::client::session::observation_for_test(changes);
        observation.state_revision = revision;
        observation.event_sequence = revision * 10;
        observation.execution.changes = executions;
        observation.execution.observed_at = format!("2026-01-01T00:00:0{revision}Z");
        observation
    }

    fn contract() -> OwnerExecutionContract {
        OwnerExecutionContract {
            base_branch: "main".to_string(),
            terminal_mode: TerminalMode::Merged,
            remote: None,
            pushed_branch: None,
        }
    }

    fn external_blocker() -> ChangeBlocker {
        ChangeBlocker {
            status: "blocked".to_string(),
            kind: BlockerKind::External,
            category: Some("pending_verification".to_string()),
            detail: Some("waiting on the signing certificate".to_string()),
            unblock_condition: Some("the certificate is issued".to_string()),
            prerequisite_owner: Some("release".to_string()),
            origin: Some("apply".to_string()),
            resumable: true,
            dependencies: Vec::new(),
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
        let diagnostics = Diagnostics::new(Some(Duration::from_secs(90)));
        let envelope = diagnostics.expire(
            "alpha",
            Some("i-1"),
            TimeoutStage::ObservingOwner,
            Some("no archive entry".to_string()),
        );
        assert_eq!(envelope.outcome, Outcome::Timeout);
        assert!(!envelope.ok);
        let message = envelope.message.unwrap();
        assert!(message.contains("within 90000ms"), "{message}");
        assert!(message.contains("no archive entry"), "{message}");
        assert_eq!(envelope.detail["commands_submitted"], 0);
        // The configured budget as data, not only inside the sentence: a caller
        // deciding whether to wait again must not have to parse prose for it.
        assert_eq!(envelope.detail["timeout_ms"], 90_000);
        assert_eq!(envelope.detail["timeout_stage"], "observing_owner");
        assert!(envelope.detail["wait_elapsed_ms"].is_u64());
    }

    /// The four stages are a closed vocabulary a script branches on.
    ///
    /// Asserted as the complete mapping rather than by sampling, because the
    /// failure that matters is a *renamed* stage: a caller matching on
    /// `remote_verification` gets no compile error when the string changes, only
    /// a branch that silently stops firing.
    #[test]
    fn every_timeout_stage_has_its_own_stable_wire_spelling() {
        assert_eq!(
            TimeoutStage::InitialObservation.as_str(),
            "initial_observation"
        );
        assert_eq!(TimeoutStage::ObservingOwner.as_str(), "observing_owner");
        assert_eq!(
            TimeoutStage::RepositoryCertification.as_str(),
            "repository_certification"
        );
        assert_eq!(
            TimeoutStage::RemoteVerification.as_str(),
            "remote_verification"
        );
        // The oracle's own halves map onto the two certification stages and
        // never onto an observation stage: a Git expiry is not an owner read.
        assert_eq!(
            TimeoutStage::from(CertificationStage::Repository),
            TimeoutStage::RepositoryCertification
        );
        assert_eq!(
            TimeoutStage::from(CertificationStage::Remote),
            TimeoutStage::RemoteVerification
        );
    }

    /// A wait that never reconciled with an owner claims nothing about one.
    ///
    /// `last_observation: null` rather than an omitted key, because the caller's
    /// question is "what did you see", and the honest answer "nothing" has to be
    /// readable without distinguishing an absent field from a failed read.
    #[test]
    fn a_timeout_before_the_first_observation_invents_no_owner_or_state() {
        let diagnostics = Diagnostics::new(Some(Duration::from_millis(500)));
        let envelope = diagnostics.expire("alpha", None, TimeoutStage::InitialObservation, None);
        assert_eq!(envelope.outcome, Outcome::Timeout);
        assert_eq!(envelope.instance_id, None);
        assert_eq!(envelope.detail["timeout_stage"], "initial_observation");
        assert!(envelope.detail["last_observation"].is_null());
        assert_eq!(envelope.detail["timeout_ms"], 500);
        assert_eq!(envelope.detail["commands_submitted"], 0);
    }

    /// The whole point of the retained observation: the target's own facts.
    #[test]
    fn a_timeout_after_an_observation_reports_the_target_row_and_its_execution() {
        let mut diagnostics = Diagnostics::new(Some(Duration::from_secs(30)));
        diagnostics.record(
            &observation(
                vec![change("alpha", "accepting")],
                vec![execution("alpha", ExecutionPhase::Acceptance)],
                4,
            ),
            "alpha",
        );
        let envelope = diagnostics.expire("alpha", Some("i-1"), TimeoutStage::ObservingOwner, None);
        let last = &envelope.detail["last_observation"];
        assert_eq!(last["state_revision"], 4);
        assert_eq!(last["event_sequence"], 40);
        assert_eq!(last["observed_at"], "2026-01-01T00:00:04Z");
        assert_eq!(last["change"]["id"], "alpha");
        assert_eq!(last["change"]["display_status"], "accepting");
        assert_eq!(last["change"]["completed_tasks"], 3);
        assert_eq!(last["change"]["total_tasks"], 7);
        assert_eq!(last["execution"]["execution_id"], "x-alpha");
        assert_eq!(last["execution"]["current_phase"], "acceptance");
        assert_eq!(last["execution"]["last_completed_phase"], "apply");
        assert_eq!(last["execution"]["execution_state"], "active");
        assert_eq!(last["execution"]["run_started_at"], "2026-01-01T00:00:00Z");
        // The bounded projections travel as themselves rather than as prose.
        assert_eq!(
            last["execution"]["latest_activity"]["event_type"],
            "phase_started"
        );
        assert_eq!(
            last["execution"]["latest_log"]["message"],
            "running acceptance"
        );
    }

    /// A timeout describes one change, even when the owner tracks many.
    ///
    /// The snapshot the wait already holds carries every proposal, so shipping
    /// it whole would be the easy implementation and the wrong one: a caller
    /// asked about `alpha`, and `beta`'s status is neither its business nor
    /// something it can act on.
    #[test]
    fn a_retained_observation_carries_no_change_but_the_requested_one() {
        let mut diagnostics = Diagnostics::new(Some(Duration::from_secs(30)));
        diagnostics.record(
            &observation(
                vec![change("alpha", "applying"), change("beta", "archiving")],
                vec![
                    execution("alpha", ExecutionPhase::Apply),
                    execution("beta", ExecutionPhase::Archive),
                ],
                2,
            ),
            "alpha",
        );
        let envelope = diagnostics.expire("alpha", Some("i-1"), TimeoutStage::ObservingOwner, None);
        let rendered = envelope.detail["last_observation"].to_string();
        assert!(rendered.contains("alpha"), "{rendered}");
        assert!(!rendered.contains("beta"), "{rendered}");
        assert!(!rendered.contains("archiving"), "{rendered}");
    }

    /// The latest coherent observation wins; nothing older survives beside it.
    #[test]
    fn a_newer_coherent_observation_replaces_the_retained_one() {
        let mut diagnostics = Diagnostics::new(Some(Duration::from_secs(30)));
        diagnostics.record(
            &observation(
                vec![change("alpha", "applying")],
                vec![execution("alpha", ExecutionPhase::Apply)],
                1,
            ),
            "alpha",
        );
        diagnostics.record(
            &observation(
                vec![change("alpha", "accepting")],
                vec![execution("alpha", ExecutionPhase::Acceptance)],
                5,
            ),
            "alpha",
        );
        let envelope = diagnostics.expire("alpha", Some("i-1"), TimeoutStage::ObservingOwner, None);
        let last = &envelope.detail["last_observation"];
        assert_eq!(last["state_revision"], 5);
        assert_eq!(last["change"]["display_status"], "accepting");
        assert_eq!(last["execution"]["current_phase"], "acceptance");
    }

    /// A row the owner stopped tracking is reported as absent, not as stale.
    ///
    /// Disappearance mid-wait proves nothing, so the wait keeps observing — but
    /// the diagnostic must still describe the observation it actually took.
    /// Carrying the previous revision's row forward under a newer
    /// `state_revision` would be a mixed-revision projection, which is the one
    /// shape a coherent observation exists to prevent.
    #[test]
    fn a_retained_observation_reports_an_absent_target_rather_than_a_stale_row() {
        let mut diagnostics = Diagnostics::new(Some(Duration::from_secs(30)));
        diagnostics.record(
            &observation(
                vec![change("alpha", "applying")],
                vec![execution("alpha", ExecutionPhase::Apply)],
                1,
            ),
            "alpha",
        );
        diagnostics.record(&observation(Vec::new(), Vec::new(), 2), "alpha");
        let envelope = diagnostics.expire("alpha", Some("i-1"), TimeoutStage::ObservingOwner, None);
        let last = &envelope.detail["last_observation"];
        assert_eq!(last["state_revision"], 2);
        assert!(last["change"].is_null());
        assert!(last["execution"].is_null());
    }

    /// A certification expiry names where it happened and keeps the observation
    /// that certification began from.
    #[test]
    fn a_certification_timeout_reports_its_stage_over_the_observation_it_started_from() {
        let mut diagnostics = Diagnostics::new(Some(Duration::from_secs(30)));
        diagnostics.record(
            &observation(
                vec![change("alpha", "merged")],
                vec![execution("alpha", ExecutionPhase::Merge)],
                9,
            ),
            "alpha",
        );
        for (stage, expected) in [
            (CertificationStage::Repository, "repository_certification"),
            (CertificationStage::Remote, "remote_verification"),
        ] {
            let envelope = diagnostics.expire("alpha", Some("i-1"), stage.into(), None);
            assert_eq!(envelope.detail["timeout_stage"], expected);
            assert_eq!(envelope.detail["last_observation"]["state_revision"], 9);
            assert_eq!(
                envelope.detail["last_observation"]["change"]["display_status"],
                "merged"
            );
            assert_eq!(envelope.detail["commands_submitted"], 0);
        }
    }

    #[test]
    fn a_manual_action_release_names_the_status_and_submits_nothing() {
        let envelope = requires_action_envelope(
            "alpha",
            "i-1",
            "merge wait",
            Some("conflict in src/lib.rs".to_string()),
            None,
            "needs an operator".to_string(),
        );
        assert_eq!(envelope.outcome, Outcome::ChangeRequiresAction);
        assert!(!envelope.ok);
        assert_eq!(envelope.exit_code(), 27);
        assert_eq!(envelope.change_id.as_deref(), Some("alpha"));
        assert_eq!(envelope.detail["observed_status"], "merge wait");
        assert_eq!(envelope.detail["error_detail"], "conflict in src/lib.rs");
        assert_eq!(envelope.detail["commands_submitted"], 0);
        // A row with no structured blocker says nothing about one, rather than
        // publishing a null a caller would have to test for.
        assert!(!envelope.detail.as_object().unwrap().contains_key("blocker"));
    }

    /// The external hold publishes the prerequisite, not just the row.
    ///
    /// The whole point of releasing here is that someone has to act, so the
    /// release has to carry what they act on — the condition that clears the
    /// wait and who owns it — as data rather than inside the message.
    #[test]
    fn an_external_blocker_release_publishes_the_prerequisite_facts() {
        let envelope = requires_action_envelope(
            "alpha",
            "i-1",
            "blocked",
            Some("waiting on the signing certificate".to_string()),
            Some(&external_blocker()),
            "blocked externally".to_string(),
        );
        assert_eq!(envelope.outcome, Outcome::ChangeRequiresAction);
        assert_eq!(envelope.exit_code(), 27);
        assert_eq!(envelope.detail["observed_status"], "blocked");
        assert_eq!(envelope.detail["commands_submitted"], 0);
        assert_eq!(
            envelope.detail["error_detail"],
            "waiting on the signing certificate"
        );
        assert_eq!(envelope.detail["blocker"]["kind"], "external");
        assert_eq!(
            envelope.detail["blocker"]["category"],
            "pending_verification"
        );
        assert_eq!(
            envelope.detail["blocker"]["unblock_condition"],
            "the certificate is issued"
        );
        assert_eq!(envelope.detail["blocker"]["prerequisite_owner"], "release");
        assert_eq!(envelope.detail["blocker"]["resumable"], true);
    }

    /// The unknown-target refusal is the shared one, at the shared exit status.
    ///
    /// A caller that already branches on `change_not_found` for `mark` or
    /// `force-stop-change` must not need a second spelling for `wait`, and the
    /// refusal has to say it submitted nothing like every other wait result.
    #[test]
    fn an_unknown_target_is_refused_with_the_shared_outcome_and_submits_nothing() {
        let envelope =
            unknown_change_envelope("aaaa", "i-1", "no archive entry on 'main'".to_string());
        assert_eq!(envelope.outcome, Outcome::ChangeNotFound);
        assert!(!envelope.ok);
        assert_eq!(envelope.exit_code(), 9);
        assert_eq!(envelope.change_id.as_deref(), Some("aaaa"));
        assert_eq!(envelope.instance_id.as_deref(), Some("i-1"));
        assert_eq!(envelope.detail["commands_submitted"], 0);
        // Both facts, not just the first: absent from the snapshot *and*
        // unproven by the repository.
        assert_eq!(
            envelope.detail["evidence_detail"],
            "no archive entry on 'main'"
        );
    }

    /// Absent detail is omitted rather than nulled, the way every other optional
    /// field in this contract is: a caller testing for presence must not have to
    /// also test for `null`.
    #[test]
    fn a_manual_action_release_omits_error_detail_when_the_owner_published_none() {
        let envelope =
            requires_action_envelope("alpha", "i-1", "stopped", None, None, "stopped".to_string());
        let detail = envelope.detail.as_object().unwrap();
        assert!(!detail.contains_key("error_detail"));
        assert_eq!(detail["observed_status"], "stopped");
        assert_eq!(detail["commands_submitted"], 0);
    }

    /// One extra look, not a poll loop: the row is the owner's last word, so the
    /// only thing a second observation can add is evidence that landed in the
    /// gap between the status moving and the push completing.
    #[test]
    fn a_settled_success_claim_gets_exactly_one_second_look() {
        assert_eq!(UNCERTIFIED_CLAIM_ROUNDS, 2);
    }

    #[test]
    fn an_unbounded_wait_never_reaches_an_operation_deadline() {
        assert!(!reached(None));
        // Even an instant that is already in the past is only reachable when the
        // caller asked for one.
        assert!(reached(Some(Instant::now() - Duration::from_secs(1))));
        assert!(!reached(Some(Instant::now() + Duration::from_secs(60))));
    }

    #[test]
    fn git_children_are_bounded_per_child_exactly_when_the_operation_is_not() {
        // The invariant the unbounded default rests on: no operation deadline
        // still means no unbounded `git`.
        assert!(matches!(
            git_deadline(None),
            GitDeadline::PerChild(GIT_INVOCATION_BUDGET)
        ));
        assert!(git_deadline(Some(Instant::now())).is_operation_deadline());
    }

    #[tokio::test]
    async fn a_step_without_a_deadline_is_run_rather_than_bounded() {
        // `within` must not synthesize a bound: an unbounded wait that quietly
        // wrapped each step in one would be the 60-minute default under a new
        // name.
        assert_eq!(within(None, async { 7 }).await, Some(7));
        // A step that would block is where the difference shows: with a deadline
        // already behind it, the same future is abandoned.
        assert_eq!(
            within(
                Some(Instant::now() - Duration::from_secs(1)),
                std::future::pending::<i32>()
            )
            .await,
            None
        );
    }
}
