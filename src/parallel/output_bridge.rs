//! Bridge between OutputHandler trait and ParallelEvent channel.
//!
//! This module provides adapters that implement OutputHandler and ApplyEventHandler
//! traits, forwarding all output to a ParallelEvent channel for the TUI to display.

use crate::agent::OutputLine;
use crate::events::{ExecutionEvent as ParallelEvent, LogEntry};
use crate::execution::apply::ApplyEventHandler;
use crate::orchestration::output::OutputHandler;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Output handler that sends events to a ParallelEvent channel.
///
/// This allows the common orchestration loops (which use OutputHandler)
/// to work with parallel execution (which uses ParallelEvent channels).
#[derive(Clone)]
#[allow(dead_code)]
pub struct ParallelOutputHandler {
    change_id: String,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
}

#[allow(dead_code)]
impl ParallelOutputHandler {
    /// Create a new parallel output handler.
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

impl OutputHandler for ParallelOutputHandler {
    fn on_stdout(&self, line: &str) {
        debug!(target: "parallel::output", "{}: {}", self.change_id, line);
        // Stdout is not typically sent as events in parallel mode
        // It's captured in apply/archive output events
    }

    fn on_stderr(&self, line: &str) {
        warn!(target: "parallel::output", "{}: {}", self.change_id, line);
        // Stderr is not typically sent as events in parallel mode
        // It's captured in apply/archive output events
    }

    fn on_info(&self, message: &str) {
        info!("{}", message);
        if let Some(ref tx) = self.event_tx {
            let log = LogEntry::info(message.to_string()).with_change_id(&self.change_id);
            let _ = tx.try_send(ParallelEvent::Log(log));
        }
    }

    fn on_warn(&self, message: &str) {
        warn!("{}", message);
        if let Some(ref tx) = self.event_tx {
            let log = LogEntry::warn(message.to_string()).with_change_id(&self.change_id);
            let _ = tx.try_send(ParallelEvent::Log(log));
        }
    }

    fn on_error(&self, message: &str) {
        error!("{}", message);
        if let Some(ref tx) = self.event_tx {
            let log = LogEntry::error(message.to_string()).with_change_id(&self.change_id);
            let _ = tx.try_send(ParallelEvent::Log(log));
        }
    }

    fn on_success(&self, message: &str) {
        info!("{}", message);
        if let Some(ref tx) = self.event_tx {
            let log = LogEntry::success(message.to_string()).with_change_id(&self.change_id);
            let _ = tx.try_send(ParallelEvent::Log(log));
        }
    }
}

/// Apply event handler that sends events to a ParallelEvent channel.
///
/// This allows the unified apply loop (which uses ApplyEventHandler)
/// to work with parallel execution (which uses ParallelEvent channels).
#[derive(Clone)]
#[allow(dead_code)]
pub struct ParallelApplyEventHandler {
    #[allow(dead_code)]
    change_id: String,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
}

#[allow(dead_code)]
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
            let _ = tx.try_send(ParallelEvent::ApplyStarted {
                change_id: change_id.to_string(),
                command: command.to_string(),
            });
        }
    }

    fn on_progress_updated(&self, change_id: &str, completed: u32, total: u32) {
        debug!(
            "Progress updated for {}: {}/{}",
            change_id, completed, total
        );
        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(ParallelEvent::ProgressUpdated {
                change_id: change_id.to_string(),
                completed,
                total,
            });
        }
    }

    fn on_hook_started(&self, change_id: &str, hook_type: &str) {
        debug!("Hook started for {}: {}", change_id, hook_type);
        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(ParallelEvent::HookStarted {
                change_id: change_id.to_string(),
                hook_type: hook_type.to_string(),
            });
        }
    }

    fn on_hook_completed(&self, change_id: &str, hook_type: &str) {
        debug!("Hook completed for {}: {}", change_id, hook_type);
        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(ParallelEvent::HookCompleted {
                change_id: change_id.to_string(),
                hook_type: hook_type.to_string(),
            });
        }
    }

    fn on_hook_failed(&self, change_id: &str, hook_type: &str, error: &str) {
        error!("Hook failed for {} ({}): {}", change_id, hook_type, error);
        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(ParallelEvent::HookFailed {
                change_id: change_id.to_string(),
                hook_type: hook_type.to_string(),
                error: error.to_string(),
            });
        }
    }

    fn on_apply_output(&self, change_id: &str, line: &OutputLine, iteration: u32) {
        let output = match line {
            OutputLine::Stdout(s) | OutputLine::Stderr(s) => s.clone(),
        };
        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(ParallelEvent::ApplyOutput {
                change_id: change_id.to_string(),
                output,
                iteration: Some(iteration),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parallel_output_handler_with_channel() {
        let (tx, mut rx) = mpsc::channel(10);
        let handler = ParallelOutputHandler::new("test-change".to_string(), Some(tx));

        handler.on_info("info message");
        handler.on_warn("warn message");
        handler.on_error("error message");
        handler.on_success("success message");

        // Should receive 4 log events
        for _ in 0..4 {
            assert!(rx.try_recv().is_ok());
        }
    }

    #[tokio::test]
    async fn test_parallel_output_handler_without_channel() {
        let handler = ParallelOutputHandler::new("test-change".to_string(), None);

        // Should not panic when no channel is provided
        handler.on_info("info message");
        handler.on_warn("warn message");
        handler.on_error("error message");
        handler.on_success("success message");
        handler.on_stdout("stdout");
        handler.on_stderr("stderr");
    }

    #[tokio::test]
    async fn test_parallel_apply_event_handler_with_channel() {
        let (tx, mut rx) = mpsc::channel(10);
        let handler = ParallelApplyEventHandler::new("test-change".to_string(), Some(tx));

        handler.on_apply_started("test-change", "test command");
        handler.on_progress_updated("test-change", 5, 10);
        handler.on_hook_started("test-change", "pre_apply");
        handler.on_hook_completed("test-change", "pre_apply");

        // Should receive 4 events
        for _ in 0..4 {
            assert!(rx.try_recv().is_ok());
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
    }
}
