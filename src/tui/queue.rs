//! Dynamic queue for runtime change additions
//!
//! This module provides a thread-safe queue for dynamically adding changes
//! during orchestrator execution.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Dynamic queue for runtime change additions
///
/// This struct provides a thread-safe queue for dynamically adding changes
/// during orchestrator execution. TUI pushes change IDs when the user adds
/// them via Space key, and the orchestrator pops them for processing.
#[derive(Clone)]
pub struct DynamicQueue {
    inner: Arc<Mutex<VecDeque<String>>>,
    removed: Arc<Mutex<HashSet<String>>>,
}

impl DynamicQueue {
    /// Create a new empty DynamicQueue
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            removed: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Push a change ID to the queue
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
        true
    }

    /// Pop the next change ID from the queue
    pub async fn pop(&self) -> Option<String> {
        let mut queue = self.inner.lock().await;
        queue.pop_front()
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
}
