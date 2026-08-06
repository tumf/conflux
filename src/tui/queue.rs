//! Dynamic queue for runtime change additions
//!
//! This module provides a thread-safe queue for dynamically adding changes
//! during orchestrator execution.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tokio_util::sync::CancellationToken;

/// Execution handle registered while a change owns a running workspace task.
///
/// `cancel` requests termination; `done` is cancelled by the executor when the
/// task actually returned, which is the only observable proof that the process
/// and task for the change exited.
#[derive(Clone)]
struct ChangeExecutionHandle {
    cancel: CancellationToken,
    done: CancellationToken,
}

/// Outcome of clearing the execution registry on a cancellation exit.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExecutionHandleRelease {
    /// Changes whose `done` handshake fired because command cleanup was confirmed.
    pub confirmed: usize,
    /// Changes released without confirmed cleanup; their `done` stayed unfired.
    pub unconfirmed: Vec<String>,
}

/// Dynamic queue for runtime change additions
///
/// This struct provides a thread-safe queue for dynamically adding changes
/// during orchestrator execution. TUI pushes change IDs when the user adds
/// them via Space key, and the orchestrator pops them for processing.
///
/// The queue uses a `Notify` to wake up the re-analysis loop immediately
/// when new items are added, enabling event-driven re-analysis without
/// relying on completion events or polling.
#[derive(Clone)]
pub struct DynamicQueue {
    inner: Arc<Mutex<VecDeque<String>>>,
    removed: Arc<Mutex<HashSet<String>>>,
    /// Set of change IDs that have been stopped
    stopped: Arc<Mutex<HashSet<String>>>,
    /// Per-change execution handles for immediate force-kill and termination waiting
    kill_tokens: Arc<Mutex<HashMap<String, ChangeExecutionHandle>>>,
    /// Change IDs whose accepted, state-changing `RetryError` admission has not
    /// been consumed by the scheduler yet.
    ///
    /// This is a target-ID-bearing one-shot edge, deliberately separate from the
    /// ordinary queue and from the generic notification: only an explicit retry
    /// may release a change's ephemeral failed classification, so a plain
    /// `AddToQueue` or a generic wake must not be able to look like one.
    explicit_retries: Arc<Mutex<HashSet<String>>>,
    /// Notification for queue changes (used to wake up re-analysis loop)
    notify: Arc<Notify>,
}

impl DynamicQueue {
    /// Create a new empty DynamicQueue
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            removed: Arc::new(Mutex::new(HashSet::new())),
            stopped: Arc::new(Mutex::new(HashSet::new())),
            kill_tokens: Arc::new(Mutex::new(HashMap::new())),
            explicit_retries: Arc::new(Mutex::new(HashSet::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Publish an accepted explicit-retry edge for `id` and wake the scheduler.
    ///
    /// Callers must only publish after a `ReducerCommand::RetryError` returned
    /// `ReduceOutcome::Changed`: a refused or no-op retry proves nothing about
    /// the change and must not release its failed classification.
    ///
    /// Returns true when this is a newly armed edge rather than a duplicate that
    /// is still waiting to be consumed.
    pub async fn publish_explicit_retry(&self, id: String) -> bool {
        let armed = {
            let mut retries = self.explicit_retries.lock().await;
            retries.insert(id)
        };
        // Notify regardless: a duplicate publication still deserves a wake so an
        // idle persistent scheduler re-evaluates promptly.
        self.notify.notify_one();
        armed
    }

    /// Take every pending explicit-retry target, leaving the set empty.
    ///
    /// One-shot by construction: an edge is consumed exactly once, so a later
    /// timer wake cannot replay it.
    pub async fn drain_explicit_retries(&self) -> Vec<String> {
        let mut retries = self.explicit_retries.lock().await;
        let mut drained: Vec<String> = retries.drain().collect();
        drained.sort_unstable();
        drained
    }

    /// Push a change ID to the queue and notify waiting listeners
    /// Returns false if the ID is already in the queue
    pub async fn push(&self, id: String) -> bool {
        {
            let mut removed = self.removed.lock().await;
            removed.remove(&id);
        }
        let mut queue = self.inner.lock().await;
        if queue.contains(&id) {
            return false;
        }
        queue.push_back(id);
        drop(queue); // Release lock before notifying

        // Notify waiting re-analysis loop
        self.notify.notify_one();
        true
    }

    /// Pop the next change ID from the queue
    pub async fn pop(&self) -> Option<String> {
        let mut queue = self.inner.lock().await;
        queue.pop_front()
    }

    /// Put back a hint the scheduler popped but could not yet dispose of.
    ///
    /// This preserves an existing wake edge rather than creating one, so unlike
    /// [`Self::push`] it clears no pending removal marker and emits no
    /// notification: the scheduler is the one holding the hint, and it is about
    /// to re-evaluate anyway. Restoring at the head keeps queue order intact.
    ///
    /// Returns false when the ID is already queued, in which case the edge is
    /// already represented and nothing needs restoring.
    pub async fn requeue_front(&self, id: String) -> bool {
        let mut queue = self.inner.lock().await;
        if queue.contains(&id) {
            return false;
        }
        queue.push_front(id);
        true
    }

    /// Remove a specific change ID from the queue
    /// Returns true if the ID was found and removed, false otherwise
    pub async fn remove(&self, id: &str) -> bool {
        let mut queue = self.inner.lock().await;
        if let Some(pos) = queue.iter().position(|i| i == id) {
            queue.remove(pos);
            true
        } else {
            false
        }
    }

    /// Mark a change ID as removed from the pending set
    /// Returns true if the ID was newly marked for removal
    pub async fn mark_removed(&self, id: String) -> bool {
        let mut removed = self.removed.lock().await;
        removed.insert(id)
    }

    /// Drain all removed IDs for pending removal processing
    #[cfg(test)]
    pub async fn drain_removed(&self) -> Vec<String> {
        let mut removed = self.removed.lock().await;
        removed.drain().collect()
    }

    /// Check if the queue is empty
    #[cfg(test)]
    pub async fn is_empty(&self) -> bool {
        let queue = self.inner.lock().await;
        queue.is_empty()
    }

    /// Check if an ID is already in the queue
    #[cfg(test)]
    pub async fn contains(&self, id: &str) -> bool {
        let queue = self.inner.lock().await;
        queue.iter().any(|i| i == id)
    }

    /// Get the current queue length
    #[cfg(test)]
    pub async fn len(&self) -> usize {
        let queue = self.inner.lock().await;
        queue.len()
    }

    /// Get a future that completes when the queue is notified
    /// This is used by the re-analysis loop to wake up when new items are added
    pub fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.notify.notified()
    }

    /// Notify the scheduler without enqueuing a new change.
    ///
    /// This is used for slot-release events such as manual resolve completion,
    /// where queued work should be reconsidered immediately even though the queue contents
    /// themselves did not change.
    pub fn notify_scheduler(&self) {
        self.notify.notify_one();
    }

    /// Mark a change ID as stopped (single-change stop)
    /// Returns true if the ID was newly marked for stopping
    pub async fn mark_stopped(&self, id: String) -> bool {
        let mut stopped = self.stopped.lock().await;
        stopped.insert(id)
    }

    /// Check if a change ID is marked as stopped
    pub async fn is_stopped(&self, id: &str) -> bool {
        let stopped = self.stopped.lock().await;
        stopped.contains(id)
    }

    /// Clear the stopped marker for a change ID (e.g., after stop completion)
    pub async fn clear_stopped(&self, id: &str) -> bool {
        let mut stopped = self.stopped.lock().await;
        stopped.remove(id)
    }

    /// Register a per-change cancellation token for immediate force-kill.
    /// Called by the parallel executor when spawning a workspace task.
    pub async fn register_kill_token(&self, id: String, token: CancellationToken) {
        let mut tokens = self.kill_tokens.lock().await;
        tokens.insert(
            id,
            ChangeExecutionHandle {
                cancel: token,
                done: CancellationToken::new(),
            },
        );
    }

    /// Unregister a per-change cancellation token (cleanup on task completion).
    ///
    /// This is the termination handshake: the executor only reaches this point
    /// after the workspace task returned, so waiters observing `done` learn that
    /// the task and its child process actually exited.
    pub async fn unregister_kill_token(&self, id: &str) {
        let handle = {
            let mut tokens = self.kill_tokens.lock().await;
            tokens.remove(id)
        };
        if let Some(handle) = handle {
            handle.done.cancel();
        }
    }

    /// Clear the execution registry after cancellation, firing `done` only for
    /// changes whose run-owned commands reached confirmed process cleanup.
    ///
    /// Ordinary completion releases one handle at a time through
    /// [`Self::unregister_kill_token`]. Operator cancellation aborts in-flight
    /// workspace tasks instead of completing them, so the scheduler releases the
    /// registry itself on its cancellation exit: this queue outlives a single run
    /// (one instance per TUI session), so a handle left behind by an aborted task
    /// would later be read as proof that an agent process is still running and
    /// would misclassify a subsequent idle stop as a force stop.
    ///
    /// Dropping a workspace future is not completion evidence, though. `done`
    /// means "the task and its child process exited", so it fires only where
    /// `is_quiescent` confirms it; an unconfirmed change leaves its waiter
    /// pending for its own bounded timeout rather than receiving a false
    /// completion.
    pub async fn release_all_execution_handles<F>(&self, is_quiescent: F) -> ExecutionHandleRelease
    where
        F: Fn(&str) -> bool,
    {
        let handles = {
            let mut tokens = self.kill_tokens.lock().await;
            std::mem::take(&mut *tokens)
        };
        let mut release = ExecutionHandleRelease::default();
        for (change_id, handle) in handles {
            if is_quiescent(&change_id) {
                handle.done.cancel();
                release.confirmed += 1;
            } else {
                release.unconfirmed.push(change_id);
            }
        }
        release.unconfirmed.sort();
        release
    }

    /// Number of currently registered per-change execution handles.
    ///
    /// A handle is registered exactly while a workspace task (the agent command
    /// path) owns a change, so a zero count is positive evidence that no agent
    /// process is running and an immediate stop must not claim force termination.
    pub async fn registered_execution_count(&self) -> usize {
        let tokens = self.kill_tokens.lock().await;
        tokens.len()
    }

    /// Mark a change stopped, cancel its execution token, and return the token that
    /// the executor cancels once the workspace task actually returned.
    ///
    /// Returns `None` when no cancellation handle is registered for the change.
    pub async fn request_cancellation(&self, id: &str) -> Option<CancellationToken> {
        self.mark_stopped(id.to_string()).await;
        let tokens = self.kill_tokens.lock().await;
        let handle = tokens.get(id)?;
        handle.cancel.cancel();
        Some(handle.done.clone())
    }
}

impl Default for DynamicQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dynamic_queue_push_pop() {
        let queue = DynamicQueue::new();

        assert!(queue.is_empty().await);

        // Push items
        assert!(queue.push("a".to_string()).await);
        assert!(queue.push("b".to_string()).await);

        assert_eq!(queue.len().await, 2);

        // Pop in FIFO order
        assert_eq!(queue.pop().await, Some("a".to_string()));
        assert_eq!(queue.pop().await, Some("b".to_string()));
        assert_eq!(queue.pop().await, None);
    }

    #[tokio::test]
    async fn test_dynamic_queue_dedup() {
        let queue = DynamicQueue::new();

        // First push succeeds
        assert!(queue.push("a".to_string()).await);

        // Duplicate push fails
        assert!(!queue.push("a".to_string()).await);

        assert_eq!(queue.len().await, 1);
    }

    #[tokio::test]
    async fn test_dynamic_queue_contains() {
        let queue = DynamicQueue::new();

        queue.push("a".to_string()).await;

        assert!(queue.contains("a").await);
        assert!(!queue.contains("b").await);
    }

    #[tokio::test]
    async fn test_dynamic_queue_remove() {
        let queue = DynamicQueue::new();

        queue.push("a".to_string()).await;
        queue.push("b".to_string()).await;
        queue.push("c".to_string()).await;

        assert_eq!(queue.len().await, 3);

        // Remove middle item
        assert!(queue.remove("b").await);
        assert_eq!(queue.len().await, 2);
        assert!(!queue.contains("b").await);

        // Order preserved: a, c
        assert_eq!(queue.pop().await, Some("a".to_string()));
        assert_eq!(queue.pop().await, Some("c".to_string()));
    }

    #[tokio::test]
    async fn test_dynamic_queue_remove_nonexistent() {
        let queue = DynamicQueue::new();

        queue.push("a".to_string()).await;

        // Remove non-existent item returns false
        assert!(!queue.remove("nonexistent").await);
        assert_eq!(queue.len().await, 1);
    }

    #[tokio::test]
    async fn test_dynamic_queue_remove_from_empty() {
        let queue = DynamicQueue::new();

        // Remove from empty queue returns false
        assert!(!queue.remove("a").await);
    }

    #[tokio::test]
    async fn test_dynamic_queue_remove_multiple() {
        let queue = DynamicQueue::new();

        queue.push("a".to_string()).await;
        queue.push("b".to_string()).await;
        queue.push("c".to_string()).await;

        // Remove first and last
        assert!(queue.remove("a").await);
        assert!(queue.remove("c").await);

        assert_eq!(queue.len().await, 1);
        assert_eq!(queue.pop().await, Some("b".to_string()));
    }

    #[tokio::test]
    async fn test_dynamic_queue_remove_then_push_same() {
        let queue = DynamicQueue::new();

        queue.push("a".to_string()).await;
        assert!(queue.remove("a").await);

        // Should be able to push the same item again
        assert!(queue.push("a".to_string()).await);
        assert_eq!(queue.len().await, 1);
    }

    #[tokio::test]
    async fn test_mark_removed_and_drain() {
        let queue = DynamicQueue::new();

        assert!(queue.mark_removed("a".to_string()).await);
        assert!(!queue.mark_removed("a".to_string()).await);
        assert!(queue.mark_removed("b".to_string()).await);

        let mut removed = queue.drain_removed().await;
        removed.sort();
        assert_eq!(removed, vec!["a".to_string(), "b".to_string()]);
        assert!(queue.drain_removed().await.is_empty());
    }

    #[tokio::test]
    async fn test_push_clears_removed_marker() {
        let queue = DynamicQueue::new();

        assert!(queue.mark_removed("a".to_string()).await);
        assert!(queue.push("a".to_string()).await);

        let removed = queue.drain_removed().await;
        assert!(removed.is_empty());
    }

    #[tokio::test]
    async fn test_request_cancellation_marks_stopped_and_cancels_token() {
        let queue = DynamicQueue::new();
        let token = CancellationToken::new();
        queue
            .register_kill_token("a".to_string(), token.clone())
            .await;

        assert!(!token.is_cancelled());
        let done = queue
            .request_cancellation("a")
            .await
            .expect("a registered handle must be returned");
        assert!(token.is_cancelled());
        assert!(queue.is_stopped("a").await);
        assert!(
            !done.is_cancelled(),
            "termination must not be reported before the task completes"
        );
    }

    #[tokio::test]
    async fn test_request_cancellation_without_token_still_marks_stopped() {
        let queue = DynamicQueue::new();

        assert!(queue.request_cancellation("b").await.is_none());
        assert!(queue.is_stopped("b").await);
    }

    /// `done` means the task *and its child process* exited.
    ///
    /// The regression this pins: the cancellation exit used to fire every
    /// registered handshake just because `JoinSet::abort_all` had dropped the
    /// workspace futures. Dropping a future proves nothing about the process
    /// group it owned, so a waiter could be told an agent had finished while it
    /// was still editing the worktree.
    #[tokio::test]
    async fn execution_done_requires_process_quiescence() {
        let queue = DynamicQueue::new();
        for change in ["confirmed", "unconfirmed"] {
            queue
                .register_kill_token(change.to_string(), CancellationToken::new())
                .await;
        }
        let confirmed_done = queue
            .request_cancellation("confirmed")
            .await
            .expect("handle registered");
        let unconfirmed_done = queue
            .request_cancellation("unconfirmed")
            .await
            .expect("handle registered");

        // The workspace futures have just been aborted. Nothing has proven
        // anything about their process groups yet.
        assert!(
            !confirmed_done.is_cancelled() && !unconfirmed_done.is_cancelled(),
            "requesting cancellation is not completion evidence"
        );

        // Only `confirmed` has matching run-scope cleanup evidence.
        let release = queue
            .release_all_execution_handles(|change_id| change_id == "confirmed")
            .await;

        assert_eq!(release.confirmed, 1);
        assert_eq!(release.unconfirmed, vec!["unconfirmed".to_string()]);
        assert!(
            confirmed_done.is_cancelled(),
            "confirmed terminal cleanup releases the handshake"
        );
        assert!(
            !unconfirmed_done.is_cancelled(),
            "an aborted task without cleanup evidence must not report completion"
        );

        // The waiter for the unconfirmed change is left to its own bounded
        // timeout rather than receiving a false completion.
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(30),
                unconfirmed_done.cancelled()
            )
            .await
            .is_err(),
            "the unconfirmed waiter times out truthfully"
        );

        // The registry itself is empty either way, so a later idle stop in the
        // same session cannot read a stale handle as a live agent process.
        assert_eq!(queue.registered_execution_count().await, 0);
    }

    #[tokio::test]
    async fn test_unregister_kill_token_confirms_termination() {
        let queue = DynamicQueue::new();
        let token = CancellationToken::new();
        queue
            .register_kill_token("a".to_string(), token.clone())
            .await;
        let done = queue
            .request_cancellation("a")
            .await
            .expect("handle registered");

        queue.unregister_kill_token("a").await;

        assert!(
            done.is_cancelled(),
            "task completion must confirm termination to waiters"
        );
        assert!(
            queue.request_cancellation("a").await.is_none(),
            "an unregistered change has no cancellation handle"
        );
    }
}
