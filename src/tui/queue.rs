//! Dynamic queue for runtime change additions
//!
//! This module provides a thread-safe queue for dynamically adding changes
//! during orchestrator execution.

use std::collections::VecDeque;
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
}

impl DynamicQueue {
    /// Create a new empty DynamicQueue
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Push a change ID to the queue
    /// Returns false if the ID is already in the queue
    pub async fn push(&self, id: String) -> bool {
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
}
