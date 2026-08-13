//! `cflx client enqueue` — one intent, expressed in the owner's own vocabulary.
//!
//! # The shape of the problem
//!
//! "Admit this change" is one idea, but the owner has three different ways to
//! honor it depending on facts only the owner knows: a change carrying
//! retryable evidence is retried, a live scheduler takes dynamic queue intent,
//! and an idle owner is started against an execution mark. Which one applies is
//! published per change as typed action eligibility, so the routing here reads
//! that eligibility rather than reimplementing the lifecycle matrix — the whole
//! point being that a client and a keypress can never disagree about what is
//! offered.
//!
//! # The dangerous case
//!
//! The idle route is the one that can do harm, because `Start` consumes *every*
//! execution mark, not just ours. An operator who marked `beta` for their next
//! run and then had an agent enqueue `alpha` must not discover that `beta`
//! started too. There is no "start only this one" command, and manufacturing an
//! isolated target set by clearing `beta`'s mark would be destroying operator
//! intent to make ours convenient. So the client refuses: it reports
//! `operator_intent_conflict` and leaves every mark exactly as it found it.
//!
//! The same race can also appear *between* our mark and our start. If it does,
//! the mark has already settled and cannot be honestly rolled back — a "rollback"
//! would itself be a mark mutation racing the same operator. `partial_intent`
//! reports precisely that: the mark is real, it will be consumed by whoever
//! starts next, and no completion was claimed.

use std::time::Duration;

use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::session::{describe_api_error, observe, Connection, Observation, ObserveError};
use crate::client::transport::TransportError;
use crate::web::remote_control_api::dto::{
    new_hex_id, ActionBlockedReason, ChangeExecutionState, ChangeResource, CommandRecord,
    CommandRequest, CommandSpec, CommandState, ErrorCode, QueueIntent,
};

/// How many times a stale revision may be recomputed before giving up.
///
/// Small on purpose: a client that loses this race three times in a row is
/// competing with a busy operator, and looping harder would only make the
/// contention worse while hiding it from the caller.
pub const MAX_REVISION_ATTEMPTS: usize = 3;

/// How long a submitted command may stay `running` before the client stops
/// waiting for its record to settle.
const SETTLEMENT_TIMEOUT: Duration = Duration::from_secs(60);

/// Gap between command-record polls while a command is still running.
const SETTLEMENT_POLL: Duration = Duration::from_millis(25);

/// How long the client waits for the owner to publish the execution episode a
/// settled admission created.
///
/// A settled command proves the owner accepted the intent; the episode ID
/// appears once the reducer's own transition reaches the execution-status
/// resource, which is a different instant. Bounded and *optional*: an owner that
/// never publishes one — an older build, or one where the admission did not
/// actually move the change — still returns an ordinary successful admission
/// rather than being reported as a failure over a field nobody asked for.
const EXECUTION_ID_TIMEOUT: Duration = Duration::from_secs(2);

/// Gap between those reads.
const EXECUTION_ID_POLL: Duration = Duration::from_millis(25);

/// What the client decided to do about one change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Nothing to do: the owner already has this change admitted.
    AlreadyAdmitted,
    /// Recover a change carrying retryable evidence.
    Retry,
    /// Add to the live scheduler's dynamic queue.
    Queue,
    /// Mark the change and start the idle owner against it.
    MarkAndStart,
}

/// Why the client refused to act, in the owner's own terms.
#[derive(Debug)]
struct Refusal {
    outcome: Outcome,
    message: String,
}

impl Refusal {
    fn new(outcome: Outcome, message: impl Into<String>) -> Self {
        Self {
            outcome,
            message: message.into(),
        }
    }
}

/// The commands this invocation actually submitted, in submission order.
///
/// A command joins the list the moment the owner answers the POST with a
/// command record, because that record *is* the owner's proof that the command
/// was admitted for execution. Settlement decides whether a command succeeded,
/// never whether it happened, so nothing is ever removed: an audit that dropped
/// a failed Start would understate the side effects an operator has to reason
/// about and would disagree with the owner's own command records. Commands that
/// were skipped, or refused before any record existed, never join it at all.
///
/// The audit spans the whole invocation rather than one revision-scoped
/// attempt, because a recomputation submits real commands too.
#[derive(Debug, Default)]
struct CommandAudit {
    submitted: Vec<&'static str>,
}

impl CommandAudit {
    fn record(&mut self, command: &'static str) {
        self.submitted.push(command);
    }

    fn submitted(&self) -> &[&'static str] {
        &self.submitted
    }
}

/// Run the enqueue intent.
pub async fn run(connection: &Connection, change_id: &str) -> ResultEnvelope {
    let mut first_instance: Option<String> = None;
    let mut audit = CommandAudit::default();

    for attempt in 0..MAX_REVISION_ATTEMPTS {
        let observation = match observe(connection, Some(change_id)).await {
            Ok(observation) => observation,
            // Incoherence is exactly the situation a recomputation loop is for:
            // reread rather than reporting a conflict the next attempt would
            // have resolved.
            Err(error) if error.is_transient() && attempt + 1 < MAX_REVISION_ATTEMPTS => continue,
            Err(error) => return error.into_envelope(Operation::Enqueue),
        };

        // A new incarnation cannot prove whether a command this client already
        // submitted settled, so the intent stops rather than being reissued.
        match &first_instance {
            None => first_instance = Some(observation.instance_id.clone()),
            Some(expected) if *expected != observation.instance_id => {
                return restarted_envelope(change_id, expected, &observation.instance_id)
            }
            Some(_) => {}
        }

        match attempt_intent(connection, &observation, change_id, &mut audit).await {
            Attempt::Settled(envelope) => {
                return resolve_execution(connection, change_id, *envelope).await
            }
            Attempt::Stale => continue,
        }
    }

    ResultEnvelope::new(Operation::Enqueue, Outcome::RevisionConflict)
        .with_change(change_id)
        .with_instance(first_instance)
        .with_message(format!(
            "the owner's state advanced past every observation this client made; \
             {MAX_REVISION_ATTEMPTS} bounded recomputations were exhausted without a settled \
             admission"
        ))
}

/// Name the execution episode a successful admission belongs to.
///
/// Only successes carry one: a refusal admitted nothing, so an episode ID on it
/// would name work this invocation is not responsible for. Concurrent callers
/// that each observe the *same* already-admitted episode read the same ID, which
/// is what makes `already_admitted` a usable subscription target rather than a
/// dead end.
///
/// Every failure here is silent by construction. The admission already settled;
/// downgrading it because an additive observability field could not be read
/// would be reporting a worse outcome than the one that happened.
async fn resolve_execution(
    connection: &Connection,
    change_id: &str,
    envelope: ResultEnvelope,
) -> ResultEnvelope {
    if !matches!(
        envelope.outcome,
        Outcome::Admitted | Outcome::AlreadyAdmitted
    ) {
        return envelope;
    }
    let deadline = tokio::time::Instant::now() + EXECUTION_ID_TIMEOUT;
    loop {
        if let Ok(observation) = observe(connection, Some(change_id)).await {
            // A different incarnation cannot vouch for the episode this
            // admission created, and binding to its ID would attach a later
            // subscription to work nobody asked for.
            if envelope.instance_id.as_deref() != Some(observation.instance_id.as_str()) {
                return envelope;
            }
            if let Some(execution_id) = observation.execution_id(change_id) {
                return envelope.with_execution(Some(execution_id));
            }
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return envelope;
        }
        tokio::time::sleep(EXECUTION_ID_POLL.min(remaining)).await;
    }
}

/// The result of one revision-scoped attempt.
enum Attempt {
    /// The intent reached a final answer.
    Settled(Box<ResultEnvelope>),
    /// The owner's revision moved; recompute the whole intent.
    Stale,
}

impl Attempt {
    fn settled(envelope: ResultEnvelope) -> Self {
        Self::Settled(Box::new(envelope))
    }
}

async fn attempt_intent(
    connection: &Connection,
    observation: &Observation,
    change_id: &str,
    audit: &mut CommandAudit,
) -> Attempt {
    let instance = Some(observation.instance_id.clone());

    // Capability first: a process with no executor refuses every mutation and
    // queues none of them, so discovering that from a refusal would mean
    // submitting a command we already know cannot run.
    if !observation.command_capable() {
        return Attempt::settled(
            ResultEnvelope::new(Operation::Enqueue, Outcome::OwnerNotCommandCapable)
                .with_change(change_id)
                .with_instance(instance)
                .with_message(
                    "this owner serves reads but has no command executor bound, so it cannot \
                     admit work. A headless `cflx run` is read-only for control purposes",
                ),
        );
    }

    let Some(change) = observation.change(change_id) else {
        return Attempt::settled(
            ResultEnvelope::new(Operation::Enqueue, Outcome::ChangeNotFound)
                .with_change(change_id)
                .with_instance(instance)
                .with_message(format!(
                    "the owner does not track a change named '{change_id}'"
                )),
        );
    };

    let execution_state = observation
        .execution
        .changes
        .iter()
        .find(|status| status.id == change_id)
        .map(|status| status.execution_state);

    let route = match classify(change, execution_state) {
        Ok(route) => route,
        Err(refusal) => {
            return Attempt::settled(
                ResultEnvelope::new(Operation::Enqueue, refusal.outcome)
                    .with_change(change_id)
                    .with_instance(instance)
                    .with_message(refusal.message)
                    .with_detail(serde_json::json!({
                        "display_status": change.display_status,
                        "queue_intent": change.queue_intent,
                        "parallel": change.parallel,
                        "actions": change.actions,
                    })),
            )
        }
    };

    match route {
        Route::AlreadyAdmitted => Attempt::settled(
            ResultEnvelope::new(Operation::Enqueue, Outcome::AlreadyAdmitted)
                .with_change(change_id)
                .with_instance(instance)
                .with_message(
                    "the owner already has this change admitted; no command was submitted",
                )
                .with_detail(serde_json::json!({
                    "display_status": change.display_status,
                    "queue_intent": change.queue_intent,
                    "execution_state": execution_state,
                    "commands_submitted": 0,
                })),
        ),
        Route::Retry => {
            submit_single(
                connection,
                observation,
                change_id,
                CommandSpec::RetryChange {
                    change_id: change_id.to_string(),
                },
                "retry_change",
                audit,
            )
            .await
        }
        Route::Queue => {
            submit_single(
                connection,
                observation,
                change_id,
                CommandSpec::SetQueueIntent {
                    change_id: change_id.to_string(),
                    queued: true,
                },
                "set_queue_intent",
                audit,
            )
            .await
        }
        Route::MarkAndStart => mark_and_start(connection, observation, change_id, audit).await,
    }
}

/// Decide the route from authoritative fields alone.
///
/// Every branch reads a field the owner published; none of them re-derives a
/// lifecycle rule. The pre-checks come first because several of them describe
/// changes whose `set_execution_mark` is still advertised as allowed — marking
/// is a legal mutation on a worktree-ineligible or dependency-blocked change,
/// but *admitting* one is not what the caller asked for.
fn classify(
    change: &ChangeResource,
    execution_state: Option<ChangeExecutionState>,
) -> Result<Route, Refusal> {
    if change.queue_intent == QueueIntent::Queued
        || matches!(
            execution_state,
            Some(
                ChangeExecutionState::Queued
                    | ChangeExecutionState::Active
                    | ChangeExecutionState::Stopping
            )
        )
        || crate::orchestration::operator_command::is_active_status(&change.display_status)
    {
        return Ok(Route::AlreadyAdmitted);
    }

    if crate::orchestration::operator_command::is_final_status(&change.display_status) {
        return Err(Refusal::new(
            Outcome::TargetIneligible,
            format!(
                "'{}' has reached the final status '{}' and accepts no admission",
                change.id, change.display_status
            ),
        ));
    }

    if !change.parallel.eligible {
        let reason = change
            .parallel
            .blocked_reason
            .map(|reason| format!("{reason:?}"))
            .unwrap_or_else(|| "unspecified".to_string());
        return Err(Refusal::new(
            Outcome::TargetIneligible,
            format!(
                "'{}' cannot take part in worktree execution ({}); commit its change directory \
                 before admitting it",
                change.id,
                reason.to_lowercase()
            ),
        ));
    }

    // The active run owns this target's exhausted Apply ceiling. It is not a
    // permanent property, but nothing this client submits can clear it.
    if blocked_by(change, ActionBlockedReason::ApplyIterationLimitActive) {
        return Err(Refusal::new(
            Outcome::TargetIneligible,
            format!(
                "'{}' has exhausted its Apply-dispatch ceiling in the active run; it becomes \
                 admissible again once that run's boundary closes",
                change.id
            ),
        ));
    }

    if change.actions.retry_change.allowed {
        return Ok(Route::Retry);
    }

    // A held change that is not retryable is waiting on something an admission
    // cannot supply, so admitting it would be a lie about what happens next.
    if matches!(change.display_status.as_str(), "blocked" | "stalled") {
        let detail = change
            .blocker
            .as_ref()
            .and_then(|blocker| blocker.detail.clone())
            .unwrap_or_else(|| "no retryable evidence".to_string());
        return Err(Refusal::new(
            Outcome::TargetIneligible,
            format!(
                "'{}' is {} and carries no retryable evidence: {detail}",
                change.id, change.display_status
            ),
        ));
    }

    if change.actions.set_queue_intent.allowed {
        return Ok(Route::Queue);
    }

    if change.actions.set_execution_mark.allowed {
        return Ok(Route::MarkAndStart);
    }

    Err(Refusal::new(
        Outcome::TargetIneligible,
        format!(
            "'{}' offers no admission route at status '{}': queue intent is refused ({}) and \
             execution marking is refused ({})",
            change.id,
            change.display_status,
            describe_block(change.actions.set_queue_intent.blocked_reason),
            describe_block(change.actions.set_execution_mark.blocked_reason),
        ),
    ))
}

fn blocked_by(change: &ChangeResource, reason: ActionBlockedReason) -> bool {
    [
        change.actions.retry_change.blocked_reason,
        change.actions.set_queue_intent.blocked_reason,
        change.actions.set_execution_mark.blocked_reason,
    ]
    .into_iter()
    .flatten()
    .any(|blocked| blocked == reason)
}

fn describe_block(reason: Option<ActionBlockedReason>) -> String {
    match reason {
        Some(reason) => serde_json::to_value(reason)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{reason:?}")),
        None => "no reason published".to_string(),
    }
}

/// Every other change currently holding an execution mark.
fn unrelated_marks(observation: &Observation, change_id: &str) -> Vec<String> {
    observation
        .state
        .snapshot
        .changes
        .iter()
        .filter(|change| change.execution_marked && change.id != change_id)
        .map(|change| change.id.clone())
        .collect()
}

/// Mark the requested change and start the idle owner against it.
async fn mark_and_start(
    connection: &Connection,
    observation: &Observation,
    change_id: &str,
    audit: &mut CommandAudit,
) -> Attempt {
    let instance = Some(observation.instance_id.clone());

    // Before anything is written, an unrelated mark is a clean refusal: nothing
    // of ours exists yet, so there is nothing partial about stopping here.
    let existing = unrelated_marks(observation, change_id);
    if !existing.is_empty() {
        return Attempt::settled(intent_conflict(change_id, instance, &existing));
    }

    // A change that is already marked needs no mark command; submitting one
    // would settle as a no-op and add nothing but a record.
    if !observation
        .change(change_id)
        .is_some_and(|change| change.execution_marked)
    {
        match submit_and_settle(
            connection,
            CommandSpec::SetExecutionMark {
                change_id: change_id.to_string(),
                marked: true,
            },
            "set_execution_mark",
            observation.state_revision,
            &observation.instance_id,
            audit,
        )
        .await
        {
            // The mark's own `result_revision` is deliberately not carried into
            // Start: the reread below is what Start must be admitted against,
            // because the owner may have advanced for unrelated reasons in
            // between and a mark that isolated the target set a moment ago may
            // no longer.
            Ok(_) => {}
            Err(SubmitFailure::Stale { .. }) => return Attempt::Stale,
            Err(failure) => {
                return Attempt::settled(failure.into_envelope(change_id, instance).with_detail(
                    serde_json::json!({
                        "commands_submitted": audit.submitted(),
                        "phase": "mark",
                    }),
                ))
            }
        }
    }

    // Reread before Start. The mark is settled from here on, so anything that
    // goes wrong below is partial intent rather than a clean refusal.
    let after_mark = match observe(connection, Some(change_id)).await {
        Ok(after_mark) => after_mark,
        Err(error) => {
            return Attempt::settled(partial_after_mark(
                change_id,
                instance,
                audit.submitted(),
                &error,
            ))
        }
    };
    if after_mark.instance_id != observation.instance_id {
        return Attempt::settled(restarted_envelope(
            change_id,
            &observation.instance_id,
            &after_mark.instance_id,
        ));
    }
    // A mark that appeared *after* ours settled is a different situation from one
    // that was already there. Ours is real now, so this is partial intent: a
    // "clean refusal" would imply nothing was written, and clearing either mark
    // to restore isolation would be mutating an operator's intent to tidy up our
    // own.
    let racing = unrelated_marks(&after_mark, change_id);
    if !racing.is_empty() {
        return Attempt::settled(partial_start(
            change_id,
            instance,
            audit.submitted(),
            format!(
                "an unrelated execution mark appeared before Start ({}), so starting would have \
                 consumed it",
                racing.join(", ")
            ),
        ));
    }
    match submit_and_settle(
        connection,
        CommandSpec::Start,
        "start",
        after_mark.state_revision,
        &observation.instance_id,
        audit,
    )
    .await
    {
        Ok(record) => Attempt::settled(
            ResultEnvelope::new(Operation::Enqueue, Outcome::Admitted)
                .with_change(change_id)
                .with_instance(instance)
                .with_message("the owner marked and started the requested change")
                .with_detail(serde_json::json!({
                    "route": "mark_and_start",
                    "commands_submitted": audit.submitted(),
                    "command_state": record.state,
                    "result_revision": record.result_revision,
                    "detail": record.detail,
                })),
        ),
        // A stale Start after a settled mark is still partial intent: the mark
        // exists, and a recomputation would submit a second one.
        Err(SubmitFailure::Stale { .. }) => Attempt::settled(partial_start(
            change_id,
            instance,
            audit.submitted(),
            "the owner's state advanced between the settled mark and Start",
        )),
        Err(failure) => {
            let message = failure.message();
            Attempt::settled(partial_start(
                change_id,
                instance,
                audit.submitted(),
                message,
            ))
        }
    }
}

fn intent_conflict(change_id: &str, instance: Option<String>, marks: &[String]) -> ResultEnvelope {
    ResultEnvelope::new(Operation::Enqueue, Outcome::OperatorIntentConflict)
        .with_change(change_id)
        .with_instance(instance)
        .with_message(format!(
            "starting this idle owner would also consume execution marks that were already set \
             when this client observed it: {}. They were left untouched; clear or start them \
             yourself, or wait for the run they belong to",
            marks.join(", ")
        ))
        .with_detail(serde_json::json!({
            "unrelated_marks": marks,
            "commands_submitted": 0,
        }))
}

fn partial_after_mark(
    change_id: &str,
    instance: Option<String>,
    submitted: &[&str],
    error: &ObserveError,
) -> ResultEnvelope {
    partial_start(
        change_id,
        instance,
        submitted,
        format!(
            "the owner could not be reread before Start: {}",
            error.message()
        ),
    )
}

/// The partial-intent result, reporting the commands *this invocation* sent.
///
/// `submitted` is the live audit list, not a description of the situation, and
/// the two disagree in both directions. A change that was already
/// execution-marked before the client looked reaches this same path without a
/// mark command, and naming `set_execution_mark` there would tell an operator
/// that the client wrote a mark it merely found. A Start that was submitted and
/// then settled as a failure is the mirror case: it is still an external effect
/// the owner recorded, so omitting it would understate what has to be reasoned
/// about next. The remaining mark and the warning that a later Start can consume
/// it are reported either way, because that part is true of the repository, not
/// of the audit.
fn partial_start(
    change_id: &str,
    instance: Option<String>,
    submitted: &[&str],
    reason: impl Into<String>,
) -> ResultEnvelope {
    let mark_was_submitted = submitted.contains(&"set_execution_mark");
    let origin = if mark_was_submitted {
        format!("the execution mark for '{change_id}' settled but Start did not")
    } else {
        format!("'{change_id}' was already execution-marked and Start did not succeed")
    };
    ResultEnvelope::new(Operation::Enqueue, Outcome::PartialIntent)
        .with_change(change_id)
        .with_instance(instance)
        .with_message(format!(
            "{origin}: {}. The mark was left in place as truthful next-run intent and a later \
             operator Start can consume it; nothing was rolled back",
            reason.into()
        ))
        .with_detail(serde_json::json!({
            "remaining_mark": change_id,
            "commands_submitted": submitted,
            "rolled_back": false,
        }))
}

fn restarted_envelope(change_id: &str, expected: &str, observed: &str) -> ResultEnvelope {
    ResultEnvelope::new(Operation::Enqueue, Outcome::OwnerRestarted)
        .with_change(change_id)
        .with_instance(Some(observed.to_string()))
        .with_message(
            "the socket began serving a different owner incarnation, which cannot prove whether \
             a command submitted to the previous one settled",
        )
        .with_detail(serde_json::json!({
            "expected_instance_id": expected,
            "observed_instance_id": observed,
        }))
}

/// Submit exactly one command and report its settled outcome.
async fn submit_single(
    connection: &Connection,
    observation: &Observation,
    change_id: &str,
    spec: CommandSpec,
    route: &'static str,
    audit: &mut CommandAudit,
) -> Attempt {
    let instance = Some(observation.instance_id.clone());
    match submit_and_settle(
        connection,
        spec,
        route,
        observation.state_revision,
        &observation.instance_id,
        audit,
    )
    .await
    {
        Ok(record) => Attempt::settled(
            ResultEnvelope::new(Operation::Enqueue, Outcome::Admitted)
                .with_change(change_id)
                .with_instance(instance)
                .with_message(format!("the owner accepted the '{route}' admission"))
                .with_detail(serde_json::json!({
                    "route": route,
                    "commands_submitted": audit.submitted(),
                    "command_state": record.state,
                    "result_revision": record.result_revision,
                    "detail": record.detail,
                })),
        ),
        Err(SubmitFailure::Stale { .. }) => Attempt::Stale,
        Err(failure) => Attempt::settled(failure.into_envelope(change_id, instance).with_detail(
            serde_json::json!({
                "route": route,
                "commands_submitted": audit.submitted(),
            }),
        )),
    }
}

/// Why a submission did not settle successfully.
#[derive(Debug)]
enum SubmitFailure {
    /// The observed revision was consumed before the command was admitted.
    Stale { current: Option<u64> },
    /// The owner has no command executor.
    ExecutorUnbound(String),
    /// The owner refused the command on lifecycle or eligibility grounds.
    Ineligible(String),
    /// The command settled as a typed failure, or could not be settled.
    Failed(String),
    /// The owner replaced itself mid-command.
    Restarted(String),
    /// Credentials were refused.
    Unauthenticated(String),
    /// The exchange failed.
    Transport(String),
}

impl SubmitFailure {
    fn message(&self) -> String {
        match self {
            Self::Stale { current } => match current {
                Some(current) => format!(
                    "the observed revision was already consumed; the owner is now at revision {current}"
                ),
                None => "the observed revision was already consumed".to_string(),
            },
            Self::ExecutorUnbound(detail)
            | Self::Ineligible(detail)
            | Self::Failed(detail)
            | Self::Restarted(detail)
            | Self::Unauthenticated(detail)
            | Self::Transport(detail) => detail.clone(),
        }
    }

    fn outcome(&self) -> Outcome {
        match self {
            Self::Stale { .. } => Outcome::RevisionConflict,
            Self::ExecutorUnbound(_) => Outcome::OwnerNotCommandCapable,
            Self::Ineligible(_) => Outcome::TargetIneligible,
            Self::Failed(_) => Outcome::CommandFailed,
            Self::Restarted(_) => Outcome::OwnerRestarted,
            Self::Unauthenticated(_) => Outcome::AuthenticationFailed,
            Self::Transport(_) => Outcome::TransportError,
        }
    }

    fn into_envelope(self, change_id: &str, instance: Option<String>) -> ResultEnvelope {
        let outcome = self.outcome();
        let message = self.message();
        ResultEnvelope::new(Operation::Enqueue, outcome)
            .with_change(change_id)
            .with_instance(instance)
            .with_message(message)
    }
}

impl From<TransportError> for SubmitFailure {
    fn from(error: TransportError) -> Self {
        Self::Transport(error.to_string())
    }
}

/// Submit one typed command and poll its record to settlement.
///
/// A fresh idempotency key per call is the correct choice, not a shortcut: the
/// key is bound to the typed identity *including* `expected_revision`, so
/// reusing one across a recomputed revision would be an `idempotency_mismatch`,
/// and reusing one across an unchanged retry is unnecessary because this client
/// never retries a submission it already sent.
async fn submit_and_settle(
    connection: &Connection,
    command: CommandSpec,
    name: &'static str,
    expected_revision: u64,
    expected_instance: &str,
    audit: &mut CommandAudit,
) -> Result<CommandRecord, SubmitFailure> {
    let request = CommandRequest {
        command,
        expected_revision,
        idempotency_key: format!("cflx-client-{}", new_hex_id()),
        correlation_id: Some(format!("cflx-client-{}", &new_hex_id()[..16])),
    };
    let body = serde_json::to_string(&request).map_err(|error| {
        SubmitFailure::Failed(format!("command envelope is not encodable: {error}"))
    })?;

    let response = connection
        .client()
        .post_json("/api/v2/commands", &body)
        .await?;

    if matches!(response.status, 401 | 403) {
        return Err(SubmitFailure::Unauthenticated(describe_api_error(
            &response.body,
            "the owner refused the presented credentials",
        )));
    }

    // The status alone does not say which body arrived. A command that was
    // *admitted* and then settled as a typed failure answers 409 with its own
    // command record, while a command refused *before* admission answers the
    // same status with an `ApiError`. Reading the record first is what keeps a
    // settled `target_ineligible` from being reported as a generic failure.
    if let Ok(record) = response.json::<CommandRecord>() {
        // The record is the owner's own proof that this command was admitted for
        // execution, so the audit is written here — before settlement is even
        // polled, let alone interpreted. Whether the command then succeeds
        // changes what the caller should do next, not what this invocation
        // already did.
        audit.record(name);
        return settle(connection, record, expected_instance).await;
    }

    let error: crate::web::remote_control_api::dto::ApiError = response.json().map_err(|_| {
        SubmitFailure::Failed(describe_api_error(
            &response.body,
            "the owner refused the command without a typed error",
        ))
    })?;
    Err(match error.error_code {
        ErrorCode::StaleRevision => SubmitFailure::Stale {
            current: error.current_revision,
        },
        ErrorCode::CommandExecutorUnbound => SubmitFailure::ExecutorUnbound(error.message),
        ErrorCode::LifecycleConflict
        | ErrorCode::TargetIneligible
        | ErrorCode::RootBusy
        | ErrorCode::NotFound => SubmitFailure::Ineligible(error.message),
        _ => SubmitFailure::Failed(format!("{} ({})", error.message, error.error_code.as_str())),
    })
}

/// Poll one command record until it stops running.
async fn settle(
    connection: &Connection,
    record: CommandRecord,
    expected_instance: &str,
) -> Result<CommandRecord, SubmitFailure> {
    let mut record = record;
    let deadline = tokio::time::Instant::now() + SETTLEMENT_TIMEOUT;
    loop {
        if record.instance_id != expected_instance {
            return Err(SubmitFailure::Restarted(
                "the command record belongs to a different owner incarnation".to_string(),
            ));
        }
        match record.state {
            CommandState::Succeeded | CommandState::NoOp => return Ok(record),
            CommandState::Failed => {
                let code = record
                    .error_code
                    .map(|code| code.as_str())
                    .unwrap_or("unspecified");
                let detail = record.detail.clone().unwrap_or_default();
                return Err(match record.error_code {
                    Some(ErrorCode::CommandExecutorUnbound) => {
                        SubmitFailure::ExecutorUnbound(detail)
                    }
                    Some(
                        ErrorCode::LifecycleConflict
                        | ErrorCode::TargetIneligible
                        | ErrorCode::RootBusy,
                    ) => SubmitFailure::Ineligible(detail),
                    Some(ErrorCode::StaleRevision) => SubmitFailure::Stale { current: None },
                    _ => SubmitFailure::Failed(format!("{detail} ({code})")),
                });
            }
            CommandState::Running => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(SubmitFailure::Failed(format!(
                "command '{}' was still running after {SETTLEMENT_TIMEOUT:?}; its effect is \
                 unknown and no second command was submitted",
                record.command_id
            )));
        }
        tokio::time::sleep(SETTLEMENT_POLL).await;
        let path = format!("/api/v2/commands/{}", record.command_id);
        let response = connection.client().get(&path).await?;
        if response.status != 200 {
            return Err(SubmitFailure::Failed(describe_api_error(
                &response.body,
                "the command record could not be read back",
            )));
        }
        record = response.json().map_err(|error| {
            SubmitFailure::Failed(format!("the command record was not usable: {error}"))
        })?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::remote_control_api::dto::{
        ActionEligibility, AttentionState, ChangeActions, ChangeTiming, ParallelBlockedReason,
        ParallelEligibility,
    };

    fn change(display_status: &str) -> ChangeResource {
        ChangeResource {
            id: "alpha".to_string(),
            display_status: display_status.to_string(),
            progress_status: "pending".to_string(),
            completed_tasks: 0,
            total_tasks: 3,
            progress_percent: 0.0,
            dependencies: Vec::new(),
            iteration_number: None,
            execution_marked: false,
            queue_intent: QueueIntent::NotQueued,
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

    fn running_change(display_status: &str) -> ChangeResource {
        let mut change = change(display_status);
        change.actions = crate::web::remote_control_api::projection::change_actions_for_test(
            "running",
            display_status,
            None,
        );
        change
    }

    #[test]
    fn an_idle_owner_routes_a_fresh_change_through_mark_and_start() {
        // An idle owner has no runtime queue to join, so queue intent is
        // refused and marking is the only route the owner actually offers.
        let idle = change("not queued");
        assert!(!idle.actions.set_queue_intent.allowed);
        assert_eq!(classify(&idle, None).unwrap(), Route::MarkAndStart);
    }

    #[test]
    fn a_live_owner_routes_a_fresh_change_through_queue_intent() {
        assert_eq!(
            classify(&running_change("not queued"), None).unwrap(),
            Route::Queue
        );
    }

    #[test]
    fn retryable_evidence_wins_over_every_other_route() {
        // An errored change under a live scheduler must be retried, not queued:
        // queue intent is refused for exactly this row, and the retry command is
        // what the owner published as allowed.
        let errored = running_change("error");
        assert!(errored.actions.retry_change.allowed);
        assert_eq!(classify(&errored, None).unwrap(), Route::Retry);
    }

    #[test]
    fn admitted_work_is_an_idempotent_success_with_no_command() {
        let mut queued = change("not queued");
        queued.queue_intent = QueueIntent::Queued;
        assert_eq!(classify(&queued, None).unwrap(), Route::AlreadyAdmitted);

        assert_eq!(
            classify(&change("not queued"), Some(ChangeExecutionState::Active)).unwrap(),
            Route::AlreadyAdmitted
        );
        assert_eq!(
            classify(&change("applying"), None).unwrap(),
            Route::AlreadyAdmitted
        );
    }

    #[test]
    fn a_final_change_is_refused_without_a_command() {
        for status in ["archived", "merged", "pushed", "rejected"] {
            let refusal = classify(&change(status), None).expect_err(status);
            assert_eq!(refusal.outcome, Outcome::TargetIneligible);
            assert!(refusal.message.contains("final status"), "{refusal:?}");
        }
    }

    #[test]
    fn a_worktree_ineligible_change_is_refused_even_though_marking_is_allowed() {
        let mut ineligible = change("not queued");
        ineligible.parallel = ParallelEligibility {
            eligible: false,
            blocked_reason: Some(ParallelBlockedReason::NotCommitted),
        };
        // The trap this guards: marking is still advertised, so a router that
        // only read `actions` would happily mark and start an unrunnable change.
        assert!(ineligible.actions.set_execution_mark.allowed);
        let refusal = classify(&ineligible, None).expect_err("worktree ineligible");
        assert_eq!(refusal.outcome, Outcome::TargetIneligible);
        assert!(
            refusal.message.contains("worktree execution"),
            "{refusal:?}"
        );
    }

    #[test]
    fn an_active_apply_iteration_limit_refuses_admission() {
        let mut limited = change("error");
        limited.actions =
            crate::web::remote_control_api::projection::limited_change_actions_for_test(
                "running", "error", true,
            );
        let refusal = classify(&limited, None).expect_err("limit is active");
        assert_eq!(refusal.outcome, Outcome::TargetIneligible);
        assert!(
            refusal.message.contains("Apply-dispatch ceiling"),
            "{refusal:?}"
        );
    }

    #[test]
    fn a_dependency_blocked_change_is_refused_rather_than_marked() {
        let mut blocked = change("blocked");
        blocked.blocker = Some(crate::web::remote_control_api::dto::ChangeBlocker {
            status: "blocked".to_string(),
            kind: crate::web::remote_control_api::dto::BlockerKind::Dependency,
            category: None,
            detail: Some("waiting on an unarchived dependency".to_string()),
            unblock_condition: None,
            prerequisite_owner: None,
            origin: None,
            resumable: true,
        });
        blocked.actions = crate::web::remote_control_api::projection::change_actions_for_test(
            "select",
            "blocked",
            blocked.blocker.as_ref(),
        );
        assert!(!blocked.actions.retry_change.allowed);
        let refusal = classify(&blocked, None).expect_err("dependency blocked");
        assert_eq!(refusal.outcome, Outcome::TargetIneligible);
        assert!(
            refusal.message.contains("unarchived dependency"),
            "{refusal:?}"
        );
    }

    #[test]
    fn a_change_with_no_offered_route_names_both_refusals() {
        let mut stuck = change("not queued");
        stuck.actions = ChangeActions {
            set_execution_mark: ActionEligibility::blocked(ActionBlockedReason::StatusImmutable),
            set_queue_intent: ActionEligibility::blocked(ActionBlockedReason::ModeHasNoQueue),
            retry_change: ActionEligibility::blocked(ActionBlockedReason::NoRetryableEvidence),
            stop_and_dequeue: ActionEligibility::allowed(),
            resolve_merge: ActionEligibility::blocked(ActionBlockedReason::NotMergeWaiting),
        };
        let refusal = classify(&stuck, None).expect_err("no route");
        assert_eq!(refusal.outcome, Outcome::TargetIneligible);
        assert!(refusal.message.contains("mode_has_no_queue"), "{refusal:?}");
        assert!(refusal.message.contains("status_immutable"), "{refusal:?}");
    }

    #[test]
    fn submission_failures_map_to_distinct_stable_outcomes() {
        assert_eq!(
            SubmitFailure::ExecutorUnbound(String::new()).outcome(),
            Outcome::OwnerNotCommandCapable
        );
        assert_eq!(
            SubmitFailure::Ineligible(String::new()).outcome(),
            Outcome::TargetIneligible
        );
        assert_eq!(
            SubmitFailure::Failed(String::new()).outcome(),
            Outcome::CommandFailed
        );
        assert_eq!(
            SubmitFailure::Restarted(String::new()).outcome(),
            Outcome::OwnerRestarted
        );
        assert_eq!(
            SubmitFailure::Stale { current: Some(4) }.outcome(),
            Outcome::RevisionConflict
        );
    }

    #[test]
    fn partial_intent_names_the_mark_and_refuses_to_claim_rollback() {
        // Both commands were submitted; only the mark settled. The audit is
        // about submission, so the failed Start belongs in it.
        let envelope = partial_start(
            "alpha",
            Some("i-1".to_string()),
            &["set_execution_mark", "start"],
            "Start was refused",
        );
        assert_eq!(envelope.outcome, Outcome::PartialIntent);
        assert!(!envelope.ok);
        let message = envelope.message.clone().unwrap();
        assert!(message.contains("left in place"), "{message}");
        assert!(message.contains("nothing was rolled back"), "{message}");
        assert_eq!(envelope.detail["remaining_mark"], "alpha");
        assert_eq!(envelope.detail["rolled_back"], false);
        assert_eq!(
            envelope.detail["commands_submitted"],
            serde_json::json!(["set_execution_mark", "start"])
        );
    }

    #[test]
    fn a_pre_existing_mark_is_never_audited_as_a_submitted_command() {
        // The mark was already there, so Start is the only command this
        // invocation sent. The remaining-mark warning still has to be reported:
        // it describes the repository, not what the client did.
        let envelope = partial_start(
            "alpha",
            Some("i-1".to_string()),
            &["start"],
            "Start was refused",
        );
        assert_eq!(envelope.outcome, Outcome::PartialIntent);
        assert_eq!(
            envelope.detail["commands_submitted"],
            serde_json::json!(["start"]),
            "a mark this client did not submit must not appear in its audit list"
        );
        let message = envelope.message.clone().unwrap();
        assert!(
            message.contains("already execution-marked"),
            "the message must not claim a mark settled: {message}"
        );
        assert!(message.contains("left in place"), "{message}");
        assert_eq!(envelope.detail["remaining_mark"], "alpha");
        assert_eq!(envelope.detail["rolled_back"], false);
    }

    #[test]
    fn a_skipped_command_never_enters_the_audit_while_a_failed_one_stays() {
        // The audit is append-only in submission order: settlement decides
        // whether a command succeeded, never whether it was sent, and a command
        // that was never POSTed leaves no trace at all.
        let mut audit = CommandAudit::default();
        assert!(audit.submitted().is_empty());
        audit.record("set_execution_mark");
        audit.record("start");
        assert_eq!(audit.submitted(), ["set_execution_mark", "start"]);
    }

    #[test]
    fn an_intent_conflict_reports_the_untouched_marks() {
        let envelope = intent_conflict("alpha", None, &["beta".to_string()]);
        assert_eq!(envelope.outcome, Outcome::OperatorIntentConflict);
        assert_eq!(envelope.detail["unrelated_marks"][0], "beta");
        assert_eq!(envelope.detail["commands_submitted"], 0);
        assert!(envelope
            .message
            .as_ref()
            .unwrap()
            .contains("left untouched"));
    }
}
