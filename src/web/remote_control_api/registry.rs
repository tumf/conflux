//! Bounded, fail-closed command and idempotency registries.
//!
//! The two registries are separate (each has its own admission limit) but are
//! always reserved together in one atomic step, so a command record can never
//! exist without the idempotency record that would let a retry find it — which
//! is exactly the state that would let a side effect run twice.
//!
//! Nothing here takes a lock or reads the clock. The projection owner holds the
//! lock and passes `now` in, which makes expiry and capacity behavior directly
//! testable without sleeping.

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};

use super::dto::{
    CommandIdentity, CommandRecord, CommandState, ErrorCode, COMMAND_RECORD_TTL_SECS,
    MAX_COMMAND_RECORDS,
};

/// Result of looking up an idempotency key.
#[derive(Debug, Clone)]
pub enum IdempotencyLookup {
    /// The key is unused in this incarnation.
    Unknown,
    /// The key is bound to a structurally equal command; replay its record.
    Replay(Box<CommandRecord>),
    /// The key is bound to a different typed identity.
    Mismatch,
}

/// Why a reservation could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveError {
    /// No slot could be freed without evicting in-progress work.
    Capacity,
}

/// How a finished command settled.
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    /// Terminal lifecycle state.
    pub state: CommandState,
    /// Revision observed after execution.
    pub result_revision: u64,
    /// Sanitized detail for the operator.
    pub detail: Option<String>,
    /// Typed failure code when `state` is `Failed`.
    pub error_code: Option<ErrorCode>,
}

/// Paired command and idempotency registries with a shared reservation step.
#[derive(Debug)]
pub struct CommandRegistry {
    records: HashMap<String, CommandRecord>,
    record_order: VecDeque<String>,
    /// idempotency key -> (command_id, identity)
    keys: HashMap<String, (String, CommandIdentity)>,
    key_order: VecDeque<String>,
    max_records: usize,
    ttl_secs: i64,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new(MAX_COMMAND_RECORDS, COMMAND_RECORD_TTL_SECS)
    }
}

impl CommandRegistry {
    /// Build a registry with explicit bounds (tests use small ones).
    pub fn new(max_records: usize, ttl_secs: i64) -> Self {
        Self {
            records: HashMap::new(),
            record_order: VecDeque::new(),
            keys: HashMap::new(),
            key_order: VecDeque::new(),
            max_records,
            ttl_secs,
        }
    }

    /// Number of live command records.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn command_len(&self) -> usize {
        self.records.len()
    }

    /// Number of live idempotency records.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn idempotency_len(&self) -> usize {
        self.keys.len()
    }

    /// Fetch a command record by ID.
    pub fn get(&self, command_id: &str) -> Option<&CommandRecord> {
        self.records.get(command_id)
    }

    /// Look up an idempotency key against a typed identity.
    ///
    /// This runs *before* revision validation so that an exact replay keeps
    /// working after the state has moved on, while a new key carrying a stale
    /// revision is still rejected.
    pub fn lookup(
        &mut self,
        key: &str,
        identity: &CommandIdentity,
        now: DateTime<Utc>,
    ) -> IdempotencyLookup {
        self.expire(now);
        let Some((command_id, bound)) = self.keys.get(key) else {
            return IdempotencyLookup::Unknown;
        };
        if bound != identity {
            return IdempotencyLookup::Mismatch;
        }
        match self.records.get(command_id) {
            Some(record) => IdempotencyLookup::Replay(Box::new(record.clone())),
            // The idempotency record outlived its command record. Treating this
            // as a mismatch is the fail-closed choice: re-running the effect
            // would be worse than making the caller pick a new key.
            None => IdempotencyLookup::Mismatch,
        }
    }

    /// Atomically reserve the command record and its idempotency binding.
    ///
    /// Either both records exist afterwards or neither does, and no in-progress
    /// record is ever removed to make room.
    pub fn reserve(
        &mut self,
        key: &str,
        identity: CommandIdentity,
        record: CommandRecord,
        now: DateTime<Utc>,
    ) -> Result<(), ReserveError> {
        self.expire(now);
        // Prove capacity for *both* registries before mutating either one.
        if !self.can_admit_record() || !self.can_admit_key() {
            return Err(ReserveError::Capacity);
        }

        let command_id = record.command_id.clone();
        self.records.insert(command_id.clone(), record);
        self.record_order.push_back(command_id.clone());
        self.keys
            .insert(key.to_string(), (command_id, identity.clone()));
        self.key_order.push_back(key.to_string());
        Ok(())
    }

    /// Settle a reserved command record.
    pub fn complete(&mut self, command_id: &str, outcome: CommandOutcome) -> Option<CommandRecord> {
        let record = self.records.get_mut(command_id)?;
        record.state = outcome.state;
        record.result_revision = Some(outcome.result_revision);
        record.detail = outcome.detail;
        record.error_code = outcome.error_code;
        record.completed_at = Some(Utc::now().to_rfc3339());
        Some(record.clone())
    }

    /// Drop expired completed records (and the keys bound to them).
    fn expire(&mut self, now: DateTime<Utc>) {
        let expired: Vec<String> = self
            .records
            .iter()
            .filter(|(_, record)| self.is_expired(record, now))
            .map(|(id, _)| id.clone())
            .collect();
        for command_id in expired {
            self.forget(&command_id);
        }
    }

    fn is_expired(&self, record: &CommandRecord, now: DateTime<Utc>) -> bool {
        if record.state.is_in_progress() {
            return false;
        }
        let Some(completed_at) = record.completed_at.as_deref() else {
            return false;
        };
        let Ok(completed) = DateTime::parse_from_rfc3339(completed_at) else {
            return false;
        };
        (now - completed.with_timezone(&Utc)).num_seconds() >= self.ttl_secs
    }

    /// Remove a command record and every key bound to it.
    fn forget(&mut self, command_id: &str) {
        self.records.remove(command_id);
        self.record_order.retain(|id| id != command_id);
        let bound_keys: Vec<String> = self
            .keys
            .iter()
            .filter(|(_, (id, _))| id == command_id)
            .map(|(key, _)| key.clone())
            .collect();
        for key in bound_keys {
            self.keys.remove(&key);
            self.key_order.retain(|k| k != &key);
        }
    }

    /// True once a record slot is free, evicting the oldest completed record if needed.
    fn can_admit_record(&mut self) -> bool {
        while self.records.len() >= self.max_records {
            let Some(victim) = self
                .record_order
                .iter()
                .find(|id| {
                    self.records
                        .get(*id)
                        .is_some_and(|r| !r.state.is_in_progress())
                })
                .cloned()
            else {
                // Every record is in progress: fail closed rather than evict work
                // whose side effect is still running.
                return false;
            };
            self.forget(&victim);
        }
        true
    }

    /// True once a key slot is free. Keys bound to in-progress commands are pinned.
    fn can_admit_key(&mut self) -> bool {
        while self.keys.len() >= self.max_records {
            let Some(victim) = self
                .key_order
                .iter()
                .find(|key| {
                    self.keys.get(*key).is_some_and(|(command_id, _)| {
                        self.records
                            .get(command_id)
                            .is_none_or(|r| !r.state.is_in_progress())
                    })
                })
                .cloned()
            else {
                return false;
            };
            self.keys.remove(&victim);
            self.key_order.retain(|k| k != &victim);
        }
        true
    }
}
