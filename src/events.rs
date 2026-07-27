//! Unified event system for OpenSpec Orchestrator
//!
//! This module provides a single event type that unifies the previously separate
//! ParallelEvent (from parallel execution) and OrchestratorEvent (from TUI) types.
//!
//! The ExecutionEvent enum represents all possible events that can occur during
//! change processing, whether in serial or parallel mode.

use std::sync::OnceLock;

use async_trait::async_trait;

use chrono::{Local, Utc};
use ratatui::style::Color;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::debug;

use crate::orchestration::state::OrchestratorState;

#[cfg(feature = "web-monitoring")]
use utoipa::ToSchema;

/// Log level for TUI logs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

/// Log entry for the TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct LogEntry {
    /// Timestamp (formatted for display)
    pub timestamp: String,
    /// Creation time (actual timestamp for relative time calculation)
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Log message
    pub message: String,
    /// Log level color (serialized as RGB string for web)
    #[serde(skip)]
    #[cfg_attr(feature = "web-monitoring", schema(ignore = true))]
    pub color: Color,
    /// Log level
    pub level: LogLevel,
    /// Optional change_id for parallel mode logs
    pub change_id: Option<String>,
    /// Optional operation type (apply, archive, resolve)
    pub operation: Option<String>,
    /// Optional iteration number (for apply operations)
    pub iteration: Option<u32>,
    /// Optional workspace path (for parallel mode logs with workspace context)
    pub workspace_path: Option<String>,
}

fn ansi_csi_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").expect("Invalid ANSI CSI regex"))
}

fn ansi_fragment_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\[[0-9;]{1,}m").expect("Invalid ANSI fragment regex"))
}

pub fn command_log_summary(command: &str) -> String {
    let digest = md5::compute(command.as_bytes());
    format!(
        "Command metadata: bytes={}, hash={:x}",
        command.len(),
        digest
    )
}

fn sanitize_log_message(message: &str) -> String {
    const MAX_LOG_MESSAGE_BYTES: usize = 8_192;

    let without_ansi = ansi_csi_regex().replace_all(message, "");
    let without_fragments = ansi_fragment_regex().replace_all(&without_ansi, "");
    let mut sanitized = String::with_capacity(without_fragments.len());
    for ch in without_fragments.chars() {
        match ch {
            '\n' => sanitized.push_str("\\n"),
            '\r' => sanitized.push_str("\\r"),
            '\t' => sanitized.push_str("\\t"),
            ch if ch.is_control() => {}
            _ => sanitized.push(ch),
        }
    }
    if sanitized.len() <= MAX_LOG_MESSAGE_BYTES {
        return sanitized;
    }

    let mut retained = MAX_LOG_MESSAGE_BYTES;
    loop {
        while !sanitized.is_char_boundary(retained) {
            retained -= 1;
        }
        let marker = format!("…[truncated {} bytes]", sanitized.len() - retained);
        let mut next = MAX_LOG_MESSAGE_BYTES.saturating_sub(marker.len());
        while !sanitized.is_char_boundary(next) {
            next -= 1;
        }
        if next == retained {
            return format!("{}{}", &sanitized[..retained], marker);
        }
        retained = next;
    }
}

impl LogEntry {
    /// Create a new info log entry
    pub fn info(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        let now_local = Local::now();
        let now_utc = Utc::now();
        Self {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            message,
            color: Color::White,
            level: LogLevel::Info,
            change_id: None,
            operation: None,
            iteration: None,
            workspace_path: None,
        }
    }

    /// Create a new success log entry
    pub fn success(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        let now_local = Local::now();
        let now_utc = Utc::now();
        Self {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            message,
            color: Color::Green,
            level: LogLevel::Success,
            change_id: None,
            operation: None,
            iteration: None,
            workspace_path: None,
        }
    }

    /// Create a new warning log entry
    pub fn warn(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        let now_local = Local::now();
        let now_utc = Utc::now();
        Self {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            message,
            color: Color::Yellow,
            level: LogLevel::Warn,
            change_id: None,
            operation: None,
            iteration: None,
            workspace_path: None,
        }
    }

    /// Create a new error log entry
    pub fn error(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        let now_local = Local::now();
        let now_utc = Utc::now();
        Self {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            message,
            color: Color::Red,
            level: LogLevel::Error,
            change_id: None,
            operation: None,
            iteration: None,
            workspace_path: None,
        }
    }

    /// Set change_id for parallel mode logs
    #[allow(dead_code)]
    pub fn with_change_id(mut self, change_id: impl Into<String>) -> Self {
        self.change_id = Some(change_id.into());
        self
    }

    /// Set operation type (apply, archive, resolve)
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    /// Set iteration number (for apply operations)
    pub fn with_iteration(mut self, iteration: u32) -> Self {
        self.iteration = Some(iteration);
        self
    }

    /// Set workspace path (for parallel mode logs with workspace context)
    #[allow(dead_code)]
    pub fn with_workspace_path(mut self, workspace_path: impl Into<String>) -> Self {
        self.workspace_path = Some(workspace_path.into());
        self
    }
}

/// Unified event type for all execution events
///
/// This enum combines events from both serial and parallel execution modes,
/// providing a single interface for event handling across the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionOutcome {
    Confirm,
    Resume,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalledBlocker {
    pub category: String,
    pub phase: String,
    pub gate: String,
    pub error_summary: String,
    pub evidence: Vec<String>,
    pub next_action: String,
    pub resumable: bool,
    pub worktree_preserved: bool,
}

impl StalledBlocker {
    pub fn permission_denial(
        phase: impl Into<String>,
        denial: &crate::permission::PermissionDenial,
    ) -> Self {
        Self {
            category: format!("permission:{}", denial.category.as_str()),
            phase: phase.into(),
            gate: "permission_policy".to_string(),
            error_summary: format!(
                "repeated unresolved permission/tool policy denial for {}: {}; evidence: {}",
                denial.category.as_str(),
                denial.denied_target,
                denial.evidence
            ),
            evidence: vec![denial.evidence.clone()],
            next_action: denial.format_guidance(),
            resumable: true,
            worktree_preserved: true,
        }
    }

    /// Build an acceptance stalled blocker from an **explicitly supplied**
    /// category.
    ///
    /// There is deliberately no prose-classifying constructor. Scanning an error
    /// summary for words like `credential`, `token`, or `auth` produced
    /// confident-looking categories that nothing had verified, so the category
    /// must now come from a validated reviewer payload or from a runtime
    /// classifier that owns the decision (for example permission-denial
    /// classification).
    #[cfg(test)]
    pub fn acceptance_external(
        category: impl Into<String>,
        error_summary: impl Into<String>,
    ) -> Self {
        let error_summary = error_summary.into();
        Self {
            category: category.into(),
            phase: "acceptance".to_string(),
            gate: "acceptance".to_string(),
            evidence: vec![error_summary.clone()],
            error_summary,
            next_action: "resolve the external verification blocker and rerun acceptance"
                .to_string(),
            resumable: true,
            worktree_preserved: true,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "category={}, phase={}, gate={}, evidence={}, resumable={}, next_action={}, error={}",
            self.category,
            self.phase,
            self.gate,
            self.evidence.join(" | "),
            self.resumable,
            self.next_action,
            self.error_summary
        )
    }

    pub fn worktree_snapshot(&self) -> String {
        if self.worktree_preserved {
            "existing worktree and WIP context are preserved while stalled".to_string()
        } else {
            "worktree preservation unavailable for this stalled hold".to_string()
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    // Lifecycle events
    /// Processing started for a change
    ProcessingStarted(String),
    /// Processing completed for a change
    ProcessingCompleted(String),
    /// Error occurred for a change
    ProcessingError { id: String, error: String },

    // Apply events
    /// Apply started in a workspace
    #[allow(dead_code)]
    ApplyStarted { change_id: String, command: String },
    /// Apply completed in a workspace
    ApplyCompleted {
        change_id: String,
        #[allow(dead_code)]
        revision: String,
    },
    /// Apply failed in a workspace
    #[allow(dead_code)]
    ApplyFailed { change_id: String, error: String },
    /// Apply output (summary of command output)
    #[allow(dead_code)]
    ApplyOutput {
        change_id: String,
        output: String,
        iteration: Option<u32>,
    },

    // Archive events
    /// Archive started for a change
    ArchiveStarted { change_id: String, command: String },
    /// Archive resumed from durable resume state context.
    ArchiveResumed {
        change_id: String,
        reason: Option<String>,
        summary: Option<String>,
    },
    /// Archive retry scheduled with structured reason context.
    ArchiveRetryScheduled {
        change_id: String,
        attempt: u32,
        max_attempts: u32,
        reason: Option<String>,
        summary: Option<String>,
    },
    /// Change archived successfully
    ChangeArchived(String),
    /// Change archive failed
    #[allow(dead_code)]
    ArchiveFailed {
        change_id: String,
        error: String,
        reason: Option<String>,
        summary: Option<String>,
    },
    /// Archive output (streaming)
    #[allow(dead_code)]
    ArchiveOutput {
        change_id: String,
        output: String,
        iteration: u32,
    },

    // Acceptance events
    /// Acceptance started for a change
    AcceptanceStarted { change_id: String, command: String },
    /// Acceptance completed for a change
    AcceptanceCompleted { change_id: String },
    /// Acceptance failed for a change
    #[allow(dead_code)]
    AcceptanceFailed { change_id: String, error: String },
    /// Change rejected after acceptance blocker detection
    ChangeRejected { change_id: String, reason: String },
    /// Rejection review completed for a change
    RejectionReviewCompleted {
        change_id: String,
        outcome: RejectionOutcome,
    },
    /// Rejection review failed for a change
    RejectionReviewFailed { change_id: String, error: String },
    /// Acceptance output (streaming)
    #[allow(dead_code)]
    AcceptanceOutput {
        change_id: String,
        output: String,
        iteration: Option<u32>,
    },

    // Progress events
    /// Progress updated for a change (task completion tracking)
    ProgressUpdated {
        change_id: String,
        completed: u32,
        total: u32,
    },

    // Workspace events (parallel mode)
    /// A workspace was created
    #[allow(dead_code)]
    WorkspaceCreated {
        change_id: String,
        workspace: String,
    },
    /// Workspace status synchronization for a specific change (parallel mode)
    WorkspaceStatusUpdated {
        change_id: String,
        #[allow(dead_code)]
        workspace_name: String,
        #[allow(dead_code)]
        status: crate::vcs::WorkspaceStatus,
    },
    /// An existing workspace was found and is being reused
    #[allow(dead_code)]
    WorkspaceResumed {
        change_id: String,
        workspace: String,
    },
    /// A workspace was preserved due to an error (not cleaned up)
    #[allow(dead_code)]
    WorkspacePreserved {
        change_id: String,
        workspace_name: String,
    },
    /// Workspace cleanup started
    #[allow(dead_code)]
    CleanupStarted { workspace: String },
    /// Workspace cleanup completed
    CleanupCompleted {
        #[allow(dead_code)]
        workspace: String,
    },

    // Merge events (parallel mode)
    /// Merge started
    #[allow(dead_code)]
    MergeStarted { revisions: Vec<String> },
    /// Merge completed
    MergeCompleted {
        change_id: String,
        #[allow(dead_code)]
        revision: String,
    },
    /// Push started for a completed change branch.
    PushStarted {
        change_id: String,
        #[allow(dead_code)]
        remote: String,
        #[allow(dead_code)]
        branch: String,
    },
    /// Push completed for a completed change branch.
    PushCompleted {
        change_id: String,
        #[allow(dead_code)]
        remote: String,
        #[allow(dead_code)]
        branch: String,
    },
    /// Push failed for a completed change branch.
    PushFailed {
        change_id: String,
        #[allow(dead_code)]
        remote: String,
        #[allow(dead_code)]
        branch: String,
        error: String,
    },
    /// Merge deferred due to dirty base or incomplete archive.
    /// `auto_resumable` is `true` when the deferral is caused by a temporary condition
    /// (base dirty, merge in progress) that will resolve automatically once a preceding
    /// merge or resolve completes.  `false` means manual intervention is required.
    #[allow(dead_code)]
    MergeDeferred {
        change_id: String,
        reason: String,
        auto_resumable: bool,
    },
    /// Merge resolution started for a change
    ResolveStarted { change_id: String, command: String },
    /// Merge resolution completed for a change
    ResolveCompleted {
        change_id: String,
        worktree_change_ids: Option<std::collections::HashSet<String>>,
    },
    /// Merge resolution failed for a change
    ResolveFailed { change_id: String, error: String },
    /// Merge resulted in conflicts
    #[allow(dead_code)]
    MergeConflict { files: Vec<String> },
    /// Conflict resolution started
    ConflictResolutionStarted,
    /// Conflict resolution completed
    ConflictResolutionCompleted,
    /// Conflict resolution failed
    #[allow(dead_code)]
    ConflictResolutionFailed { error: String },

    /// A change was skipped because a dependency failed
    #[allow(dead_code)]
    ChangeSkipped { change_id: String, reason: String },

    /// A change is blocked waiting for dependencies to be resolved
    DependencyBlocked {
        change_id: String,
        #[allow(dead_code)]
        dependency_ids: Vec<String>,
    },

    /// A change's dependencies were resolved and it can now be queued
    DependencyResolved { change_id: String },

    /// Acceptance observed a compatibility gated token that becomes a non-terminal stalled hold.
    AcceptanceGated {
        change_id: String,
        blocker: StalledBlocker,
    },
    /// Apply or acceptance observed a repeated unresolved permission/policy blocker.
    ExecutionBlocked {
        change_id: String,
        blocker: StalledBlocker,
    },

    // Analysis events (parallel mode)
    /// Analysis started for remaining changes.
    ///
    /// `attempt_id` is observability-only metadata used by UIs to distinguish
    /// separate scheduler analysis attempts that happen to have the same
    /// remaining count. It must not be used as workflow-control state.
    #[allow(dead_code)]
    AnalysisStarted {
        remaining_changes: usize,
        attempt_id: String,
    },
    /// Analysis output (streaming)
    #[allow(dead_code)]
    AnalysisOutput { output: String, iteration: u32 },
    /// Analysis completed
    #[allow(dead_code)]
    AnalysisCompleted { groups_found: usize },
    /// Resolve output (streaming)
    #[allow(dead_code)]
    ResolveOutput {
        change_id: String,
        output: String,
        iteration: Option<u32>,
    },

    // Hook events
    /// Hook execution started
    #[allow(dead_code)]
    HookStarted {
        change_id: String,
        hook_type: String,
    },
    /// Hook execution completed successfully
    #[allow(dead_code)]
    HookCompleted {
        change_id: String,
        hook_type: String,
    },
    /// Hook execution failed
    #[allow(dead_code)]
    HookFailed {
        change_id: String,
        hook_type: String,
        error: String,
    },

    // General events
    /// Warning message (non-fatal)
    Warning { title: String, message: String },
    /// Changes rejected at parallel start-time eligibility filter
    ///
    /// Sent when backend filtering excludes one or more changes before parallel execution
    /// starts. Callers should use this to restore a consistent non-running state for the
    /// rejected changes (e.g. reset Queued rows in TUI, report zero-start in CLI).
    ParallelStartRejected {
        change_ids: Vec<String>,
        reason: String,
    },
    /// Log message
    Log(LogEntry),
    /// Processing stopping (graceful stop initiated)
    Stopping,
    /// Processing stopped (graceful stop completed)
    Stopped,
    /// All processing completed
    AllCompleted,
    /// Error during execution
    Error { message: String },
    /// Changes list refreshed
    ChangesRefreshed {
        changes: Vec<crate::openspec::Change>,
        /// Rejected marker rows derived from the same refresh source as `changes`.
        rejected_changes: Vec<crate::openspec::Change>,
        committed_change_ids: std::collections::HashSet<String>,
        /// Set of change_ids with uncommitted or untracked files under openspec/changes/<change_id>/
        uncommitted_file_change_ids: std::collections::HashSet<String>,
        worktree_change_ids: std::collections::HashSet<String>,
        /// Map of change_id to worktree path for active worktrees
        worktree_paths: std::collections::HashMap<String, std::path::PathBuf>,
        /// Set of change_ids whose worktrees are NOT ahead of base (for auto-clearing MergeWait)
        worktree_not_ahead_ids: std::collections::HashSet<String>,
        /// Set of change_ids in WorkspaceState::Archived (for MergeWait restoration)
        merge_wait_ids: std::collections::HashSet<String>,
    },
    /// Worktrees list refreshed (for worktree view)
    WorktreesRefreshed {
        worktrees: Vec<crate::tui::types::WorktreeInfo>,
    },
    /// Branch merge started (TUI worktree view)
    BranchMergeStarted { branch_name: String },
    /// Branch merge completed successfully (TUI worktree view)
    BranchMergeCompleted { branch_name: String },
    /// Branch merge failed (TUI worktree view)
    BranchMergeFailed { branch_name: String, error: String },
    /// Change force-stopped and dequeued successfully (single-change stop-and-dequeue)
    ChangeDequeued { change_id: String },
    /// Legacy single-change stop event (kept for compatibility)
    #[allow(dead_code)]
    ChangeStopped { change_id: String },
    /// Change stop failed (single-change stop)
    #[allow(dead_code)]
    ChangeStopFailed { change_id: String, error: String },
    /// Incremental update from a remote server WebSocket (applies non-regression rule)
    RemoteChangeUpdate {
        /// Change ID as displayed in TUI (may be "project/change-id" for remote mode)
        id: String,
        /// Updated number of completed tasks
        completed_tasks: u32,
        /// Updated total number of tasks
        total_tasks: u32,
        /// Updated remote status (optional)
        status: Option<String>,
        /// Iteration number (applies monotonic non-regression rule)
        iteration_number: Option<u32>,
    },
}

/// Frontend-agnostic sink for execution events and state transitions.
#[async_trait]
pub trait EventSink: Send + Sync {
    /// Handle an execution event emitted by orchestration logic.
    async fn on_event(&self, event: &ExecutionEvent);

    /// Handle reducer state transition notifications.
    async fn on_state_changed(&self, state: &OrchestratorState);
}

/// No-op sink used for state-only update notifications.
pub struct NoopEventSink;

#[async_trait]
impl EventSink for NoopEventSink {
    async fn on_event(&self, _event: &ExecutionEvent) {}

    async fn on_state_changed(&self, _state: &OrchestratorState) {}
}

/// Helper to send events through the channel.
///
/// Logs debug message if sending fails (channel closed).
pub async fn send_event(tx: &Option<mpsc::Sender<ExecutionEvent>>, event: ExecutionEvent) {
    if let Some(ref tx) = tx {
        if let Err(e) = tx.send(event).await {
            debug!("Failed to send execution event: {}", e);
        }
    }
}

/// Dispatches an event to reducer and all frontend sinks.
pub async fn dispatch_event(
    state: &tokio::sync::RwLock<OrchestratorState>,
    sinks: &[std::sync::Arc<dyn EventSink>],
    event: ExecutionEvent,
) {
    let state_snapshot = {
        let mut guard = state.write().await;
        guard.apply_execution_event(&event);
        guard.clone()
    };

    for sink in sinks {
        sink.on_event(&event).await;
    }

    for sink in sinks {
        sink.on_state_changed(&state_snapshot).await;
    }
}

/// Build sink list for CLI mode (no frontend sink, reducer update only).
pub fn cli_event_sinks() -> Vec<std::sync::Arc<dyn EventSink>> {
    vec![std::sync::Arc::new(NoopEventSink)]
}

// ── External lifecycle bridge ──────────────────────────────────────────────

/// Map an orchestration event to a semantic external-lifecycle event.
///
/// This is a one-way observability projection. It never changes `EventSink`
/// frontend ownership and never participates in workflow-control decisions:
/// events that carry no semantic lifecycle meaning simply map to `None`.
pub fn lifecycle_event_for_execution_event(
    event: &ExecutionEvent,
    workspace: Option<&str>,
) -> Option<crate::lifecycle_integration::LifecycleEvent> {
    use crate::lifecycle_integration::{LifecycleContext, LifecycleEvent, LifecycleState};

    let context = |change_id: Option<&str>| LifecycleContext {
        workspace: workspace.map(str::to_owned),
        change_id: change_id.map(str::to_owned),
        ..Default::default()
    };

    let (state, change_id) = match event {
        // Work is executing.
        ExecutionEvent::ProcessingStarted(id) => (LifecycleState::Working, Some(id.as_str())),
        ExecutionEvent::ApplyStarted { change_id, .. }
        | ExecutionEvent::ArchiveStarted { change_id, .. }
        | ExecutionEvent::AcceptanceStarted { change_id, .. }
        | ExecutionEvent::ResolveStarted { change_id, .. } => {
            (LifecycleState::Working, Some(change_id.as_str()))
        }
        ExecutionEvent::ConflictResolutionStarted => (LifecycleState::Working, None),
        // Graceful stop is still active work until it completes.
        ExecutionEvent::Stopping => (LifecycleState::Working, None),

        // A human decision is required before work can continue.
        ExecutionEvent::AcceptanceGated { change_id, .. }
        | ExecutionEvent::ExecutionBlocked { change_id, .. }
        | ExecutionEvent::DependencyBlocked { change_id, .. } => {
            (LifecycleState::Blocked, Some(change_id.as_str()))
        }
        ExecutionEvent::ProcessingError { id, .. } => (LifecycleState::Blocked, Some(id.as_str())),
        ExecutionEvent::Error { .. } => (LifecycleState::Blocked, None),

        // Nothing is executing anymore.
        ExecutionEvent::AllCompleted | ExecutionEvent::Stopped => (LifecycleState::Idle, None),

        // Every other event is progress detail without semantic lifecycle meaning.
        _ => return None,
    };

    Some(LifecycleEvent::StateChanged {
        state,
        context: context(change_id),
    })
}

/// `EventSink` adapter that projects orchestration events onto an external
/// lifecycle adapter.
///
/// Publishing is non-blocking and failure-isolated, so adding this sink cannot
/// change orchestration timing or routing.
pub struct LifecycleEventSink {
    handle: crate::lifecycle_integration::LifecycleHandle,
    workspace: Option<String>,
}

impl LifecycleEventSink {
    /// Create a sink that forwards mapped events to `handle`.
    pub fn new(
        handle: crate::lifecycle_integration::LifecycleHandle,
        workspace: Option<String>,
    ) -> Self {
        Self { handle, workspace }
    }
}

#[async_trait]
impl EventSink for LifecycleEventSink {
    async fn on_event(&self, event: &ExecutionEvent) {
        if let Some(lifecycle_event) =
            lifecycle_event_for_execution_event(event, self.workspace.as_deref())
        {
            self.handle.publish(lifecycle_event);
        }
    }

    async fn on_state_changed(&self, _state: &OrchestratorState) {}
}

/// Sink used by tests to collect emitted events.
#[derive(Default)]
#[allow(dead_code)]
pub struct MockEventSink {
    events: tokio::sync::Mutex<Vec<ExecutionEvent>>,
}

#[allow(dead_code)]
impl MockEventSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn events(&self) -> Vec<ExecutionEvent> {
        self.events.lock().await.clone()
    }
}

#[async_trait]
impl EventSink for MockEventSink {
    async fn on_event(&self, event: &ExecutionEvent) {
        self.events.lock().await.push(event.clone());
    }

    async fn on_state_changed(&self, _state: &OrchestratorState) {}
}

#[cfg(test)]
mod lifecycle_bridge_tests {
    use super::*;
    use crate::lifecycle_integration::{
        LifecycleEvent, LifecycleHandle, LifecyclePublisher, LifecycleState,
    };
    use std::sync::{Arc, Mutex};

    /// In-memory lifecycle dispatcher double.
    ///
    /// Keeps this suite unit-scoped: no adapter process, filesystem, or other
    /// stateful external boundary is touched.
    #[derive(Default)]
    struct MockLifecycleDispatcher {
        published: Mutex<Vec<LifecycleEvent>>,
    }

    impl MockLifecycleDispatcher {
        fn published(&self) -> Vec<LifecycleEvent> {
            self.published.lock().expect("mock lock").clone()
        }

        fn states(&self) -> Vec<LifecycleState> {
            self.published()
                .into_iter()
                .filter_map(|event| match event {
                    LifecycleEvent::StateChanged { state, .. } => Some(state),
                    _ => None,
                })
                .collect()
        }
    }

    impl LifecyclePublisher for MockLifecycleDispatcher {
        fn publish(&self, event: LifecycleEvent) {
            self.published.lock().expect("mock lock").push(event);
        }
    }

    fn mock_sink() -> (Arc<MockLifecycleDispatcher>, LifecycleEventSink) {
        let dispatcher = Arc::new(MockLifecycleDispatcher::default());
        let handle =
            LifecycleHandle::from_publisher(dispatcher.clone() as Arc<dyn LifecyclePublisher>);
        (
            dispatcher,
            LifecycleEventSink::new(handle, Some("/repo".to_string())),
        )
    }

    fn state_of(event: &ExecutionEvent) -> Option<LifecycleState> {
        match lifecycle_event_for_execution_event(event, Some("/repo")) {
            Some(LifecycleEvent::StateChanged { state, .. }) => Some(state),
            _ => None,
        }
    }

    #[test]
    fn active_execution_events_map_to_working() {
        for event in [
            ExecutionEvent::ProcessingStarted("change-a".to_string()),
            ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            },
            ExecutionEvent::ArchiveStarted {
                change_id: "change-a".to_string(),
                command: "archive".to_string(),
            },
            ExecutionEvent::AcceptanceStarted {
                change_id: "change-a".to_string(),
                command: "accept".to_string(),
            },
            ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "resolve".to_string(),
            },
            ExecutionEvent::Stopping,
        ] {
            assert_eq!(
                state_of(&event),
                Some(LifecycleState::Working),
                "event should report working: {event:?}"
            );
        }
    }

    #[test]
    fn user_decision_events_map_to_blocked() {
        let blocker = StalledBlocker::acceptance_external(
            "pending_verification",
            "verification job still running",
        );

        for event in [
            ExecutionEvent::AcceptanceGated {
                change_id: "change-a".to_string(),
                blocker: blocker.clone(),
            },
            ExecutionEvent::ExecutionBlocked {
                change_id: "change-a".to_string(),
                blocker,
            },
            ExecutionEvent::DependencyBlocked {
                change_id: "change-a".to_string(),
                dependency_ids: vec!["dep".to_string()],
            },
            ExecutionEvent::ProcessingError {
                id: "change-a".to_string(),
                error: "boom".to_string(),
            },
            ExecutionEvent::Error {
                message: "boom".to_string(),
            },
        ] {
            assert_eq!(
                state_of(&event),
                Some(LifecycleState::Blocked),
                "event should report blocked: {event:?}"
            );
        }
    }

    #[test]
    fn terminal_events_map_to_idle() {
        for event in [ExecutionEvent::AllCompleted, ExecutionEvent::Stopped] {
            assert_eq!(
                state_of(&event),
                Some(LifecycleState::Idle),
                "event should report idle: {event:?}"
            );
        }
    }

    #[test]
    fn progress_detail_events_are_not_projected() {
        for event in [
            ExecutionEvent::Log(LogEntry::info("hello")),
            ExecutionEvent::ApplyOutput {
                change_id: "change-a".to_string(),
                output: "chunk".to_string(),
                iteration: Some(1),
            },
            ExecutionEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 1,
                total: 2,
            },
            ExecutionEvent::WorktreesRefreshed { worktrees: vec![] },
        ] {
            assert!(
                lifecycle_event_for_execution_event(&event, Some("/repo")).is_none(),
                "presentation-only event must not be projected: {event:?}"
            );
        }
    }

    #[test]
    fn projected_context_is_limited_to_workspace_and_change_id() {
        let projected = lifecycle_event_for_execution_event(
            &ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "secret-agent --token abcdef".to_string(),
            },
            Some("/repo"),
        )
        .expect("apply start should project");

        match projected {
            LifecycleEvent::StateChanged { context, .. } => {
                assert_eq!(context.workspace.as_deref(), Some("/repo"));
                assert_eq!(context.change_id.as_deref(), Some("change-a"));
                assert_eq!(context.session_id, None);
            }
            other => panic!("unexpected projection: {other:?}"),
        }
    }

    #[tokio::test]
    async fn lifecycle_sink_receives_orchestration_events_through_dispatch() {
        let (dispatcher, lifecycle_sink) = mock_sink();
        let state = tokio::sync::RwLock::new(crate::orchestration::state::OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        ));
        let frontend_sink = std::sync::Arc::new(MockEventSink::new());
        let sinks: Vec<std::sync::Arc<dyn EventSink>> =
            vec![frontend_sink.clone(), std::sync::Arc::new(lifecycle_sink)];

        for event in [
            ExecutionEvent::ProcessingStarted("change-a".to_string()),
            ExecutionEvent::Log(LogEntry::info("noise")),
            ExecutionEvent::AllCompleted,
        ] {
            dispatch_event(&state, &sinks, event).await;
        }

        assert_eq!(
            dispatcher.states(),
            vec![LifecycleState::Working, LifecycleState::Idle],
            "lifecycle sink must observe semantic transitions only"
        );

        assert_eq!(
            frontend_sink.events().await.len(),
            3,
            "existing frontend sink ownership must be unchanged"
        );
    }

    #[tokio::test]
    async fn lifecycle_sink_does_not_alter_reducer_state() {
        let (_dispatcher, lifecycle_sink) = mock_sink();
        let with_lifecycle = tokio::sync::RwLock::new(
            crate::orchestration::state::OrchestratorState::new(vec!["change-a".to_string()], 10),
        );
        let without_lifecycle = tokio::sync::RwLock::new(
            crate::orchestration::state::OrchestratorState::new(vec!["change-a".to_string()], 10),
        );

        let with_sinks: Vec<std::sync::Arc<dyn EventSink>> =
            vec![std::sync::Arc::new(lifecycle_sink)];
        let without_sinks: Vec<std::sync::Arc<dyn EventSink>> = cli_event_sinks();

        dispatch_event(
            &with_lifecycle,
            &with_sinks,
            ExecutionEvent::ProcessingStarted("change-a".to_string()),
        )
        .await;
        dispatch_event(
            &without_lifecycle,
            &without_sinks,
            ExecutionEvent::ProcessingStarted("change-a".to_string()),
        )
        .await;

        assert_eq!(
            with_lifecycle.read().await.all_display_statuses(),
            without_lifecycle.read().await.all_display_statuses(),
            "attaching a lifecycle sink must not change core state transitions"
        );
    }

    #[tokio::test]
    async fn disabled_lifecycle_handle_drops_projected_events() {
        let sink = LifecycleEventSink::new(LifecycleHandle::disabled(), Some("/repo".to_string()));

        // Must not panic and must remain a no-op.
        sink.on_event(&ExecutionEvent::ProcessingStarted("change-a".to_string()))
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_event_debug() {
        let event = ExecutionEvent::WorkspaceCreated {
            change_id: "test".to_string(),
            workspace: "ws-test".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("WorkspaceCreated"));
    }

    #[tokio::test]
    async fn test_dispatch_event_notifies_mock_sink() {
        let state = tokio::sync::RwLock::new(crate::orchestration::state::OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        ));
        let mock_sink = std::sync::Arc::new(MockEventSink::new());
        let sinks: Vec<std::sync::Arc<dyn EventSink>> = vec![mock_sink.clone()];

        dispatch_event(
            &state,
            &sinks,
            ExecutionEvent::ProcessingStarted("change-a".to_string()),
        )
        .await;

        let captured = mock_sink.events().await;
        assert_eq!(captured.len(), 1);
        assert!(matches!(
            captured.first(),
            Some(ExecutionEvent::ProcessingStarted(id)) if id == "change-a"
        ));
    }

    #[test]
    fn test_log_entry_info() {
        let entry = LogEntry::info("test message");
        assert_eq!(entry.message, "test message");
        assert!(matches!(entry.color, Color::White));
        assert!(entry.change_id.is_none());
    }

    #[test]
    fn test_log_entry_strips_ansi_sequences() {
        let entry = LogEntry::info("\x1b[96mRead\x1b[0m");
        assert_eq!(entry.message, "Read");
    }

    #[test]
    fn test_log_entry_strips_sgr_fragments() {
        let entry = LogEntry::info("[96m[1m| [0m[90m Read");
        assert_eq!(entry.message, "|  Read");
    }

    #[test]
    fn log_entry_replaces_line_breaks_instead_of_joining_words() {
        let entry = LogEntry::info("first\nsecond\tthird\rfourth");
        assert_eq!(entry.message, "first\\nsecond\\tthird\\rfourth");
    }

    #[test]
    fn log_entry_bounds_large_messages() {
        let entry = LogEntry::info("x".repeat(1_000_000));

        assert!(entry.message.len() <= 8_192);
        let marker_start = entry.message.find("…[truncated ").unwrap();
        let omitted = entry.message[marker_start + "…[truncated ".len()..]
            .trim_end_matches(" bytes]")
            .parse::<usize>()
            .unwrap();
        assert_eq!(omitted, 1_000_000 - marker_start);
    }

    #[test]
    fn log_entry_bounds_large_utf8_messages() {
        let entry = LogEntry::info("日".repeat(3_000));

        assert!(entry.message.len() <= 8_192);
        assert!(entry.message.contains("[truncated "));
    }

    #[test]
    fn test_log_entry_with_change_id() {
        let entry = LogEntry::info("test").with_change_id("test-change");
        assert_eq!(entry.change_id, Some("test-change".to_string()));
    }

    #[test]
    fn test_hook_started_event() {
        let event = ExecutionEvent::HookStarted {
            change_id: "test-change".to_string(),
            hook_type: "pre_apply".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("pre_apply"));
    }

    #[test]
    fn test_hook_completed_event() {
        let event = ExecutionEvent::HookCompleted {
            change_id: "test-change".to_string(),
            hook_type: "post_apply".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookCompleted"));
        assert!(debug_str.contains("post_apply"));
    }

    #[test]
    fn test_hook_failed_event() {
        let event = ExecutionEvent::HookFailed {
            change_id: "test-change".to_string(),
            hook_type: "pre_archive".to_string(),
            error: "Hook timed out".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookFailed"));
        assert!(debug_str.contains("pre_archive"));
        assert!(debug_str.contains("Hook timed out"));
    }

    #[test]
    fn test_progress_updated_event() {
        let event = ExecutionEvent::ProgressUpdated {
            change_id: "test-change".to_string(),
            completed: 5,
            total: 10,
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ProgressUpdated"));
        assert!(debug_str.contains("test-change"));
    }

    #[test]
    fn test_log_entry_with_operation() {
        let entry = LogEntry::info("test").with_operation("apply");
        assert_eq!(entry.operation, Some("apply".to_string()));
    }

    #[test]
    fn test_log_entry_with_iteration() {
        let entry = LogEntry::info("test").with_iteration(2);
        assert_eq!(entry.iteration, Some(2));
    }

    #[test]
    fn test_log_entry_with_operation_and_iteration() {
        let entry = LogEntry::info("test")
            .with_change_id("test-change")
            .with_operation("apply")
            .with_iteration(3);
        assert_eq!(entry.change_id, Some("test-change".to_string()));
        assert_eq!(entry.operation, Some("apply".to_string()));
        assert_eq!(entry.iteration, Some(3));
    }

    #[test]
    fn test_log_entry_info_level() {
        let entry = LogEntry::info("test");
        assert_eq!(entry.level, LogLevel::Info);
        assert!(matches!(entry.color, Color::White));
    }

    #[test]
    fn test_log_entry_success_level() {
        let entry = LogEntry::success("test");
        assert_eq!(entry.level, LogLevel::Success);
        assert!(matches!(entry.color, Color::Green));
    }

    #[test]
    fn test_log_entry_warn_level() {
        let entry = LogEntry::warn("test");
        assert_eq!(entry.level, LogLevel::Warn);
        assert!(matches!(entry.color, Color::Yellow));
    }

    #[test]
    fn test_log_entry_error_level() {
        let entry = LogEntry::error("test");
        assert_eq!(entry.level, LogLevel::Error);
        assert!(matches!(entry.color, Color::Red));
    }

    #[test]
    fn test_log_level_equality() {
        assert_eq!(LogLevel::Info, LogLevel::Info);
        assert_ne!(LogLevel::Info, LogLevel::Error);
    }

    #[test]
    fn test_acceptance_started_event_with_command() {
        let event = ExecutionEvent::AcceptanceStarted {
            change_id: "test-change".to_string(),
            command: "claude --dangerously-skip-permissions acceptance test-change".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("AcceptanceStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("acceptance"));
    }

    #[test]
    fn test_archive_started_event_with_command() {
        let event = ExecutionEvent::ArchiveStarted {
            change_id: "test-change".to_string(),
            command: "claude --dangerously-skip-permissions archive test-change".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ArchiveStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("archive"));
    }

    #[test]
    fn test_resolve_started_event_with_command() {
        let event = ExecutionEvent::ResolveStarted {
            change_id: "test-change".to_string(),
            command: "claude --dangerously-skip-permissions resolve test-change".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ResolveStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("resolve"));
    }
}
