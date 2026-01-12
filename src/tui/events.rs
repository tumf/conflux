//! Event and command types for TUI communication
//!
//! Contains types for communication between TUI and orchestrator.
//! This module now re-exports the unified ExecutionEvent type.

// Re-export unified event types
pub use crate::events::{ExecutionEvent, LogEntry};

// Alias for backward compatibility
pub type OrchestratorEvent = ExecutionEvent;

/// Commands sent from TUI to orchestrator
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Start processing selected changes
    StartProcessing(Vec<String>),
    /// Add a change to the queue dynamically
    AddToQueue(String),
    /// Remove a change from the queue dynamically
    RemoveFromQueue(String),
    /// Approve a change and add it to the queue (used in select/stopped/completed mode)
    ApproveAndQueue(String),
    /// Approve a change without adding to queue (used in running mode)
    ApproveOnly(String),
    /// Unapprove a change and remove it from the queue (used in running/completed mode)
    UnapproveAndDequeue(String),
    /// Stop processing (graceful shutdown)
    #[allow(dead_code)]
    Stop,
    /// Submit a new proposal (execute propose_command with the given text)
    SubmitProposal(String),
}
