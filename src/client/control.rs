//! Client control: the same five things a TUI operator does, and nothing else.
//!
//! # Why this replaced an "admit this change" intent
//!
//! The previous client expressed admission: it read per-change eligibility and
//! picked between retry, queue intent, and mark-then-start on the caller's
//! behalf. That made a *client* the place where admission policy lived, and it
//! had one concrete failure — writing `SetQueueIntent` directly put a change
//! into the scheduler's admitted set while its execution mark was still false,
//! which is a path a keypress cannot take and which skips the owner-side mark
//! settlement that decides whether stable marked work should run at all.
//!
//! So the boundary moved to where the TUI's is. An operator marks proposals and
//! then explicitly starts; the owner decides what admission means. This module
//! does exactly that and refuses to be clever:
//!
//! * `mark` / `unmark` write one target's execution mark and return. They never
//!   construct queue intent, Start, Retry, or an execution identity, and they do
//!   not wait to see whether the owner later admitted anything.
//! * `start` / `stop` / `force_stop` submit the shared operator intent a
//!   keypress submits, against the authoritative mark set the owner already
//!   holds. There is no caller-supplied target list, because the TUI has none
//!   either.
//!
//! # Target-scoped, which is the whole point
//!
//! A mark write names one change. Marking `alpha` leaves `beta`'s mark exactly
//! as it was, so an operator's next-run intent survives an agent's request — the
//! situation the old client could only answer by refusing outright.
//!
//! # Truthfulness across a multi-target request
//!
//! Every target is classified against *one* coherent observation before any
//! command is submitted, so a request-level refusal really does happen before
//! any record exists. After that, commands go out in request order, and if a
//! later one fails the earlier ones are real: that is `partial_intent`, with the
//! exact list of records created, and no claim of rollback. Undoing a settled
//! mark would itself be a mark mutation racing whoever set it.

use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::session::{describe_api_error, observe, Connection, Observation};
use crate::client::transport::TransportError;
use crate::web::remote_control_api::dto::{
    new_hex_id, ActionBlockedReason, CommandRecord, CommandRequest, CommandSpec, CommandState,
    ErrorCode,
};

/// Most proposals one control request may name.
///
/// The same ceiling the owner publishes for subscriptions, for the same reason:
/// a request is a bounded human intent, and an unbounded one would be a
/// repository-wide mutation wearing a per-target API.
pub const MAX_TARGETS: usize = 64;

/// How many times a stale revision may be recomputed before giving up.
///
/// Small on purpose: a client that loses this race three times in a row is
/// competing with a busy operator, and looping harder would only make the
/// contention worse while hiding it from the caller.
pub const MAX_REVISION_ATTEMPTS: usize = 3;

/// How long a submitted command may stay `running` before the client stops
/// waiting for its record to settle.
const SETTLEMENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Gap between command-record polls while a command is still running.
const SETTLEMENT_POLL: std::time::Duration = std::time::Duration::from_millis(25);

/// One control action, in the vocabulary the CLI verbs and the MCP tool share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Set the named proposals' execution marks.
    Mark,
    /// Clear the named proposals' execution marks.
    Unmark,
    /// Submit the shared Start intent — F5 / `!` in the TUI.
    Start,
    /// Submit the shared graceful Stop intent.
    Stop,
    /// Submit the shared ForceStop intent.
    ForceStop,
    /// Kill exactly one named proposal immediately and dequeue it.
    ///
    /// The one control that is target-scoped *and* lifecycle: it names a single
    /// proposal, submits the shared `force_stop_change` command, and leaves
    /// every unrelated mark, queue intent, process, and the process-wide run
    /// mode untouched. It is never a spelling of [`Self::ForceStop`], which
    /// stops everything the owner is running.
    ForceStopChange,
}

impl Action {
    /// The operation an envelope reports for this action.
    pub fn operation(self) -> Operation {
        match self {
            Self::Mark => Operation::ControlMark,
            Self::Unmark => Operation::ControlUnmark,
            Self::Start => Operation::ControlStart,
            Self::Stop => Operation::ControlStop,
            Self::ForceStop => Operation::ControlForceStop,
            Self::ForceStopChange => Operation::ControlForceStopChange,
        }
    }

    /// Wire spelling accepted by `cflx_control`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mark => "mark",
            Self::Unmark => "unmark",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::ForceStop => "force_stop",
            Self::ForceStopChange => "force_stop_change",
        }
    }

    /// Parse one `cflx_control` action name.
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "mark" => Self::Mark,
            "unmark" => Self::Unmark,
            "start" => Self::Start,
            "stop" => Self::Stop,
            "force_stop" => Self::ForceStop,
            "force_stop_change" => Self::ForceStopChange,
            _ => return None,
        })
    }

    /// Whether this action addresses named proposals.
    ///
    /// The split is the contract, not a convenience: a lifecycle action that
    /// accepted a target list would be inventing "start only these", which the
    /// shared transaction does not offer and which the TUI cannot express.
    pub fn is_mark(self) -> bool {
        matches!(self, Self::Mark | Self::Unmark)
    }

    /// Whether this action is lifecycle control addressed at exactly one proposal.
    ///
    /// Kept apart from [`Self::is_mark`] because the three shapes are three
    /// contracts: a mark names 1..64 proposals and writes intent, a process-wide
    /// lifecycle action names none and consumes the owner's mark set, and this
    /// names exactly one and ends that one proposal's execution episode.
    pub fn is_single_target_lifecycle(self) -> bool {
        matches!(self, Self::ForceStopChange)
    }

    /// The mark value this action requests, for the two that request one.
    fn desired_mark(self) -> Option<bool> {
        match self {
            Self::Mark => Some(true),
            Self::Unmark => Some(false),
            _ => None,
        }
    }

    /// The shared lifecycle command this action submits, for the three that do.
    ///
    /// [`Self::ForceStopChange`] is deliberately absent: it carries a target, so
    /// its command cannot be constructed from the action alone.
    fn lifecycle_command(self) -> Option<CommandSpec> {
        match self {
            Self::Start => Some(CommandSpec::Start),
            Self::Stop => Some(CommandSpec::Stop),
            Self::ForceStop => Some(CommandSpec::ForceStop),
            _ => None,
        }
    }
}

/// Reject a target list before any owner is contacted.
///
/// Bounded, non-empty, and distinct. Duplicates are refused rather than
/// deduplicated: two spellings of the same target in one request mean the caller
/// and this client disagree about what was asked for, and silently collapsing
/// them would hide that disagreement inside a successful-looking audit.
pub fn validate_targets(change_ids: &[String]) -> Result<(), String> {
    if change_ids.is_empty() {
        return Err("name at least one proposal".to_string());
    }
    if change_ids.len() > MAX_TARGETS {
        return Err(format!(
            "one request may name at most {MAX_TARGETS} proposals; {} were given",
            change_ids.len()
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for change_id in change_ids {
        if !seen.insert(change_id.as_str()) {
            return Err(format!("'{change_id}' is named more than once"));
        }
    }
    Ok(())
}

/// Reject one whole control request from its arguments alone.
///
/// Pure: it inspects the action and the target list and nothing else — no
/// filesystem, no socket, no owner. That is what lets "a request-level
/// validation failure refuses before any command" be a property of the code
/// rather than a promise about ordering.
pub fn validate_request(action: Action, change_ids: &[String]) -> Result<(), String> {
    if action.is_mark() {
        return validate_targets(change_ids);
    }
    if action.is_single_target_lifecycle() {
        validate_targets(change_ids)?;
        if change_ids.len() != 1 {
            return Err(format!(
                "'{}' addresses exactly one proposal; {} were given. It is not a process-wide \
                 stop, and it accepts no target list",
                action.as_str(),
                change_ids.len()
            ));
        }
        return Ok(());
    }
    if !change_ids.is_empty() {
        return Err(format!(
            "'{}' consumes the owner's authoritative mark set and accepts no target list; \
             mark the proposals you want first",
            action.as_str()
        ));
    }
    Ok(())
}

/// Run one control action against an existing owner.
pub async fn run(connection: &Connection, action: Action, change_ids: &[String]) -> ResultEnvelope {
    let operation = action.operation();
    if let Err(message) = validate_request(action, change_ids) {
        return ResultEnvelope::new(operation, Outcome::UsageError).with_message(message);
    }
    if action.is_mark() {
        return marks(connection, action, change_ids).await;
    }
    if action.is_single_target_lifecycle() {
        return single_target_lifecycle(connection, action, &change_ids[0]).await;
    }
    lifecycle(connection, action).await
}

// ============================================================================
// Target-scoped lifecycle control
// ============================================================================

/// Submit one target-scoped lifecycle command and report how it settled.
///
/// Like [`lifecycle`], nothing is derived here: eligibility, cancellation
/// policy, process-group termination, reaping proof, and dequeue settlement all
/// belong to the shared transaction. The one thing this client does read is the
/// owner's own published `actions.force_stop_change`, and only to refuse before
/// creating a command record — never to invent an admission the owner did not
/// publish.
async fn single_target_lifecycle(
    connection: &Connection,
    action: Action,
    change_id: &str,
) -> ResultEnvelope {
    let operation = action.operation();
    let mut first_instance: Option<String> = None;

    for attempt in 0..MAX_REVISION_ATTEMPTS {
        let observation = match observe(connection, None).await {
            Ok(observation) => observation,
            Err(error) if error.is_transient() && attempt + 1 < MAX_REVISION_ATTEMPTS => continue,
            Err(error) => return error.into_envelope(operation),
        };
        match &first_instance {
            None => first_instance = Some(observation.instance_id.clone()),
            Some(expected) if *expected != observation.instance_id => {
                return restarted_envelope(operation, expected, &observation.instance_id)
            }
            Some(_) => {}
        }
        let instance = Some(observation.instance_id.clone());
        if !observation.command_capable() {
            return not_command_capable(operation, instance);
        }

        let Some(change) = observation.change(change_id) else {
            return ResultEnvelope::new(operation, Outcome::ChangeNotFound)
                .with_instance(instance)
                .with_change(change_id.to_string())
                .with_message(format!(
                    "the owner does not track a proposal named '{change_id}'"
                ));
        };
        let eligibility = change.actions.force_stop_change;
        if !eligibility.allowed {
            return ResultEnvelope::new(operation, Outcome::TargetIneligible)
                .with_instance(instance)
                .with_change(change_id.to_string())
                .with_message(format!(
                    "this owner cannot force-stop '{change_id}' right now ({}); its display \
                     status is '{}'",
                    describe_block(eligibility.blocked_reason),
                    change.display_status
                ))
                .with_detail(serde_json::json!({
                    "action": action.as_str(),
                    "commands_submitted": Vec::<serde_json::Value>::new(),
                    "observed_status": change.display_status,
                    "blocked_reason": eligibility.blocked_reason,
                }));
        }

        let mut audit = Vec::new();
        match submit_and_settle(
            connection,
            CommandSpec::ForceStopChange {
                change_id: change_id.to_string(),
            },
            action.as_str(),
            Some(change_id),
            observation.state_revision,
            &observation.instance_id,
            &mut audit,
        )
        .await
        {
            Ok(record) => {
                return ResultEnvelope::new(operation, Outcome::Stopped)
                    .with_instance(instance)
                    .with_change(change_id.to_string())
                    .with_message(format!(
                        "'{change_id}' was force-stopped and dequeued; completed worktree \
                         effects were not rolled back"
                    ))
                    .with_detail(serde_json::json!({
                        "action": action.as_str(),
                        "commands_submitted": audit,
                        "command_state": record.state,
                        "result_revision": record.result_revision,
                        "result": record.result,
                        "detail": record.detail,
                    }))
            }
            Err(SubmitFailure::Stale { .. }) => continue,
            Err(failure) => {
                return failure
                    .into_envelope(operation, instance)
                    .with_change(change_id.to_string())
                    .with_detail(serde_json::json!({
                        "action": action.as_str(),
                        "commands_submitted": audit,
                    }))
            }
        }
    }

    revision_conflict(operation, first_instance)
}

// ============================================================================
// Lifecycle control
// ============================================================================

/// Submit one shared lifecycle intent and report that it settled.
///
/// Nothing is derived here. Mode admissibility, eligibility, retry routing,
/// cancellation classification, and scheduler dispatch all belong to the shared
/// transaction on the owner, and the client's whole job is to hand it the same
/// intent a keypress hands it.
async fn lifecycle(connection: &Connection, action: Action) -> ResultEnvelope {
    let operation = action.operation();
    let command = action
        .lifecycle_command()
        .expect("a lifecycle action always names a command");
    let mut first_instance: Option<String> = None;

    for attempt in 0..MAX_REVISION_ATTEMPTS {
        let observation = match observe(connection, None).await {
            Ok(observation) => observation,
            Err(error) if error.is_transient() && attempt + 1 < MAX_REVISION_ATTEMPTS => continue,
            Err(error) => return error.into_envelope(operation),
        };
        match &first_instance {
            None => first_instance = Some(observation.instance_id.clone()),
            Some(expected) if *expected != observation.instance_id => {
                return restarted_envelope(operation, expected, &observation.instance_id)
            }
            Some(_) => {}
        }
        let instance = Some(observation.instance_id.clone());
        if !observation.command_capable() {
            return not_command_capable(operation, instance);
        }

        let mut audit = Vec::new();
        match submit_and_settle(
            connection,
            command.clone(),
            action.as_str(),
            None,
            observation.state_revision,
            &observation.instance_id,
            &mut audit,
        )
        .await
        {
            Ok(record) => {
                return ResultEnvelope::new(operation, Outcome::Accepted)
                    .with_instance(instance)
                    .with_message(format!(
                        "the owner accepted the shared '{}' intent",
                        action.as_str()
                    ))
                    .with_detail(serde_json::json!({
                        "action": action.as_str(),
                        "commands_submitted": audit,
                        "command_state": record.state,
                        "result_revision": record.result_revision,
                        "detail": record.detail,
                    }))
            }
            Err(SubmitFailure::Stale { .. }) => continue,
            Err(failure) => {
                return failure
                    .into_envelope(operation, instance)
                    .with_detail(serde_json::json!({
                        "action": action.as_str(),
                        "commands_submitted": audit,
                    }))
            }
        }
    }

    revision_conflict(operation, first_instance)
}

// ============================================================================
// Execution-mark control
// ============================================================================

/// What one target's classification decided, before anything is submitted.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Plan {
    /// Submit a `SetExecutionMark` for this target.
    Submit,
    /// The desired state already holds; settle unchanged without a command.
    Satisfied,
}

/// Why the whole request is refused before any command record exists.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Refusal {
    outcome: Outcome,
    change_id: String,
    message: String,
}

/// One target's settled result, in request order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetResult {
    change_id: String,
    changed: bool,
    reason: String,
}

impl TargetResult {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "change_id": self.change_id,
            "changed": self.changed,
            "reason": self.reason,
        })
    }
}

/// Classify one target against the owner's own published eligibility.
///
/// Every branch reads a field the owner published; none re-derives a lifecycle
/// rule. The distinction that matters is *whose* fact a refusal is:
///
/// * a blocked `set_execution_mark` carrying [`ActionBlockedReason::FinalStatus`]
///   is a fact about the target, and the shared service answers it as a reasoned
///   unchanged no-op, so the command is still submitted and the owner supplies
///   the reason;
/// * any other blocked reason is a fact about the *mode* the owner is in, and no
///   target in the request can be marked, so the whole request is refused before
///   a single record exists.
fn classify(
    observation: &Observation,
    change_id: &str,
    desired: bool,
) -> Result<Plan, Box<Refusal>> {
    let Some(change) = observation.change(change_id) else {
        return Err(Box::new(Refusal {
            outcome: Outcome::ChangeNotFound,
            change_id: change_id.to_string(),
            message: format!("the owner does not track a proposal named '{change_id}'"),
        }));
    };
    let mark = &change.actions.set_execution_mark;
    if !mark.allowed && !matches!(mark.blocked_reason, Some(ActionBlockedReason::FinalStatus)) {
        return Err(Box::new(Refusal {
            outcome: Outcome::TargetIneligible,
            change_id: change_id.to_string(),
            message: format!(
                "this owner refuses execution-mark mutation right now ({}), so no proposal in \
                 this request was marked",
                describe_block(mark.blocked_reason)
            ),
        }));
    }
    if change.execution_marked == desired {
        return Ok(Plan::Satisfied);
    }
    Ok(Plan::Submit)
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

/// Apply one desired mark state to every named target.
async fn marks(connection: &Connection, action: Action, change_ids: &[String]) -> ResultEnvelope {
    let operation = action.operation();
    let desired = action
        .desired_mark()
        .expect("a mark action always names a desired state");

    // Both survive stale-revision recomputation. The audit is the owner's own
    // record of what this invocation created, and a recomputation that restated
    // it would either duplicate a settled command or lose one.
    let mut settled: Vec<TargetResult> = Vec::new();
    let mut audit: Vec<serde_json::Value> = Vec::new();
    let mut first_instance: Option<String> = None;

    for attempt in 0..MAX_REVISION_ATTEMPTS {
        let observation = match observe(connection, None).await {
            Ok(observation) => observation,
            Err(error) if error.is_transient() && attempt + 1 < MAX_REVISION_ATTEMPTS => continue,
            Err(error) if settled.is_empty() => return error.into_envelope(operation),
            Err(error) => {
                return partial(operation, first_instance, &settled, &audit, error.message())
            }
        };
        match &first_instance {
            None => first_instance = Some(observation.instance_id.clone()),
            Some(expected) if *expected != observation.instance_id => {
                return restarted_envelope(operation, expected, &observation.instance_id)
            }
            Some(_) => {}
        }
        let instance = Some(observation.instance_id.clone());
        if !observation.command_capable() {
            return not_command_capable(operation, instance);
        }

        // Targets already settled by an earlier attempt are never reclassified
        // and never resubmitted: their command records exist.
        let remaining: Vec<&String> = change_ids
            .iter()
            .filter(|change_id| !settled.iter().any(|result| result.change_id == **change_id))
            .collect();

        let mut plans = Vec::with_capacity(remaining.len());
        for change_id in &remaining {
            match classify(&observation, change_id, desired) {
                Ok(plan) => plans.push((*change_id, plan)),
                // A request-level refusal after earlier targets settled is still
                // partial intent: the settled marks are real.
                Err(refusal) if settled.is_empty() => {
                    return ResultEnvelope::new(operation, refusal.outcome)
                        .with_instance(instance)
                        .with_change(refusal.change_id.clone())
                        .with_message(refusal.message.clone())
                        .with_detail(serde_json::json!({
                            "action": action.as_str(),
                            "commands_submitted": Vec::<serde_json::Value>::new(),
                            "targets": Vec::<serde_json::Value>::new(),
                        }))
                }
                Err(refusal) => {
                    return partial(
                        operation,
                        first_instance,
                        &settled,
                        &audit,
                        &refusal.message,
                    )
                }
            }
        }

        let mut revision = observation.state_revision;
        let mut stale = false;
        for (change_id, plan) in plans {
            if plan == Plan::Satisfied {
                settled.push(TargetResult {
                    change_id: change_id.clone(),
                    changed: false,
                    reason: "execution mark already had the requested value".to_string(),
                });
                continue;
            }
            match submit_and_settle(
                connection,
                CommandSpec::SetExecutionMark {
                    change_id: change_id.clone(),
                    marked: desired,
                },
                "set_execution_mark",
                Some(change_id),
                revision,
                &observation.instance_id,
                &mut audit,
            )
            .await
            {
                Ok(record) => {
                    // A no-op leaves the revision where it was, and a change
                    // advances it; either way the record names the revision the
                    // next command has to be admitted against.
                    revision = record.result_revision.unwrap_or(revision);
                    let changed = matches!(record.state, CommandState::Succeeded);
                    settled.push(TargetResult {
                        change_id: change_id.clone(),
                        changed,
                        reason: record.detail.clone().unwrap_or_else(|| match changed {
                            true => "the execution mark was updated".to_string(),
                            false => "the owner settled the request unchanged".to_string(),
                        }),
                    });
                }
                Err(SubmitFailure::Stale { .. }) => {
                    stale = true;
                    break;
                }
                // A restart discovered mid-sequence outranks partial intent, and
                // the request stops rather than walking the rest of the list. A
                // `partial_intent` here would say "the settled marks stand" about
                // marks that belonged to a process which is gone, and it would
                // keep submitting the remaining targets to an incarnation that
                // never saw the first one.
                Err(SubmitFailure::Restarted(observed)) => {
                    let mut envelope =
                        restarted_envelope(operation, &observation.instance_id, &observed);
                    // Extended rather than replaced: the two incarnation IDs are
                    // what a caller compares, and the audit is what it has to
                    // reason about next. Overwriting one with the other would
                    // drop half the answer.
                    if let Some(detail) = envelope.detail.as_object_mut() {
                        detail.insert("action".to_string(), serde_json::json!(action.as_str()));
                        detail.insert("commands_submitted".to_string(), serde_json::json!(audit));
                        detail.insert("stopped_at".to_string(), serde_json::json!(change_id));
                    }
                    return envelope;
                }
                Err(failure) if settled.is_empty() => {
                    return failure
                        .into_envelope(operation, instance)
                        .with_change(change_id.clone())
                        .with_detail(serde_json::json!({
                            "action": action.as_str(),
                            "commands_submitted": audit,
                            "targets": Vec::<serde_json::Value>::new(),
                        }))
                }
                Err(failure) => {
                    return partial(
                        operation,
                        first_instance,
                        &settled,
                        &audit,
                        failure.message(),
                    )
                }
            }
        }
        if stale {
            continue;
        }
        return succeeded(action, first_instance, &settled, &audit);
    }

    if settled.is_empty() {
        return revision_conflict(operation, first_instance);
    }
    partial(
        operation,
        first_instance,
        &settled,
        &audit,
        format!(
            "the owner's state advanced past every observation this client made; \
             {MAX_REVISION_ATTEMPTS} bounded recomputations were exhausted"
        ),
    )
}

/// The successful multi-target result.
///
/// `marked` / `unmarked` when at least one target moved, `unchanged` when none
/// did. Both are successes: a request whose desired state already held asked for
/// exactly what is now true.
fn succeeded(
    action: Action,
    instance: Option<String>,
    settled: &[TargetResult],
    audit: &[serde_json::Value],
) -> ResultEnvelope {
    let changed = settled.iter().filter(|result| result.changed).count();
    let outcome = match (changed > 0, action) {
        (false, _) => Outcome::Unchanged,
        (true, Action::Mark) => Outcome::Marked,
        (true, Action::Unmark) => Outcome::Unmarked,
        // Unreachable: only the two mark actions reach this function.
        (true, _) => Outcome::Accepted,
    };
    let envelope = ResultEnvelope::new(action.operation(), outcome)
        .with_instance(instance)
        .with_message(format!(
            "{changed} of {} named proposals changed; the owner's own settlement may later \
             admit stable marked work, and nothing here waited for it",
            settled.len()
        ))
        .with_detail(serde_json::json!({
            "action": action.as_str(),
            "commands_submitted": audit,
            "targets": settled.iter().map(TargetResult::to_json).collect::<Vec<_>>(),
        }));
    // A single-target request names its target the way every other operation
    // does; a multi-target one cannot, and the per-target list is the answer.
    match settled {
        [only] => envelope.with_change(only.change_id.clone()),
        _ => envelope,
    }
}

/// The partial-intent result, reporting exactly the records this invocation made.
///
/// No rollback is attempted and none is claimed. A settled mark is the
/// operator's next-run intent now; clearing it to tidy up would be this client
/// mutating intent it did not create.
fn partial(
    operation: Operation,
    instance: Option<String>,
    settled: &[TargetResult],
    audit: &[serde_json::Value],
    reason: impl Into<String>,
) -> ResultEnvelope {
    ResultEnvelope::new(operation, Outcome::PartialIntent)
        .with_instance(instance)
        .with_message(format!(
            "{} of the named proposals settled before the request stopped: {}. Nothing was \
             rolled back and the settled marks stand",
            settled.len(),
            reason.into()
        ))
        .with_detail(serde_json::json!({
            "commands_submitted": audit,
            "targets": settled.iter().map(TargetResult::to_json).collect::<Vec<_>>(),
            "rolled_back": false,
        }))
}

fn restarted_envelope(operation: Operation, expected: &str, observed: &str) -> ResultEnvelope {
    ResultEnvelope::new(operation, Outcome::OwnerRestarted)
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

fn not_command_capable(operation: Operation, instance: Option<String>) -> ResultEnvelope {
    ResultEnvelope::new(operation, Outcome::OwnerNotCommandCapable)
        .with_instance(instance)
        .with_message(
            "this owner serves reads but has no command executor bound, so it can run no \
             control command. A headless `cflx run` is read-only for control purposes",
        )
}

fn revision_conflict(operation: Operation, instance: Option<String>) -> ResultEnvelope {
    ResultEnvelope::new(operation, Outcome::RevisionConflict)
        .with_instance(instance)
        .with_message(format!(
            "the owner's state advanced past every observation this client made; \
             {MAX_REVISION_ATTEMPTS} bounded recomputations were exhausted without a settled \
             command"
        ))
}

// ============================================================================
// Command submission
// ============================================================================

/// Why a submission did not settle successfully.
#[derive(Debug)]
enum SubmitFailure {
    /// The observed revision was consumed before the command was admitted.
    Stale {
        /// The revision the owner reported instead, when it named one.
        current: Option<u64>,
    },
    /// The owner has no command executor.
    ExecutorUnbound(String),
    /// The owner refused the command on lifecycle or eligibility grounds.
    Ineligible(String),
    /// The command settled as a typed failure, or could not be settled.
    Failed(String),
    /// The owner replaced itself mid-command; carries the incarnation that
    /// answered instead.
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
                    "the observed revision was already consumed; the owner is now at revision \
                     {current}"
                ),
                None => "the observed revision was already consumed".to_string(),
            },
            Self::Restarted(observed) => format!(
                "the command record belongs to owner incarnation '{observed}', so this socket \
                 began serving a different process"
            ),
            Self::ExecutorUnbound(detail)
            | Self::Ineligible(detail)
            | Self::Failed(detail)
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

    fn into_envelope(self, operation: Operation, instance: Option<String>) -> ResultEnvelope {
        let outcome = self.outcome();
        let message = self.message();
        ResultEnvelope::new(operation, outcome)
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
#[allow(clippy::too_many_arguments)]
async fn submit_and_settle(
    connection: &Connection,
    command: CommandSpec,
    name: &str,
    change_id: Option<&str>,
    expected_revision: u64,
    expected_instance: &str,
    audit: &mut Vec<serde_json::Value>,
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
        let mut entry = serde_json::json!({
            "command": name,
            "command_id": record.command_id,
        });
        if let (Some(object), Some(change_id)) = (entry.as_object_mut(), change_id) {
            object.insert("change_id".to_string(), serde_json::json!(change_id));
        }
        audit.push(entry);
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
            // The observed incarnation travels with the refusal: a caller told
            // only "somebody else answered" cannot report which owner it is now
            // talking to, and that is the fact its next decision turns on.
            return Err(SubmitFailure::Restarted(record.instance_id.clone()));
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
        ActionEligibility, AttentionState, ChangeActions, ChangeResource, ChangeTiming,
        ParallelEligibility, QueueIntent,
    };

    fn change(id: &str, display_status: &str, marked: bool) -> ChangeResource {
        ChangeResource {
            id: id.to_string(),
            display_status: display_status.to_string(),
            progress_status: "pending".to_string(),
            completed_tasks: 0,
            total_tasks: 3,
            progress_percent: 0.0,
            dependencies: Vec::new(),
            iteration_number: None,
            execution_marked: marked,
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

    /// A classification-only observation: nothing here reaches a socket, which
    /// is the same boundary the contract draws — every refusal below has to be
    /// provable before any command record exists.
    fn observation(changes: Vec<ChangeResource>) -> Observation {
        crate::client::session::observation_for_test(changes)
    }

    // ── Action vocabulary ───────────────────────────────────────────────────

    #[test]
    fn every_action_round_trips_through_its_wire_name() {
        for action in [
            Action::Mark,
            Action::Unmark,
            Action::Start,
            Action::Stop,
            Action::ForceStop,
        ] {
            assert_eq!(Action::parse(action.as_str()), Some(action));
        }
        // Nothing that would be admission policy is spellable.
        for retired in ["enqueue", "queue", "retry", "admit", "set_queue_intent"] {
            assert_eq!(
                Action::parse(retired),
                None,
                "{retired} must not be an action"
            );
        }
    }

    /// Mark and lifecycle are different shapes, and the difference is enforced
    /// rather than documented: a "start only these" the shared transaction does
    /// not offer must not be expressible.
    #[test]
    fn only_mark_actions_address_named_proposals() {
        assert!(Action::Mark.is_mark());
        assert!(Action::Unmark.is_mark());
        for action in [Action::Start, Action::Stop, Action::ForceStop] {
            assert!(!action.is_mark());
            assert_eq!(action.desired_mark(), None);
            assert!(action.lifecycle_command().is_some());
        }
        assert_eq!(Action::Mark.desired_mark(), Some(true));
        assert_eq!(Action::Unmark.desired_mark(), Some(false));
        assert!(Action::Mark.lifecycle_command().is_none());
    }

    /// Start submits the shared intent and nothing else. Queue intent and retry
    /// are not reachable from this module at all, which is the property that
    /// stopped the analyze bypass.
    #[test]
    fn lifecycle_actions_map_onto_the_shared_run_control_commands() {
        assert_eq!(Action::Start.lifecycle_command(), Some(CommandSpec::Start));
        assert_eq!(Action::Stop.lifecycle_command(), Some(CommandSpec::Stop));
        assert_eq!(
            Action::ForceStop.lifecycle_command(),
            Some(CommandSpec::ForceStop)
        );
    }

    // ── Target validation ───────────────────────────────────────────────────

    #[test]
    fn a_target_list_is_bounded_non_empty_and_distinct() {
        assert!(validate_targets(&["alpha".to_string()]).is_ok());
        assert!(validate_targets(&[]).is_err());

        let duplicated = ["alpha", "beta", "alpha"].map(str::to_string).to_vec();
        let error = validate_targets(&duplicated).expect_err("a duplicate is refused");
        assert!(error.contains("more than once"), "{error}");

        let at_limit: Vec<String> = (0..MAX_TARGETS).map(|n| format!("change-{n}")).collect();
        assert!(validate_targets(&at_limit).is_ok());
        let over: Vec<String> = (0..MAX_TARGETS + 1)
            .map(|n| format!("change-{n}"))
            .collect();
        assert!(validate_targets(&over).is_err());
    }

    // ── Classification ──────────────────────────────────────────────────────

    #[test]
    fn a_target_already_in_the_desired_state_needs_no_command() {
        let observed = observation(vec![change("alpha", "not queued", true)]);
        assert_eq!(classify(&observed, "alpha", true).unwrap(), Plan::Satisfied);
        assert_eq!(classify(&observed, "alpha", false).unwrap(), Plan::Submit);
    }

    #[test]
    fn an_unknown_proposal_refuses_the_whole_request() {
        let observed = observation(vec![change("alpha", "not queued", false)]);
        let refusal = classify(&observed, "missing", true).expect_err("unknown");
        assert_eq!(refusal.outcome, Outcome::ChangeNotFound);
        assert_eq!(refusal.change_id, "missing");
    }

    /// A terminal row is a fact about the *target*, and the shared service
    /// answers it as a reasoned unchanged no-op. The client must therefore still
    /// submit rather than inventing its own refusal — reimplementing that rule
    /// here is exactly what the contract forbids.
    #[test]
    fn a_terminal_target_is_submitted_and_left_to_the_shared_no_op() {
        for status in ["archived", "merged", "pushed", "rejected"] {
            let observed = observation(vec![change("alpha", status, false)]);
            assert!(
                !observed
                    .change("alpha")
                    .unwrap()
                    .actions
                    .set_execution_mark
                    .allowed
            );
            assert_eq!(
                classify(&observed, "alpha", true).unwrap(),
                Plan::Submit,
                "{status} must reach the shared service"
            );
        }
    }

    /// A mode-level refusal is a fact about the owner, not about one target, so
    /// it stops the whole request before any record exists.
    #[test]
    fn a_mode_level_mark_refusal_stops_the_request_before_submission() {
        let mut blocked = change("alpha", "not queued", false);
        blocked.actions = ChangeActions {
            set_execution_mark: ActionEligibility::blocked(ActionBlockedReason::StopPending),
            ..blocked.actions
        };
        let observed = observation(vec![blocked]);
        let refusal = classify(&observed, "alpha", true).expect_err("mode refuses marking");
        assert_eq!(refusal.outcome, Outcome::TargetIneligible);
        assert!(refusal.message.contains("stop_pending"), "{refusal:?}");
        assert!(
            refusal
                .message
                .contains("no proposal in this request was marked"),
            "{refusal:?}"
        );
    }

    // ── Result projection ───────────────────────────────────────────────────

    #[test]
    fn a_request_that_moved_nothing_is_an_unchanged_success() {
        let settled = vec![TargetResult {
            change_id: "alpha".to_string(),
            changed: false,
            reason: "execution mark already had the requested value".to_string(),
        }];
        let envelope = succeeded(Action::Mark, Some("i-1".to_string()), &settled, &[]);
        assert_eq!(envelope.outcome, Outcome::Unchanged);
        assert!(envelope.ok);
        assert_eq!(envelope.exit_code(), 0);
        assert_eq!(envelope.change_id.as_deref(), Some("alpha"));
        // Admission is never claimed, and no episode is named.
        assert!(envelope.execution_id.is_none());
    }

    #[test]
    fn a_multi_target_success_reports_each_target_and_names_no_single_change() {
        let settled = vec![
            TargetResult {
                change_id: "alpha".to_string(),
                changed: true,
                reason: "the execution mark was updated".to_string(),
            },
            TargetResult {
                change_id: "gamma".to_string(),
                changed: false,
                reason: "execution mark already had the requested value".to_string(),
            },
        ];
        let envelope = succeeded(Action::Mark, None, &settled, &[]);
        assert_eq!(envelope.outcome, Outcome::Marked);
        assert!(envelope.change_id.is_none());
        assert_eq!(envelope.detail["targets"][0]["change_id"], "alpha");
        assert_eq!(envelope.detail["targets"][0]["changed"], true);
        assert_eq!(envelope.detail["targets"][1]["changed"], false);
        // Nothing in a mark result may describe queue intent or admission.
        let rendered = envelope.to_json_line();
        for forbidden in ["queue_intent", "admitted", "execution_id"] {
            assert!(!rendered.contains(forbidden), "{forbidden}: {rendered}");
        }
    }

    #[test]
    fn unmark_reports_its_own_success_token() {
        let settled = vec![TargetResult {
            change_id: "alpha".to_string(),
            changed: true,
            reason: "the execution mark was updated".to_string(),
        }];
        assert_eq!(
            succeeded(Action::Unmark, None, &settled, &[]).outcome,
            Outcome::Unmarked
        );
    }

    /// The audit lists exactly the records this invocation created, in order,
    /// and the result never claims a rollback that did not happen.
    #[test]
    fn partial_intent_lists_created_records_in_order_without_claiming_rollback() {
        let settled = vec![
            TargetResult {
                change_id: "alpha".to_string(),
                changed: true,
                reason: "the execution mark was updated".to_string(),
            },
            TargetResult {
                change_id: "beta".to_string(),
                changed: true,
                reason: "the execution mark was updated".to_string(),
            },
        ];
        let audit = vec![
            serde_json::json!({"command": "set_execution_mark", "command_id": "c-1", "change_id": "alpha"}),
            serde_json::json!({"command": "set_execution_mark", "command_id": "c-2", "change_id": "beta"}),
            serde_json::json!({"command": "set_execution_mark", "command_id": "c-3", "change_id": "gamma"}),
        ];
        let envelope = partial(
            Operation::ControlMark,
            Some("i-1".to_string()),
            &settled,
            &audit,
            "gamma was refused",
        );
        assert_eq!(envelope.outcome, Outcome::PartialIntent);
        assert!(!envelope.ok);
        assert_eq!(envelope.detail["rolled_back"], false);
        let commands = envelope.detail["commands_submitted"].as_array().unwrap();
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0]["change_id"], "alpha");
        assert_eq!(commands[2]["change_id"], "gamma");
        assert!(envelope.message.as_ref().unwrap().contains("rolled back"));
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

    /// No connection exists at all here: a "start only these three" request
    /// describes something the shared transaction cannot do, so it has to be
    /// refused from the arguments rather than after an owner is contacted.
    #[test]
    fn a_lifecycle_action_refuses_a_target_list_before_contact() {
        for action in [Action::Start, Action::Stop, Action::ForceStop] {
            assert!(validate_request(action, &[]).is_ok(), "{}", action.as_str());
            let error = validate_request(action, &["alpha".to_string()])
                .expect_err("a lifecycle target list is refused");
            assert!(error.contains("authoritative mark set"), "{error}");
        }
        for action in [Action::Mark, Action::Unmark] {
            assert!(validate_request(action, &["alpha".to_string()]).is_ok());
            assert!(validate_request(action, &[]).is_err());
        }
        assert_eq!(
            ResultEnvelope::new(Operation::ControlStart, Outcome::UsageError).exit_code(),
            2
        );
    }
}
