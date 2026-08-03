//! The single process-local projection owner.
//!
//! One mutex-guarded owner holds the API snapshot, `state_revision`,
//! `event_sequence`, the event and log rings, and the command registries. Every
//! state-affecting input, every log, every read, and every command admission
//! passes through it, which is what makes a published event's revision and the
//! snapshot a client can fetch describe the same instant.
//!
//! Publication happens while the lock is held, so subscribers observe sequences
//! in allocation order.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use serde_json::json;
use tokio::sync::broadcast;

use crate::events::{ExecutionEvent, LogEntry};
use crate::orchestration::operator_command::{
    classify_mark_route, classify_retry_route, is_active_status, is_final_status, MarkRoute,
    OperatorMode, RetryRoute,
};
use crate::orchestration::state::BlockerKind;
use crate::web::state::OrchestratorStateSnapshot;

use super::dto::{
    new_hex_id, ActionBlockedReason, ActionEligibility, BlockerKind as DtoBlockerKind,
    ChangeActions, ChangeBlocker, ChangeResource, CommandRecord, CommandRequest, CommandState,
    EventCategory, EventEnvelope, InstanceSnapshot, SnapshotTotals, MAX_EVENTS, MAX_LOGS,
};
use super::registry::{CommandOutcome, CommandRegistry, IdempotencyLookup, ReserveError};

/// Broadcast capacity for live event fan-out.
const EVENT_CHANNEL_CAPACITY: usize = 512;

/// Result of asking for events after a cursor.
#[derive(Debug, Clone, PartialEq)]
pub enum EventsSince {
    /// The cursor is retained; these events follow it in order.
    Replay(Vec<EventEnvelope>),
    /// The cursor is older than the ring (or from another incarnation).
    Gap,
}

/// Result of admitting a command.
#[derive(Debug, Clone)]
pub enum Admission {
    /// Reserved; the caller must execute and then settle the record.
    Admitted(Box<CommandRecord>),
    /// An exact structural replay; no side effect must run.
    Replay(Box<CommandRecord>),
    /// The key is bound to a different typed identity.
    IdempotencyMismatch,
    /// `expected_revision` does not match; carries the current revision.
    Stale(u64),
    /// No slot could be reserved without evicting in-progress work.
    Capacity,
}

struct Inner {
    state_revision: u64,
    event_sequence: u64,
    snapshot: InstanceSnapshot,
    events: VecDeque<EventEnvelope>,
    logs: VecDeque<LogEntry>,
    registry: CommandRegistry,
}

/// The projection owner.
pub struct Projection {
    instance_id: String,
    started_at: String,
    inner: Mutex<Inner>,
    tx: broadcast::Sender<EventEnvelope>,
}

impl Default for Projection {
    fn default() -> Self {
        Self::new()
    }
}

impl Projection {
    /// Start a new process incarnation with a fresh random identity.
    pub fn new() -> Self {
        Self::with_registry(CommandRegistry::default())
    }

    /// Start an incarnation with explicit registry bounds (used by tests).
    pub fn with_registry(registry: CommandRegistry) -> Self {
        let (tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            instance_id: new_hex_id(),
            started_at: Utc::now().to_rfc3339(),
            inner: Mutex::new(Inner {
                state_revision: 0,
                event_sequence: 0,
                snapshot: InstanceSnapshot::empty(),
                events: VecDeque::new(),
                logs: VecDeque::new(),
                registry,
            }),
            tx,
        }
    }

    /// This process incarnation's ID. Changes on every start.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Process start time (RFC 3339).
    pub fn started_at(&self) -> &str {
        &self.started_at
    }

    /// Subscribe to live events.
    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.tx.subscribe()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // ── Reads ────────────────────────────────────────────────────────────────

    /// Coherent snapshot with the revision and cursor it belongs to.
    pub fn snapshot(&self) -> (InstanceSnapshot, u64, u64) {
        let inner = self.lock();
        (
            inner.snapshot.clone(),
            inner.state_revision,
            inner.event_sequence,
        )
    }

    /// Current revision without cloning the snapshot.
    pub fn revision(&self) -> u64 {
        self.lock().state_revision
    }

    /// Retained logs, oldest first, with the revision and cursor.
    pub fn logs(&self) -> (Vec<LogEntry>, u64, u64) {
        let inner = self.lock();
        (
            inner.logs.iter().cloned().collect(),
            inner.state_revision,
            inner.event_sequence,
        )
    }

    /// Events strictly after `after`, or `Gap` when that cursor is unusable.
    pub fn events_after(&self, after: u64) -> EventsSince {
        let inner = self.lock();
        // A cursor ahead of us cannot have come from this incarnation.
        if after > inner.event_sequence {
            return EventsSince::Gap;
        }
        if after == inner.event_sequence {
            return EventsSince::Replay(Vec::new());
        }
        // The client needs `after + 1`; if the ring already dropped it, only a
        // full snapshot can put them back on a coherent footing.
        match inner.events.front() {
            Some(oldest) if oldest.event_sequence <= after + 1 => EventsSince::Replay(
                inner
                    .events
                    .iter()
                    .filter(|event| event.event_sequence > after)
                    .cloned()
                    .collect(),
            ),
            _ => EventsSince::Gap,
        }
    }

    /// Build the envelope that tells a client its cursor is unusable.
    pub fn gap_envelope(&self, requested_after: u64) -> EventEnvelope {
        let inner = self.lock();
        EventEnvelope {
            instance_id: self.instance_id.clone(),
            event_sequence: inner.event_sequence,
            state_revision: inner.state_revision,
            category: EventCategory::Gap,
            event_type: "replay_gap".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            change_id: None,
            payload: json!({
                "requested_after": requested_after,
                "oldest_retained": inner.events.front().map(|e| e.event_sequence),
                "recover_with": "GET /api/v2/state",
            }),
        }
    }

    // ── Writes ───────────────────────────────────────────────────────────────

    /// Apply a state-affecting input in one transaction.
    ///
    /// The revision advances once and only if the candidate snapshot differs
    /// from the stored one; the sequence always advances; the event carries the
    /// resulting revision; storage happens before publication.
    pub fn apply_state(
        &self,
        event_type: &str,
        change_id: Option<String>,
        payload: serde_json::Value,
        candidate: InstanceSnapshot,
    ) -> EventEnvelope {
        let mut inner = self.lock();

        if inner.snapshot != candidate {
            inner.state_revision += 1;
            inner.snapshot = candidate;
        }
        inner.event_sequence += 1;

        let envelope = EventEnvelope {
            instance_id: self.instance_id.clone(),
            event_sequence: inner.event_sequence,
            state_revision: inner.state_revision,
            category: EventCategory::State,
            event_type: event_type.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            change_id,
            payload,
        };
        self.store_and_publish(&mut inner, envelope.clone());
        envelope
    }

    /// Apply a state-affecting input only when it actually changes the snapshot.
    ///
    /// Periodic disk refreshes call this. Emitting an event for every tick would
    /// churn the 1000-entry ring on a completely idle process and make a
    /// client's replay window a function of uptime rather than of activity.
    pub fn apply_state_if_changed(
        &self,
        event_type: &str,
        candidate: InstanceSnapshot,
    ) -> Option<EventEnvelope> {
        {
            let inner = self.lock();
            if inner.snapshot == candidate {
                return None;
            }
        }
        Some(self.apply_state(event_type, None, json!({}), candidate))
    }

    /// Apply a presentation-only input.
    ///
    /// Allocates one ordered sequence at the *current* revision and stores it
    /// like any other event, but never compares or replaces the snapshot: the
    /// event carries no field the snapshot is built from, so recomputing a
    /// candidate could only produce a revision bump nothing asked for.
    pub fn apply_presentation(
        &self,
        event_type: &str,
        change_id: Option<String>,
        payload: serde_json::Value,
    ) -> EventEnvelope {
        let mut inner = self.lock();
        inner.event_sequence += 1;

        let envelope = EventEnvelope {
            instance_id: self.instance_id.clone(),
            event_sequence: inner.event_sequence,
            state_revision: inner.state_revision,
            category: EventCategory::State,
            event_type: event_type.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            change_id,
            payload,
        };
        self.store_and_publish(&mut inner, envelope.clone());
        envelope
    }

    /// Apply an observational log input.
    ///
    /// A log allocates a sequence and carries the *current* revision. It never
    /// advances the revision, so a chatty run cannot invalidate every client's
    /// optimistic concurrency token.
    pub fn apply_log(&self, log: LogEntry) -> EventEnvelope {
        let mut inner = self.lock();
        inner.event_sequence += 1;

        let envelope = EventEnvelope {
            instance_id: self.instance_id.clone(),
            event_sequence: inner.event_sequence,
            state_revision: inner.state_revision,
            category: EventCategory::Log,
            event_type: "log".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            change_id: log.change_id.clone(),
            payload: serde_json::to_value(&log).unwrap_or_else(|_| json!({})),
        };

        inner.logs.push_back(log);
        while inner.logs.len() > MAX_LOGS {
            inner.logs.pop_front();
        }
        self.store_and_publish(&mut inner, envelope.clone());
        envelope
    }

    fn store_and_publish(&self, inner: &mut Inner, envelope: EventEnvelope) {
        inner.events.push_back(envelope.clone());
        while inner.events.len() > MAX_EVENTS {
            inner.events.pop_front();
        }
        // Publish while still holding the lock so subscribers see allocation order.
        let _ = self.tx.send(envelope);
    }

    // ── Command admission ────────────────────────────────────────────────────

    /// Admit a command, or explain why no side effect may run.
    ///
    /// Ordering matters: idempotency lookup precedes revision validation, so an
    /// exact replay still resolves after the state advanced, while a *new* key
    /// carrying a stale revision is rejected before anything is reserved.
    pub fn admit(
        &self,
        request: &CommandRequest,
        correlation_id: &str,
        now: DateTime<Utc>,
    ) -> Admission {
        let identity = request.identity();
        let mut inner = self.lock();

        match inner
            .registry
            .lookup(&request.idempotency_key, &identity, now)
        {
            IdempotencyLookup::Replay(record) => return Admission::Replay(record),
            IdempotencyLookup::Mismatch => return Admission::IdempotencyMismatch,
            IdempotencyLookup::Unknown => {}
        }

        if request.expected_revision != inner.state_revision {
            return Admission::Stale(inner.state_revision);
        }

        let record = CommandRecord {
            command_id: new_hex_id(),
            instance_id: self.instance_id.clone(),
            command_type: request.command.type_name().to_string(),
            state: CommandState::Running,
            expected_revision: request.expected_revision,
            result_revision: None,
            correlation_id: correlation_id.to_string(),
            idempotency_key: request.idempotency_key.clone(),
            created_at: now.to_rfc3339(),
            completed_at: None,
            detail: None,
            error_code: None,
        };

        match inner
            .registry
            .reserve(&request.idempotency_key, identity, record.clone(), now)
        {
            Ok(()) => Admission::Admitted(Box::new(record)),
            Err(ReserveError::Capacity) => Admission::Capacity,
        }
    }

    /// Settle a reserved command record with the revision observed afterwards.
    pub fn complete_command(
        &self,
        command_id: &str,
        state: CommandState,
        detail: Option<String>,
        error_code: Option<super::dto::ErrorCode>,
    ) -> Option<CommandRecord> {
        let mut inner = self.lock();
        let result_revision = inner.state_revision;
        inner.registry.complete(
            command_id,
            CommandOutcome {
                state,
                result_revision,
                detail,
                error_code,
            },
        )
    }

    /// Look up a command record by ID.
    pub fn command(&self, command_id: &str) -> Option<CommandRecord> {
        self.lock().registry.get(command_id).cloned()
    }

    /// Live command-registry and idempotency-registry sizes (used by tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn registry_sizes(&self) -> (usize, usize) {
        let inner = self.lock();
        (
            inner.registry.command_len(),
            inner.registry.idempotency_len(),
        )
    }

    /// Bind an already-built identity/record pair directly (used by tests to
    /// fill the registry without going through the HTTP layer).
    #[cfg(test)]
    pub fn reserve_for_test(
        &self,
        key: &str,
        identity: super::dto::CommandIdentity,
        record: CommandRecord,
        now: DateTime<Utc>,
    ) -> Result<(), ReserveError> {
        self.lock().registry.reserve(key, identity, record, now)
    }
}

// ============================================================================
// Snapshot projection
// ============================================================================

/// Project the web monitoring snapshot into the v2 read model.
///
/// Statuses stay reducer-derived: `queue_status` is already the canonical
/// display taxonomy, and `not queued` is its explicit absence rather than a
/// separate concept.
pub fn project_snapshot(source: &OrchestratorStateSnapshot) -> InstanceSnapshot {
    let mode = OperatorMode::from_app_mode(&source.app_mode);
    let changes: Vec<ChangeResource> = source
        .changes
        .iter()
        .map(|change| {
            let display_status = change
                .queue_status
                .clone()
                .unwrap_or_else(|| "not queued".to_string());
            ChangeResource {
                id: change.id.clone(),
                actions: classify_actions(mode, &display_status, change.blocker.as_ref()),
                display_status,
                progress_status: change.status.clone(),
                completed_tasks: change.completed_tasks,
                total_tasks: change.total_tasks,
                progress_percent: change.progress_percent,
                dependencies: change.dependencies.clone(),
                iteration_number: change.iteration_number,
                execution_marked: change.execution_marked,
                queue_intent: change.queue_intent,
                attention: change.attention,
                blocker: change.blocker.clone(),
                error_detail: change.error_detail.clone(),
                parallel: change.parallel,
                timing: change.timing.clone(),
                latest_activity: change.latest_activity.clone(),
                worktree: change.worktree.clone(),
            }
        })
        .collect();

    InstanceSnapshot {
        app_mode: source.app_mode.clone(),
        is_resolving: source.is_resolving,
        process_error: source.process_error.clone(),
        parallel: source.parallel.clone(),
        totals: SnapshotTotals {
            total: changes.len(),
            completed: source.completed_changes,
            in_progress: source.in_progress_changes,
            pending: source.pending_changes,
        },
        changes,
    }
}

/// Decide which change-addressed commands the current state permits.
///
/// Every decision comes from the shared lifecycle matrix in
/// [`crate::orchestration::operator_command`] — the same one the TUI routes key
/// presses through — so a remote controller and a keypress can never be offered
/// different actions for the same change.
///
/// The rule is one-directional on purpose: an action reported as `allowed` must
/// really pass its guards, while an action may be reported as blocked slightly
/// more often than the reducer strictly requires. `resolve_merge` is the case
/// where that matters: the reducer would also accept it for an idle non-terminal
/// change, but advertising "resolve" on a change that is not waiting on a merge
/// describes an operation nobody asked for.
fn classify_actions(
    mode: OperatorMode,
    display_status: &str,
    blocker: Option<&ChangeBlocker>,
) -> ChangeActions {
    let final_status = is_final_status(display_status);
    let route = classify_mark_route(mode, display_status);

    let immutable_reason = if final_status {
        ActionBlockedReason::FinalStatus
    } else if matches!(mode, OperatorMode::Stopping) {
        ActionBlockedReason::StopPending
    } else {
        ActionBlockedReason::StatusImmutable
    };

    let set_execution_mark = match route {
        MarkRoute::MarkOnly | MarkRoute::QueueIntent => ActionEligibility::allowed(),
        MarkRoute::RetryRequired => ActionEligibility::blocked(ActionBlockedReason::RetryRequired),
        MarkRoute::Immutable => ActionEligibility::blocked(immutable_reason),
    };

    let set_queue_intent = match route {
        MarkRoute::QueueIntent => ActionEligibility::allowed(),
        // Select and Stopped have no runtime queue to mutate; a base-lane wait in
        // Running has one but refuses membership changes.
        MarkRoute::MarkOnly if matches!(mode, OperatorMode::Select | OperatorMode::Stopped) => {
            ActionEligibility::blocked(ActionBlockedReason::ModeHasNoQueue)
        }
        MarkRoute::MarkOnly => ActionEligibility::blocked(ActionBlockedReason::StatusImmutable),
        MarkRoute::RetryRequired => ActionEligibility::blocked(ActionBlockedReason::RetryRequired),
        MarkRoute::Immutable => ActionEligibility::blocked(immutable_reason),
    };

    let retry_change = classify_retry_action(display_status, blocker, final_status);

    // `DequeueChange` is refused only for a final outcome; `error` and `stopped`
    // are terminal for accounting but still dequeuable.
    let stop_and_dequeue = if final_status {
        ActionEligibility::blocked(ActionBlockedReason::FinalStatus)
    } else {
        ActionEligibility::allowed()
    };

    let resolve_merge = match display_status {
        "merge wait" | "resolve pending" | "archived" => ActionEligibility::allowed(),
        status if is_active_status(status) => {
            ActionEligibility::blocked(ActionBlockedReason::ChangeActive)
        }
        status if is_final_status(status) => {
            ActionEligibility::blocked(ActionBlockedReason::FinalStatus)
        }
        _ => ActionEligibility::blocked(ActionBlockedReason::NotMergeWaiting),
    };

    ChangeActions {
        set_execution_mark,
        set_queue_intent,
        retry_change,
        stop_and_dequeue,
        resolve_merge,
    }
}

/// Action eligibility for an `app_mode`/status pair, for tests and fixtures.
#[cfg(test)]
pub fn change_actions_for_test(
    app_mode: &str,
    display_status: &str,
    blocker: Option<&ChangeBlocker>,
) -> ChangeActions {
    classify_actions(
        OperatorMode::from_app_mode(app_mode),
        display_status,
        blocker,
    )
}

/// Retry eligibility, including the non-resumable hold the service refuses.
fn classify_retry_action(
    display_status: &str,
    blocker: Option<&ChangeBlocker>,
    final_status: bool,
) -> ActionEligibility {
    if final_status {
        return ActionEligibility::blocked(ActionBlockedReason::FinalStatus);
    }
    let blocker_kind = match blocker.map(|blocker| blocker.kind) {
        Some(DtoBlockerKind::Dependency) => BlockerKind::Dependency,
        Some(DtoBlockerKind::External) => BlockerKind::External,
        _ => BlockerKind::None,
    };
    let Some(route) = classify_retry_route(display_status, blocker_kind) else {
        return ActionEligibility::blocked(ActionBlockedReason::NoRetryableEvidence);
    };
    // A non-resumable acceptance hold keeps its blocker evidence instead of
    // being retried, so it must not be advertised as retryable.
    if matches!(route, RetryRoute::AcceptanceStall)
        && blocker.is_some_and(|blocker| !blocker.resumable)
    {
        return ActionEligibility::blocked(ActionBlockedReason::HoldNotResumable);
    }
    ActionEligibility::allowed()
}

/// Wire event type, target change, and sanitized payload for an execution event.
///
/// `ExecutionEvent` is an internal enum, so this is a deliberate projection
/// rather than a serialization of it. The match is exhaustive with no `_` arm:
/// a new variant must be projected explicitly instead of silently degrading to
/// a generic `orchestration_event` that drops its change ID and every field
/// with it.
///
/// Raw agent command lines are the one thing deliberately not published. They
/// routinely carry prompts and credentials, and the operator-facing substitute
/// (`command_bytes`/`command_hash`) identifies a command without reproducing
/// it.
pub fn describe_event(event: &ExecutionEvent) -> (&'static str, Option<String>, serde_json::Value) {
    use ExecutionEvent as E;

    /// Sanitize operator-facing free text exactly like a retained log message.
    fn detail(text: &str) -> serde_json::Value {
        json!(crate::events::sanitize_detail(text))
    }

    fn optional_detail(text: &Option<String>) -> serde_json::Value {
        match text {
            Some(text) => detail(text),
            None => serde_json::Value::Null,
        }
    }

    fn command_facts(command: &str) -> serde_json::Value {
        json!(crate::events::command_log_summary(command))
    }

    match event {
        E::ProcessingStarted(id) => ("processing_started", Some(id.clone()), json!({})),
        E::ProcessingCompleted(id) => ("processing_completed", Some(id.clone()), json!({})),
        E::ProcessingError { id, error } => (
            "processing_error",
            Some(id.clone()),
            json!({ "detail": detail(error) }),
        ),
        E::ApplyStarted { change_id, command } => (
            "apply_started",
            Some(change_id.clone()),
            json!({ "command_summary": command_facts(command) }),
        ),
        E::ApplyCompleted {
            change_id,
            revision,
        } => (
            "apply_completed",
            Some(change_id.clone()),
            json!({ "revision": revision }),
        ),
        E::ApplyFailed { change_id, error } => (
            "apply_failed",
            Some(change_id.clone()),
            json!({ "detail": detail(error) }),
        ),
        E::ApplyOutput {
            change_id,
            iteration,
            ..
        } => (
            "apply_output",
            Some(change_id.clone()),
            json!({ "iteration": iteration }),
        ),
        E::ArchiveStarted { change_id, command } => (
            "archive_started",
            Some(change_id.clone()),
            json!({ "command_summary": command_facts(command) }),
        ),
        E::ArchiveResumed {
            change_id,
            reason,
            summary,
        } => (
            "archive_resumed",
            Some(change_id.clone()),
            json!({ "detail": optional_detail(reason), "summary": optional_detail(summary) }),
        ),
        E::ArchiveRetryScheduled {
            change_id,
            attempt,
            max_attempts,
            reason,
            summary,
        } => (
            "archive_retry_scheduled",
            Some(change_id.clone()),
            json!({
                "detail": optional_detail(reason),
                "summary": optional_detail(summary),
                "attempt": attempt,
                "max_attempts": max_attempts,
            }),
        ),
        E::ChangeArchived(id) => ("change_archived", Some(id.clone()), json!({})),
        E::ArchiveFailed {
            change_id,
            error,
            reason,
            summary,
        } => (
            "archive_failed",
            Some(change_id.clone()),
            json!({
                "detail": detail(error),
                "reason": optional_detail(reason),
                "summary": optional_detail(summary),
            }),
        ),
        E::ArchiveOutput {
            change_id,
            iteration,
            ..
        } => (
            "archive_output",
            Some(change_id.clone()),
            json!({ "iteration": iteration }),
        ),
        E::AcceptanceStarted { change_id, command } => (
            "acceptance_started",
            Some(change_id.clone()),
            json!({ "command_summary": command_facts(command) }),
        ),
        E::AcceptanceCompleted { change_id } => {
            ("acceptance_completed", Some(change_id.clone()), json!({}))
        }
        E::AcceptanceFailed { change_id, error } => (
            "acceptance_failed",
            Some(change_id.clone()),
            json!({ "detail": detail(error) }),
        ),
        E::ChangeRejected { change_id, reason } => (
            "change_rejected",
            Some(change_id.clone()),
            json!({ "detail": detail(reason) }),
        ),
        E::RejectionReviewCompleted { change_id, outcome } => (
            "rejection_review_completed",
            Some(change_id.clone()),
            json!({ "outcome": rejection_outcome_name(*outcome) }),
        ),
        E::RejectionReviewFailed { change_id, error } => (
            "rejection_review_failed",
            Some(change_id.clone()),
            json!({ "detail": detail(error) }),
        ),
        E::AcceptanceOutput {
            change_id,
            iteration,
            ..
        } => (
            "acceptance_output",
            Some(change_id.clone()),
            json!({ "iteration": iteration }),
        ),
        E::ProgressUpdated {
            change_id,
            completed,
            total,
        } => (
            "progress_updated",
            Some(change_id.clone()),
            json!({ "completed": completed, "total": total }),
        ),

        // Workspace lane.
        E::WorkspaceCreated {
            change_id,
            workspace,
        } => (
            "workspace_created",
            Some(change_id.clone()),
            json!({ "workspace": detail(workspace) }),
        ),
        E::WorkspaceResumed {
            change_id,
            workspace,
        } => (
            "workspace_resumed",
            Some(change_id.clone()),
            json!({ "workspace": detail(workspace) }),
        ),
        E::WorkspacePreserved {
            change_id,
            workspace_name,
        } => (
            "workspace_preserved",
            Some(change_id.clone()),
            json!({ "workspace": detail(workspace_name) }),
        ),
        E::WorkspaceStatusUpdated {
            change_id,
            workspace_name,
            status,
        } => (
            "workspace_status_updated",
            Some(change_id.clone()),
            json!({
                "workspace": detail(workspace_name),
                "status": format!("{status:?}"),
            }),
        ),

        // Post-archive and merge-lane transitions. They are the difference
        // between "archived" and "actually landed", so a controller that cannot
        // see them cannot tell a finished change from a waiting one.
        E::ResolveStarted { change_id, command } => (
            "resolve_started",
            Some(change_id.clone()),
            json!({ "command_summary": command_facts(command) }),
        ),
        E::ResolveCompleted { change_id, .. } => {
            ("resolve_completed", Some(change_id.clone()), json!({}))
        }
        E::ResolveFailed { change_id, error } => (
            "resolve_failed",
            Some(change_id.clone()),
            json!({ "detail": detail(error) }),
        ),
        E::ResolveOutput {
            change_id,
            iteration,
            ..
        } => (
            "resolve_output",
            Some(change_id.clone()),
            json!({ "iteration": iteration }),
        ),
        E::MergeCompleted {
            change_id,
            revision,
        } => (
            "merge_completed",
            Some(change_id.clone()),
            json!({ "revision": revision }),
        ),
        E::MergeDeferred {
            change_id,
            reason,
            auto_resumable,
        } => (
            "merge_deferred",
            Some(change_id.clone()),
            json!({ "detail": detail(reason), "auto_resumable": auto_resumable }),
        ),
        E::PushStarted {
            change_id,
            remote,
            branch,
        } => (
            "push_started",
            Some(change_id.clone()),
            json!({ "remote": detail(remote), "branch": detail(branch) }),
        ),
        E::PushCompleted {
            change_id,
            remote,
            branch,
        } => (
            "push_completed",
            Some(change_id.clone()),
            json!({ "remote": detail(remote), "branch": detail(branch) }),
        ),
        E::PushFailed {
            change_id,
            remote,
            branch,
            error,
        } => (
            "push_failed",
            Some(change_id.clone()),
            json!({
                "detail": detail(error),
                "remote": detail(remote),
                "branch": detail(branch),
            }),
        ),

        // Scheduling and hold lane.
        E::ChangeSkipped { change_id, reason } => (
            "change_skipped",
            Some(change_id.clone()),
            json!({ "detail": detail(reason) }),
        ),
        E::DependencyBlocked {
            change_id,
            dependency_ids,
        } => (
            "dependency_blocked",
            Some(change_id.clone()),
            json!({ "dependency_ids": dependency_ids }),
        ),
        E::DependencyResolved { change_id } => {
            ("dependency_resolved", Some(change_id.clone()), json!({}))
        }
        E::AcceptanceGated { change_id, blocker } => (
            "acceptance_gated",
            Some(change_id.clone()),
            describe_blocker(blocker),
        ),
        E::ExecutionBlocked { change_id, blocker } => (
            "execution_blocked",
            Some(change_id.clone()),
            describe_blocker(blocker),
        ),

        // Hook lane.
        E::HookStarted {
            change_id,
            hook_type,
        } => (
            "hook_started",
            Some(change_id.clone()),
            json!({ "hook_type": detail(hook_type) }),
        ),
        E::HookCompleted {
            change_id,
            hook_type,
        } => (
            "hook_completed",
            Some(change_id.clone()),
            json!({ "hook_type": detail(hook_type) }),
        ),
        E::HookFailed {
            change_id,
            hook_type,
            error,
        } => (
            "hook_failed",
            Some(change_id.clone()),
            json!({ "hook_type": detail(hook_type), "detail": detail(error) }),
        ),

        // Single-change stop lane.
        E::ChangeDequeued { change_id } => ("change_dequeued", Some(change_id.clone()), json!({})),
        E::ChangeStopped { change_id } => ("change_stopped", Some(change_id.clone()), json!({})),
        E::ChangeStopFailed { change_id, error } => (
            "change_stop_failed",
            Some(change_id.clone()),
            json!({ "detail": detail(error) }),
        ),

        // Process-level, not change-level.
        E::Stopping => ("stopping", None, json!({})),
        E::Stopped => ("stopped", None, json!({})),
        E::AllCompleted => ("all_completed", None, json!({})),
        E::Error { message } => ("process_error", None, json!({ "detail": detail(message) })),
        E::Log(entry) => ("log", entry.change_id.clone(), json!({})),
        E::ChangesRefreshed {
            changes,
            rejected_changes,
            ..
        } => (
            "changes_refreshed",
            None,
            json!({ "changes": changes.len(), "rejected_changes": rejected_changes.len() }),
        ),
        E::WorktreesRefreshed { worktrees } => (
            "worktrees_refreshed",
            None,
            json!({ "worktrees": worktrees.len() }),
        ),

        // Presentation-only detail. Ordered on the stream, never snapshot input.
        E::CleanupStarted { workspace } => (
            "cleanup_started",
            None,
            json!({ "workspace": detail(workspace) }),
        ),
        E::CleanupCompleted { workspace } => (
            "cleanup_completed",
            None,
            json!({ "workspace": detail(workspace) }),
        ),
        E::MergeStarted { revisions } => ("merge_started", None, json!({ "revisions": revisions })),
        E::MergeConflict { files } => ("merge_conflict", None, json!({ "files": files })),
        E::ConflictResolutionStarted => ("conflict_resolution_started", None, json!({})),
        E::ConflictResolutionCompleted => ("conflict_resolution_completed", None, json!({})),
        E::ConflictResolutionFailed { error } => (
            "conflict_resolution_failed",
            None,
            json!({ "detail": detail(error) }),
        ),
        E::AnalysisStarted {
            remaining_changes,
            attempt_id,
        } => (
            "analysis_started",
            None,
            json!({ "remaining_changes": remaining_changes, "attempt_id": attempt_id }),
        ),
        E::AnalysisOutput { iteration, .. } => {
            ("analysis_output", None, json!({ "iteration": iteration }))
        }
        E::AnalysisCompleted { groups_found } => (
            "analysis_completed",
            None,
            json!({ "groups_found": groups_found }),
        ),
        E::Warning { title, message } => (
            "warning",
            None,
            json!({ "title": detail(title), "detail": detail(message) }),
        ),
        E::ParallelStartRejected { change_ids, reason } => (
            "parallel_start_rejected",
            None,
            json!({ "change_ids": change_ids, "detail": detail(reason) }),
        ),
        E::BranchMergeStarted { branch_name } => (
            "branch_merge_started",
            None,
            json!({ "branch": detail(branch_name) }),
        ),
        E::BranchMergeCompleted { branch_name } => (
            "branch_merge_completed",
            None,
            json!({ "branch": detail(branch_name) }),
        ),
        E::BranchMergeFailed { branch_name, error } => (
            "branch_merge_failed",
            None,
            json!({ "branch": detail(branch_name), "detail": detail(error) }),
        ),
    }
}

fn rejection_outcome_name(outcome: crate::events::RejectionOutcome) -> &'static str {
    match outcome {
        crate::events::RejectionOutcome::Confirm => "confirm",
        crate::events::RejectionOutcome::Resume => "resume",
        crate::events::RejectionOutcome::Block => "block",
    }
}

/// Publish the reported blocker facts a hold carries.
///
/// The reducer classifies these facts into a lifecycle status; the stream
/// publishes the facts themselves so a controller can explain the hold without
/// re-reading a snapshot.
fn describe_blocker(blocker: &crate::events::StalledBlocker) -> serde_json::Value {
    let sanitize = crate::events::sanitize_detail;
    json!({
        "category": sanitize(&blocker.category),
        "phase": sanitize(&blocker.phase),
        "gate": sanitize(&blocker.gate),
        "detail": sanitize(&blocker.error_summary),
        "evidence": blocker
            .evidence
            .iter()
            .map(|item| sanitize(item))
            .collect::<Vec<_>>(),
        "unblock_condition": blocker.unblock_condition.as_deref().map(sanitize),
        "prerequisite_owner": blocker.prerequisite_owner.as_deref().map(sanitize),
        "next_action": sanitize(&blocker.next_action),
        "resumable": blocker.resumable,
    })
}
