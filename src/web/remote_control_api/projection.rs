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
use crate::web::state::OrchestratorStateSnapshot;

use super::dto::{
    new_hex_id, ChangeResource, CommandRecord, CommandRequest, CommandState, EventCategory,
    EventEnvelope, InstanceSnapshot, SnapshotTotals, MAX_EVENTS, MAX_LOGS,
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
    let changes: Vec<ChangeResource> = source
        .changes
        .iter()
        .map(|change| ChangeResource {
            id: change.id.clone(),
            display_status: change
                .queue_status
                .clone()
                .unwrap_or_else(|| "not queued".to_string()),
            progress_status: change.status.clone(),
            completed_tasks: change.completed_tasks,
            total_tasks: change.total_tasks,
            progress_percent: change.progress_percent,
            dependencies: change.dependencies.clone(),
            iteration_number: change.iteration_number,
        })
        .collect();

    InstanceSnapshot {
        app_mode: source.app_mode.clone(),
        is_resolving: source.is_resolving,
        totals: SnapshotTotals {
            total: changes.len(),
            completed: source.completed_changes,
            in_progress: source.in_progress_changes,
            pending: source.pending_changes,
        },
        changes,
    }
}

/// Wire event type, target change, and sanitized payload for an execution event.
///
/// `ExecutionEvent` is an internal enum, so this is a deliberate projection
/// rather than a serialization of it: only the fields a remote controller needs
/// cross the boundary, and unenumerated variants degrade to a generic
/// `orchestration_event` instead of leaking internal shape.
pub fn describe_event(event: &ExecutionEvent) -> (&'static str, Option<String>, serde_json::Value) {
    use ExecutionEvent as E;
    let (event_type, change_id, detail) = match event {
        E::ProcessingStarted(id) => ("processing_started", Some(id.clone()), None),
        E::ProcessingCompleted(id) => ("processing_completed", Some(id.clone()), None),
        E::ProcessingError { id, error } => {
            ("processing_error", Some(id.clone()), Some(error.clone()))
        }
        E::ApplyStarted { change_id, .. } => ("apply_started", Some(change_id.clone()), None),
        E::ApplyCompleted { change_id, .. } => ("apply_completed", Some(change_id.clone()), None),
        E::ApplyFailed { change_id, error } => {
            ("apply_failed", Some(change_id.clone()), Some(error.clone()))
        }
        E::ApplyOutput { change_id, .. } => ("apply_output", Some(change_id.clone()), None),
        E::ArchiveStarted { change_id, .. } => ("archive_started", Some(change_id.clone()), None),
        E::ArchiveResumed {
            change_id, reason, ..
        } => ("archive_resumed", Some(change_id.clone()), reason.clone()),
        E::ArchiveRetryScheduled {
            change_id, reason, ..
        } => (
            "archive_retry_scheduled",
            Some(change_id.clone()),
            reason.clone(),
        ),
        E::ChangeArchived(id) => ("change_archived", Some(id.clone()), None),
        E::ArchiveFailed {
            change_id, error, ..
        } => (
            "archive_failed",
            Some(change_id.clone()),
            Some(error.clone()),
        ),
        E::ArchiveOutput { change_id, .. } => ("archive_output", Some(change_id.clone()), None),
        E::AcceptanceStarted { change_id, .. } => {
            ("acceptance_started", Some(change_id.clone()), None)
        }
        E::AcceptanceCompleted { change_id } => {
            ("acceptance_completed", Some(change_id.clone()), None)
        }
        E::AcceptanceFailed { change_id, error } => (
            "acceptance_failed",
            Some(change_id.clone()),
            Some(error.clone()),
        ),
        E::ChangeRejected { change_id, reason } => (
            "change_rejected",
            Some(change_id.clone()),
            Some(reason.clone()),
        ),
        E::RejectionReviewCompleted { change_id, .. } => {
            ("rejection_review_completed", Some(change_id.clone()), None)
        }
        E::RejectionReviewFailed { change_id, error } => (
            "rejection_review_failed",
            Some(change_id.clone()),
            Some(error.clone()),
        ),
        E::AcceptanceOutput { change_id, .. } => {
            ("acceptance_output", Some(change_id.clone()), None)
        }
        E::ProgressUpdated { change_id, .. } => ("progress_updated", Some(change_id.clone()), None),
        E::Log(entry) => ("log", entry.change_id.clone(), None),
        _ => ("orchestration_event", None, None),
    };

    let payload = match &detail {
        Some(detail) => json!({ "detail": detail }),
        None => json!({}),
    };
    (event_type, change_id, payload)
}
