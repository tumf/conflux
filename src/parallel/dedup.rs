//! Shared in-memory diagnostic deduplication helpers.
//!
//! The store is runtime-only observability state. It must never be persisted or
//! used as an authoritative workflow-control input.

use std::collections::HashSet;
use std::future::Future;
use std::hash::Hash;

/// Unified key space for operator-visible diagnostic deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum DiagnosticDeduplicationKey {
    QueueReconciliation {
        change_id: String,
        reason: String,
    },
    NoAnalysis {
        reducer_queued: Vec<String>,
        queued_len: usize,
        in_flight_len: usize,
        reason: String,
    },
    DispatchCapacityZero {
        order: Vec<String>,
        queued_len: usize,
        in_flight_len: usize,
        max_parallelism: usize,
        reason: String,
    },
    AnalysisFailure {
        queued_ids: Vec<String>,
        in_flight_ids: Vec<String>,
        error: String,
    },
    /// Ordinary timer analysis skipped because the analysis input was already analyzed.
    UnchangedAnalysisInput {
        signature: String,
        queued_len: usize,
        in_flight_len: usize,
        degraded: bool,
    },
    /// Analysis-input signature construction failed, so suppression was not applied.
    AnalysisSignatureUnavailable {
        queued_len: usize,
        in_flight_len: usize,
        error: String,
    },
    /// Ordinary timer analysis is waiting for a bounded fail-open retry deadline.
    ///
    /// Nothing was recorded as analyzed; the wake is only rate-limited, so the reason is
    /// reported separately from unchanged-input suppression.
    BoundedAnalysisRetryPending {
        cause: &'static str,
        queued_len: usize,
        in_flight_len: usize,
    },
    DependencyBlocker {
        change_id: String,
        fingerprint: Vec<(String, String)>,
    },
    TuiAnalysisStarted {
        remaining_changes: usize,
        attempt_id: String,
    },
    TuiMergeDeferred {
        change_id: String,
        reason: String,
        auto_resumable: bool,
    },
}

/// In-memory set-backed diagnostic deduplication store.
#[derive(Debug, Clone)]
pub(crate) struct DiagnosticDeduplicationStore<K> {
    seen: HashSet<K>,
}

impl<K> Default for DiagnosticDeduplicationStore<K>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K> DiagnosticDeduplicationStore<K>
where
    K: Eq + Hash,
{
    pub(crate) fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    /// Records `key` and returns true only when this is the first observation.
    pub(crate) fn should_emit(&mut self, key: K) -> bool {
        self.seen.insert(key)
    }

    /// Emits once for `key`; duplicate keys execute `on_suppressed` instead.
    pub(crate) async fn emit_or_suppress<Emit, EmitFuture, Suppressed>(
        &mut self,
        key: K,
        emit: Emit,
        on_suppressed: Suppressed,
    ) -> bool
    where
        Emit: FnOnce() -> EmitFuture,
        EmitFuture: Future<Output = ()>,
        Suppressed: FnOnce(),
    {
        if !self.should_emit(key) {
            on_suppressed();
            return false;
        }

        emit().await;
        true
    }

    #[allow(dead_code)]
    pub(crate) fn reset(&mut self) {
        self.seen.clear();
    }

    pub(crate) fn reset_matching(&mut self, mut should_reset: impl FnMut(&K) -> bool) {
        self.seen.retain(|key| !should_reset(key));
    }
}

#[cfg(test)]
mod tests {
    use super::DiagnosticDeduplicationStore;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn new_store_emits_once_and_suppresses_duplicates() {
        let mut store = DiagnosticDeduplicationStore::new();
        let emitted = Arc::new(AtomicUsize::new(0));
        let suppressed = Arc::new(AtomicUsize::new(0));

        let first_emitted = Arc::clone(&emitted);
        let first_suppressed = Arc::clone(&suppressed);
        let first = store
            .emit_or_suppress(
                "diagnostic-key",
                move || async move {
                    first_emitted.fetch_add(1, Ordering::SeqCst);
                },
                move || {
                    first_suppressed.fetch_add(1, Ordering::SeqCst);
                },
            )
            .await;

        let duplicate_emitted = Arc::clone(&emitted);
        let duplicate_suppressed = Arc::clone(&suppressed);
        let duplicate = store
            .emit_or_suppress(
                "diagnostic-key",
                move || async move {
                    duplicate_emitted.fetch_add(1, Ordering::SeqCst);
                },
                move || {
                    duplicate_suppressed.fetch_add(1, Ordering::SeqCst);
                },
            )
            .await;

        assert!(first);
        assert!(!duplicate);
        assert_eq!(emitted.load(Ordering::SeqCst), 1);
        assert_eq!(suppressed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn reset_clears_seen_keys() {
        let mut store = DiagnosticDeduplicationStore::new();
        assert!(store.should_emit("key"));
        assert!(!store.should_emit("key"));
        store.reset();
        assert!(store.should_emit("key"));
    }
}
