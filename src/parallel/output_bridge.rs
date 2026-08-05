//! Bridge between OutputHandler trait and ParallelEvent channel.
//!
//! This module provides adapters that implement OutputHandler and ApplyEventHandler
//! traits, forwarding all output to a ParallelEvent channel for the TUI to display.

use crate::agent::OutputLine;
use crate::events::{ApplyCommitPhase, CommitOutputStream, ExecutionEvent as ParallelEvent};
use crate::execution::apply::ApplyEventHandler;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Apply event handler that sends events to a ParallelEvent channel.
///
/// This allows the unified apply loop (which uses ApplyEventHandler)
/// to work with parallel execution (which uses ParallelEvent channels).
#[derive(Clone)]
pub struct ParallelApplyEventHandler {
    #[allow(dead_code)]
    change_id: String,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
}

impl ParallelApplyEventHandler {
    /// Create a new parallel apply event handler.
    ///
    /// # Arguments
    ///
    /// * `change_id` - The change ID for event tagging
    /// * `event_tx` - Optional event channel sender
    pub fn new(change_id: String, event_tx: Option<mpsc::Sender<ParallelEvent>>) -> Self {
        Self {
            change_id,
            event_tx,
        }
    }
}

impl ApplyEventHandler for ParallelApplyEventHandler {
    fn on_apply_started(&self, change_id: &str, command: &str) {
        info!("Apply started for {}: {}", change_id, command);
        if let Some(ref tx) = self.event_tx {
            // Avoid dropping events when the bounded channel is temporarily full.
            // Apply output can be high-volume; losing events makes CLI/TUI appear stalled.
            let tx = tx.clone();
            let event = ParallelEvent::ApplyStarted {
                change_id: change_id.to_string(),
                command: command.to_string(),
            };
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        }
    }

    fn on_progress_updated(&self, change_id: &str, completed: u32, total: u32) {
        debug!(
            "Progress updated for {}: {}/{}",
            change_id, completed, total
        );
        if let Some(ref tx) = self.event_tx {
            let tx = tx.clone();
            let event = ParallelEvent::ProgressUpdated {
                change_id: change_id.to_string(),
                completed,
                total,
            };
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        }
    }

    fn on_hook_started(&self, change_id: &str, hook_type: &str) {
        debug!("Hook started for {}: {}", change_id, hook_type);
        if let Some(ref tx) = self.event_tx {
            let tx = tx.clone();
            let event = ParallelEvent::HookStarted {
                change_id: change_id.to_string(),
                hook_type: hook_type.to_string(),
            };
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        }
    }

    fn on_hook_completed(&self, change_id: &str, hook_type: &str) {
        debug!("Hook completed for {}: {}", change_id, hook_type);
        if let Some(ref tx) = self.event_tx {
            let tx = tx.clone();
            let event = ParallelEvent::HookCompleted {
                change_id: change_id.to_string(),
                hook_type: hook_type.to_string(),
            };
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        }
    }

    fn on_hook_failed(&self, change_id: &str, hook_type: &str, error: &str) {
        error!("Hook failed for {} ({}): {}", change_id, hook_type, error);
        if let Some(ref tx) = self.event_tx {
            let tx = tx.clone();
            let event = ParallelEvent::HookFailed {
                change_id: change_id.to_string(),
                hook_type: hook_type.to_string(),
                error: error.to_string(),
            };
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        }
    }

    fn on_apply_output(&self, change_id: &str, line: &OutputLine, iteration: u32) {
        let output = match line {
            OutputLine::Stdout(s) | OutputLine::Stderr(s) => s.clone(),
        };
        if let Some(ref tx) = self.event_tx {
            let tx = tx.clone();
            let event = ParallelEvent::ApplyOutput {
                change_id: change_id.to_string(),
                output,
                iteration: Some(iteration),
            };
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        }
    }

    fn on_apply_commit_phase(&self, change_id: &str, phase: ApplyCommitPhase, attempt: u32) {
        debug!(
            "Apply commit phase for {}: {} (attempt {})",
            change_id,
            phase.as_str(),
            attempt
        );
        if let Some(ref tx) = self.event_tx {
            let tx = tx.clone();
            let event = ParallelEvent::ApplyCommitPhase {
                change_id: change_id.to_string(),
                phase,
                attempt,
            };
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        }
    }

    fn on_apply_commit_output(
        &self,
        change_id: &str,
        attempt: u32,
        stream: CommitOutputStream,
        line: &str,
    ) {
        if let Some(ref tx) = self.event_tx {
            let tx = tx.clone();
            let event = ParallelEvent::ApplyCommitOutput {
                change_id: change_id.to_string(),
                attempt,
                stream,
                line: line.to_string(),
            };
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_parallel_apply_event_handler_with_channel() {
        let (tx, mut rx) = mpsc::channel(10);
        let handler = ParallelApplyEventHandler::new("test-change".to_string(), Some(tx));

        handler.on_apply_started("test-change", "test command");
        handler.on_progress_updated("test-change", 5, 10);
        handler.on_hook_started("test-change", "pre_apply");
        handler.on_hook_completed("test-change", "pre_apply");

        // Should receive 4 events (async send)
        for _ in 0..4 {
            timeout(Duration::from_secs(1), rx.recv())
                .await
                .expect("timeout waiting for event")
                .expect("channel closed unexpectedly");
        }
    }

    #[tokio::test]
    async fn test_parallel_apply_event_handler_without_channel() {
        let handler = ParallelApplyEventHandler::new("test-change".to_string(), None);

        // Should not panic when no channel is provided
        handler.on_apply_started("test-change", "test command");
        handler.on_progress_updated("test-change", 5, 10);
        handler.on_hook_started("test-change", "pre_apply");
        handler.on_hook_completed("test-change", "pre_apply");
        handler.on_hook_failed("test-change", "pre_apply", "test error");
        handler.on_apply_commit_phase("test-change", ApplyCommitPhase::Started, 1);
        handler.on_apply_commit_output("test-change", 1, CommitOutputStream::Stdout, "line");
    }

    /// Collect the next `count` events, in order.
    async fn drain(rx: &mut mpsc::Receiver<ParallelEvent>, count: usize) -> Vec<ParallelEvent> {
        let mut events = Vec::new();
        for _ in 0..count {
            events.push(
                timeout(Duration::from_secs(1), rx.recv())
                    .await
                    .expect("timeout waiting for event")
                    .expect("channel closed unexpectedly"),
            );
        }
        events
    }

    /// The bridge must forward finalization presentation to the frontend
    /// channel, with the change, phase, and attempt intact.
    #[tokio::test]
    async fn commit_phase_transitions_reach_the_event_channel() {
        let (tx, mut rx) = mpsc::channel(10);
        let handler = ParallelApplyEventHandler::new("change-a".to_string(), Some(tx));

        handler.on_apply_commit_phase("change-a", ApplyCommitPhase::Started, 3);
        handler.on_apply_commit_phase("change-a", ApplyCommitPhase::Failed, 3);

        let mut observed: Vec<(String, u32)> = drain(&mut rx, 2)
            .await
            .into_iter()
            .map(|event| match event {
                ParallelEvent::ApplyCommitPhase {
                    change_id,
                    phase,
                    attempt,
                } => {
                    assert_eq!(change_id, "change-a");
                    (phase.as_str().to_string(), attempt)
                }
                other => panic!("unexpected event: {other:?}"),
            })
            .collect();
        // The bridge sends each event from its own task, so ordering between
        // two independent sends is not guaranteed; the set is what matters.
        observed.sort();
        assert_eq!(
            observed,
            vec![("failed".to_string(), 3), ("started".to_string(), 3)]
        );
    }

    /// Streamed commit lines must arrive with their stream and attempt so the
    /// frontend can label them; nothing here may collapse duplicates.
    #[tokio::test]
    async fn streamed_commit_lines_reach_the_event_channel_with_attribution() {
        let (tx, mut rx) = mpsc::channel(10);
        let handler = ParallelApplyEventHandler::new("change-a".to_string(), Some(tx));

        handler.on_apply_commit_output("change-a", 2, CommitOutputStream::Stderr, "hook says hi");

        let events = drain(&mut rx, 1).await;
        match &events[0] {
            ParallelEvent::ApplyCommitOutput {
                change_id,
                attempt,
                stream,
                line,
            } => {
                assert_eq!(change_id, "change-a");
                assert_eq!(*attempt, 2);
                assert_eq!(*stream, CommitOutputStream::Stderr);
                assert_eq!(line, "hook says hi");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
