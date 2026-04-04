//! Event and command types for TUI communication
//!
//! Contains types for communication between TUI and orchestrator.
//! This module now re-exports the unified ExecutionEvent type.

// Re-export unified event types
pub use crate::events::{ExecutionEvent, LogEntry, LogLevel};

// Alias for backward compatibility
pub type OrchestratorEvent = ExecutionEvent;

use std::path::PathBuf;

/// Event sink implementation for TUI event channel.
pub struct TuiEventSink {
    tx: mpsc::Sender<OrchestratorEvent>,
}

impl TuiEventSink {
    pub fn new(tx: mpsc::Sender<OrchestratorEvent>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl EventSink for TuiEventSink {
    async fn on_event(&self, event: &ExecutionEvent) {
        if let Err(err) = self.tx.send(event.clone()).await {
            warn!(error = %err, "failed to send TUI event through sink");
        }
    }

    async fn on_state_changed(&self, _state: &OrchestratorState) {}
}

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::warn;

use crate::events::EventSink;
use crate::orchestration::state::OrchestratorState;

/// Commands sent from TUI to orchestrator
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Start processing selected changes
    StartProcessing(Vec<String>),
    /// Add a change to the queue dynamically
    AddToQueue(String),
    /// Remove a change from the queue dynamically
    RemoveFromQueue(String),
    /// Stop processing (graceful shutdown)
    #[allow(dead_code)]
    Stop,
    /// Cancel a pending stop request
    CancelStop,
    /// Force stop immediately
    ForceStop,
    /// Retry error changes
    Retry,
    /// Delete a worktree by path (from worktree view)
    /// The optional String is the branch name to delete after worktree removal
    DeleteWorktreeByPath(PathBuf, Option<String>),
    /// Resolve a deferred merge for a change
    ResolveMerge(String),
    /// Merge a worktree branch into the base branch
    MergeWorktreeBranch {
        worktree_path: PathBuf,
        branch_name: String,
    },
    /// Force-stop and dequeue a single active change (during Running mode)
    DequeueChange(String),
}
