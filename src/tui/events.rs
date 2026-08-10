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
use crate::tui::types::DeleteIntent;

/// Presentation-only observations the local TUI refresh task hands to the render loop.
///
/// Deliberately *not* an [`ExecutionEvent`]: nothing carried here is a workflow
/// fact. It never reaches the orchestration reducer, the shared state store, the
/// operator snapshot, or `/api/v2`, so this channel cannot change a published
/// contract or a next-action decision — only what the header draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiRefreshObservation {
    /// One **successful** dirty-state read of the repository root captured at
    /// TUI startup.
    ///
    /// A failed read publishes nothing at all. That absence is what preserves
    /// the last successful observation instead of reporting an unobservable
    /// workspace as clean.
    WorkspaceDirty { dirty: bool },
}

/// Commands sent from TUI to orchestrator
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Start processing selected changes.
    ///
    /// This is also the retry intent: retry is start in `Error` mode, so the
    /// shared run-control service decides start vs. resume vs. retry from the
    /// mode it is given rather than from a separate command variant.
    StartProcessing(Vec<String>),
    /// Add a change to the queue dynamically.
    ///
    /// Explicit queue intent only. No key press produces it any more: Space and
    /// bulk `x` write execution marks, which never alias onto queue membership.
    /// The adapter is retained so the explicit queue service stays reachable
    /// from the TUI command channel exactly as `/api/v2 set_queue_intent` is.
    #[allow(dead_code)]
    AddToQueue(String),
    /// Remove a change from the queue dynamically. Explicit intent only; see
    /// [`TuiCommand::AddToQueue`].
    #[allow(dead_code)]
    RemoveFromQueue(String),
    /// Stop processing (graceful shutdown)
    #[allow(dead_code)]
    Stop,
    /// Cancel a pending stop request
    CancelStop,
    /// Force stop immediately
    ForceStop,
    /// Delete a worktree confirmed in the worktree view.
    ///
    /// The intent carries the identity the confirmation was taken against — it
    /// is revalidated against a fresh observation before the mutation — plus the
    /// teardown and dirty-discard permissions the operator actually granted.
    DeleteWorktree(DeleteIntent),
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
