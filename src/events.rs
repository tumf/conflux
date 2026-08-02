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

/// Message marker identifying a recoverable dependency-analysis fallback diagnostic.
///
/// Producers emit this prefix when an LLM analysis response is rejected but
/// metadata-dependency-only analysis safely takes over, so scheduler execution
/// continues. It is operator-facing wording only: consumers MUST NOT classify
/// fatality from it. Non-fatality is carried by the warning-log event type the
/// producer uses, never by message content.
pub const RECOVERABLE_ANALYSIS_FALLBACK_MARKER: &str =
    "Dependency analysis degraded: falling back to metadata-dependency-only analysis";

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

/// Strip control characters and ANSI escapes from operator-facing detail and
/// bound its length.
///
/// Exposed because log entries are not the only place a raw tool string reaches
/// an operator: the `/api/v2` snapshot publishes blocker, error, and activity
/// detail, and those must be sanitized by exactly the same rule.
pub fn sanitize_detail(message: &str) -> String {
    sanitize_log_message(message)
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

/// Blocker facts an execution phase reports to the orchestrator.
///
/// This is *reported evidence*, not a lifecycle decision. The orchestrator runs
/// [`crate::orchestration::blocker_classification::classify_execution_hold`] over
/// these facts to decide between external `blocked` and execution `stalled`; a
/// phase can never assign canonical lifecycle status by populating this struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalledBlocker {
    pub category: String,
    pub phase: String,
    pub gate: String,
    pub error_summary: String,
    pub evidence: Vec<String>,
    /// Verifiable condition that clears an external prerequisite wait.
    ///
    /// `None` for execution holds (no-progress, repeated findings, exhausted
    /// retry, permission denial), which is exactly what keeps them out of the
    /// external `blocked` classification.
    pub unblock_condition: Option<String>,
    /// Owning team/role or named prerequisite, when the reporter supplied one.
    pub prerequisite_owner: Option<String>,
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
            // A repeated permission denial is an execution hold, not a named
            // external prerequisite: it deliberately supplies no unblock
            // condition, so the classifier keeps it on the `stalled` path.
            unblock_condition: None,
            prerequisite_owner: None,
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
            unblock_condition: Some(format!(
                "the external prerequisite behind '{error_summary}' is satisfied"
            )),
            prerequisite_owner: None,
            error_summary,
            next_action: "resolve the external verification blocker and rerun acceptance"
                .to_string(),
            resumable: true,
            worktree_preserved: true,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "category={}, phase={}, gate={}, evidence={}, unblock_condition={}, resumable={}, next_action={}, error={}",
            self.category,
            self.phase,
            self.gate,
            self.evidence.join(" | "),
            self.unblock_condition.as_deref().unwrap_or("none reported"),
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
}

/// Which projection owner is responsible for one internal execution event.
///
/// The classification is what keeps "one internal event, one reducer
/// transition, one ordered remote event" checkable: it is decided once, by
/// [`classify_event`], and every frontend reads it from there instead of
/// re-deriving its own opinion from the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOwnership {
    /// May change reducer state and the published operator snapshot.
    ///
    /// Dispatch clones the post-transition reducer state for these so a
    /// frontend projects from the authoritative output rather than from its own
    /// copy.
    State,
    /// Observational log output. Never advances a state revision.
    Log,
    /// Progress detail that cannot change the published operator snapshot.
    ///
    /// A presentation event still gets one ordered remote event, but no
    /// candidate snapshot is computed for it — it carries no field the snapshot
    /// is built from.
    Presentation,
}

/// The stable variant name and projection ownership of one execution event.
///
/// This match is deliberately exhaustive with no `_` arm: adding an
/// `ExecutionEvent` variant without classifying it fails to compile, which is
/// what makes the ownership table in the tests impossible to leave stale.
pub fn classify_event(event: &ExecutionEvent) -> (&'static str, EventOwnership) {
    use EventOwnership::{Log, Presentation, State};
    use ExecutionEvent as E;

    match event {
        E::ProcessingStarted(_) => ("ProcessingStarted", State),
        E::ProcessingCompleted(_) => ("ProcessingCompleted", State),
        E::ProcessingError { .. } => ("ProcessingError", State),
        E::ApplyStarted { .. } => ("ApplyStarted", State),
        E::ApplyCompleted { .. } => ("ApplyCompleted", State),
        E::ApplyFailed { .. } => ("ApplyFailed", State),
        E::ApplyOutput { .. } => ("ApplyOutput", State),
        E::ArchiveStarted { .. } => ("ArchiveStarted", State),
        E::ArchiveResumed { .. } => ("ArchiveResumed", State),
        E::ArchiveRetryScheduled { .. } => ("ArchiveRetryScheduled", State),
        E::ChangeArchived(_) => ("ChangeArchived", State),
        E::ArchiveFailed { .. } => ("ArchiveFailed", State),
        E::ArchiveOutput { .. } => ("ArchiveOutput", State),
        E::AcceptanceStarted { .. } => ("AcceptanceStarted", State),
        E::AcceptanceCompleted { .. } => ("AcceptanceCompleted", State),
        E::AcceptanceFailed { .. } => ("AcceptanceFailed", State),
        E::ChangeRejected { .. } => ("ChangeRejected", State),
        E::RejectionReviewCompleted { .. } => ("RejectionReviewCompleted", State),
        E::RejectionReviewFailed { .. } => ("RejectionReviewFailed", State),
        E::AcceptanceOutput { .. } => ("AcceptanceOutput", State),
        E::ProgressUpdated { .. } => ("ProgressUpdated", State),
        E::WorkspaceCreated { .. } => ("WorkspaceCreated", State),
        E::WorkspaceStatusUpdated { .. } => ("WorkspaceStatusUpdated", State),
        E::WorkspaceResumed { .. } => ("WorkspaceResumed", State),
        E::WorkspacePreserved { .. } => ("WorkspacePreserved", State),
        E::MergeCompleted { .. } => ("MergeCompleted", State),
        E::PushStarted { .. } => ("PushStarted", State),
        E::PushCompleted { .. } => ("PushCompleted", State),
        E::PushFailed { .. } => ("PushFailed", State),
        E::MergeDeferred { .. } => ("MergeDeferred", State),
        E::ResolveStarted { .. } => ("ResolveStarted", State),
        E::ResolveCompleted { .. } => ("ResolveCompleted", State),
        E::ResolveFailed { .. } => ("ResolveFailed", State),
        E::ResolveOutput { .. } => ("ResolveOutput", State),
        E::ChangeSkipped { .. } => ("ChangeSkipped", State),
        E::DependencyBlocked { .. } => ("DependencyBlocked", State),
        E::DependencyResolved { .. } => ("DependencyResolved", State),
        E::AcceptanceGated { .. } => ("AcceptanceGated", State),
        E::ExecutionBlocked { .. } => ("ExecutionBlocked", State),
        E::HookStarted { .. } => ("HookStarted", State),
        E::HookCompleted { .. } => ("HookCompleted", State),
        E::HookFailed { .. } => ("HookFailed", State),
        E::Stopping => ("Stopping", State),
        E::Stopped => ("Stopped", State),
        E::AllCompleted => ("AllCompleted", State),
        E::Error { .. } => ("Error", State),
        E::ChangesRefreshed { .. } => ("ChangesRefreshed", State),
        E::WorktreesRefreshed { .. } => ("WorktreesRefreshed", State),
        E::ChangeDequeued { .. } => ("ChangeDequeued", State),
        E::ChangeStopped { .. } => ("ChangeStopped", State),
        E::ChangeStopFailed { .. } => ("ChangeStopFailed", State),

        E::Log(_) => ("Log", Log),

        // Presentation-only exceptions. Each carries no change-addressed or
        // process-level field the operator snapshot is built from, so it can be
        // ordered on the remote stream without recomputing a candidate.
        E::CleanupStarted { .. } => ("CleanupStarted", Presentation),
        E::CleanupCompleted { .. } => ("CleanupCompleted", Presentation),
        E::MergeStarted { .. } => ("MergeStarted", Presentation),
        E::MergeConflict { .. } => ("MergeConflict", Presentation),
        E::ConflictResolutionStarted => ("ConflictResolutionStarted", Presentation),
        E::ConflictResolutionCompleted => ("ConflictResolutionCompleted", Presentation),
        E::ConflictResolutionFailed { .. } => ("ConflictResolutionFailed", Presentation),
        E::AnalysisStarted { .. } => ("AnalysisStarted", Presentation),
        E::AnalysisOutput { .. } => ("AnalysisOutput", Presentation),
        E::AnalysisCompleted { .. } => ("AnalysisCompleted", Presentation),
        E::Warning { .. } => ("Warning", Presentation),
        E::ParallelStartRejected { .. } => ("ParallelStartRejected", Presentation),
        E::BranchMergeStarted { .. } => ("BranchMergeStarted", Presentation),
        E::BranchMergeCompleted { .. } => ("BranchMergeCompleted", Presentation),
        E::BranchMergeFailed { .. } => ("BranchMergeFailed", Presentation),
    }
}

/// The stable variant name of one execution event.
pub fn event_variant_name(event: &ExecutionEvent) -> &'static str {
    classify_event(event).0
}

/// Projection ownership of one execution event.
pub fn event_ownership(event: &ExecutionEvent) -> EventOwnership {
    classify_event(event).1
}

/// Terminal app modes a late or duplicate `AllCompleted` must not overwrite.
pub const RETAINED_TERMINAL_MODES: [&str; 2] = ["error", "stopped"];

/// Whether `AllCompleted` may overwrite `current_mode`.
///
/// `Error` and `Stopped` are authoritative terminal modes: the scheduler's own
/// completion event routinely arrives after an operator stop or a fatal error,
/// and a frontend that let it win would report "all done" for a run that
/// actually failed. TUI and the `/api/v2` projection both route the decision
/// through here so they cannot disagree about the same run.
pub fn all_completed_may_overwrite_mode(current_mode: &str) -> bool {
    !RETAINED_TERMINAL_MODES.contains(&current_mode)
}

/// One authoritative dispatch of an execution event.
///
/// Produced by [`dispatch_event`] after the reducer transition has already
/// happened, so a sink receives the event and the state it produced together
/// rather than reading the reducer back out on its own schedule.
pub struct EventDispatch<'a> {
    /// Process-unique dispatch identity.
    ///
    /// A frontend uses it to make a repeated delivery of the same dispatch a
    /// no-op; it is not a wire sequence and never leaves the process.
    pub id: u64,
    /// The event being dispatched.
    pub event: &'a ExecutionEvent,
    /// Projection ownership decided by [`classify_event`].
    pub ownership: EventOwnership,
    /// Authoritative reducer state after the transition.
    ///
    /// `None` for [`EventOwnership::Log`] and [`EventOwnership::Presentation`],
    /// whose events cannot change it.
    pub state: Option<&'a OrchestratorState>,
}

/// Process-local dispatch counter backing [`EventDispatch::id`].
static DISPATCH_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Allocate the next process-unique dispatch identity.
pub fn next_dispatch_id() -> u64 {
    DISPATCH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

/// Frontend-agnostic sink for execution events and state transitions.
#[async_trait]
pub trait EventSink: Send + Sync {
    /// Handle an execution event emitted by orchestration logic.
    async fn on_event(&self, event: &ExecutionEvent);

    /// Handle reducer state transition notifications.
    async fn on_state_changed(&self, state: &OrchestratorState);

    /// Handle one authoritative dispatch.
    ///
    /// Override this when a frontend needs the event and the state it produced
    /// in the same transaction; the default keeps the two-callback shape.
    async fn on_dispatch(&self, dispatch: &EventDispatch<'_>) {
        self.on_event(dispatch.event).await;
        if let Some(state) = dispatch.state {
            self.on_state_changed(state).await;
        }
    }
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

/// The single authoritative dispatch owner.
///
/// Applies the event to reducer state exactly once, then fans the resulting
/// event/state output to every frontend sink. Frontends never reapply the
/// event to the reducer, which is what keeps one internal event to one
/// transition no matter how many frontends are attached.
///
/// The reducer write lock is released before any sink runs, so a sink is free
/// to read shared state without deadlocking against this dispatch.
pub async fn dispatch_event(
    state: &tokio::sync::RwLock<OrchestratorState>,
    sinks: &[std::sync::Arc<dyn EventSink>],
    event: ExecutionEvent,
) {
    let ownership = event_ownership(&event);
    let id = next_dispatch_id();

    let state_snapshot = {
        let mut guard = state.write().await;
        guard.apply_execution_event(&event);
        // Only a state-owning event can change what a frontend publishes;
        // cloning the reducer for every log line would be pure waste.
        matches!(ownership, EventOwnership::State).then(|| guard.clone())
    };

    let dispatch = EventDispatch {
        id,
        event: &event,
        ownership,
        state: state_snapshot.as_ref(),
    };

    for sink in sinks {
        sink.on_dispatch(&dispatch).await;
    }
}

/// The dispatch owner for one orchestration boundary run.
///
/// Bundles the reducer state and the frontend sinks so every producer in a run
/// emits through the same path, including producers that can only speak
/// `mpsc::Sender` (see [`EventDispatcher::bridge`]).
pub struct EventDispatcher {
    state: std::sync::Arc<tokio::sync::RwLock<OrchestratorState>>,
    sinks: Vec<std::sync::Arc<dyn EventSink>>,
}

impl EventDispatcher {
    /// Own `state` and fan out to `sinks`.
    pub fn new(
        state: std::sync::Arc<tokio::sync::RwLock<OrchestratorState>>,
        sinks: Vec<std::sync::Arc<dyn EventSink>>,
    ) -> Self {
        Self { state, sinks }
    }

    /// Apply one event and fan it out.
    pub async fn dispatch(&self, event: ExecutionEvent) {
        dispatch_event(&self.state, &self.sinks, event).await;
    }

    /// A channel whose receiver forwards into this dispatch owner.
    ///
    /// Hook runners, output handlers, and the parallel scheduler can only emit
    /// through an `mpsc::Sender`. Handing them a bridge instead of a raw
    /// frontend channel is what stops serial-mode logs from reaching the TUI
    /// while never reaching the remote projection.
    ///
    /// The forwarding task ends when every sender is dropped.
    pub fn bridge(
        self: &std::sync::Arc<Self>,
        buffer: usize,
    ) -> (mpsc::Sender<ExecutionEvent>, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel(buffer);
        let owner = self.clone();
        let handle = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                owner.dispatch(event).await;
            }
        });
        (tx, handle)
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

/// One sample of every `ExecutionEvent` variant, and the ownership contract
/// they are held to.
///
/// Lives in the crate (behind `cfg(test)`) rather than in one test file because
/// the projection tests hold the *same* table to the *same* variants: a variant
/// that is state-owning here must not become a presentation event at the remote
/// boundary.
#[cfg(test)]
pub(crate) mod ownership_fixtures {
    use super::*;

    fn change(id: &str) -> crate::openspec::Change {
        crate::openspec::Change {
            id: id.to_string(),
            completed_tasks: 1,
            total_tasks: 2,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: crate::openspec::ProposalMetadata::default(),
        }
    }

    fn worktree() -> crate::tui::types::WorktreeInfo {
        crate::tui::types::WorktreeInfo {
            path: std::path::PathBuf::from("/tmp/ws"),
            head: "abc1234".to_string(),
            branch: "change-a".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        }
    }

    pub(crate) fn blocker() -> StalledBlocker {
        StalledBlocker {
            category: "external_service".to_string(),
            phase: "acceptance".to_string(),
            gate: "acceptance".to_string(),
            error_summary: "registry returned 503".to_string(),
            evidence: vec!["curl: 503".to_string()],
            unblock_condition: Some("the registry answers 200".to_string()),
            prerequisite_owner: Some("platform".to_string()),
            next_action: "retry acceptance".to_string(),
            resumable: true,
            worktree_preserved: true,
        }
    }

    /// Every `ExecutionEvent` variant, once.
    ///
    /// The list is checked against [`classify_event`] rather than trusted: a
    /// missing or duplicated entry fails
    /// `ownership_table_names_every_variant_exactly_once`.
    pub(crate) fn all_execution_events() -> Vec<ExecutionEvent> {
        use ExecutionEvent as E;
        vec![
            E::ProcessingStarted("change-a".to_string()),
            E::ProcessingCompleted("change-a".to_string()),
            E::ProcessingError {
                id: "change-a".to_string(),
                error: "boom".to_string(),
            },
            E::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply --token secret".to_string(),
            },
            E::ApplyCompleted {
                change_id: "change-a".to_string(),
                revision: "rev-1".to_string(),
            },
            E::ApplyFailed {
                change_id: "change-a".to_string(),
                error: "apply boom".to_string(),
            },
            E::ApplyOutput {
                change_id: "change-a".to_string(),
                output: "chunk".to_string(),
                iteration: Some(2),
            },
            E::ArchiveStarted {
                change_id: "change-a".to_string(),
                command: "archive".to_string(),
            },
            E::ArchiveResumed {
                change_id: "change-a".to_string(),
                reason: Some("resume reason".to_string()),
                summary: Some("resume summary".to_string()),
            },
            E::ArchiveRetryScheduled {
                change_id: "change-a".to_string(),
                attempt: 1,
                max_attempts: 3,
                reason: Some("retry reason".to_string()),
                summary: Some("retry summary".to_string()),
            },
            E::ChangeArchived("change-a".to_string()),
            E::ArchiveFailed {
                change_id: "change-a".to_string(),
                error: "archive boom".to_string(),
                reason: Some("archive reason".to_string()),
                summary: Some("archive summary".to_string()),
            },
            E::ArchiveOutput {
                change_id: "change-a".to_string(),
                output: "chunk".to_string(),
                iteration: 3,
            },
            E::AcceptanceStarted {
                change_id: "change-a".to_string(),
                command: "accept".to_string(),
            },
            E::AcceptanceCompleted {
                change_id: "change-a".to_string(),
            },
            E::AcceptanceFailed {
                change_id: "change-a".to_string(),
                error: "acceptance boom".to_string(),
            },
            E::ChangeRejected {
                change_id: "change-a".to_string(),
                reason: "rejected reason".to_string(),
            },
            E::RejectionReviewCompleted {
                change_id: "change-a".to_string(),
                outcome: RejectionOutcome::Confirm,
            },
            E::RejectionReviewFailed {
                change_id: "change-a".to_string(),
                error: "review boom".to_string(),
            },
            E::AcceptanceOutput {
                change_id: "change-a".to_string(),
                output: "chunk".to_string(),
                iteration: Some(4),
            },
            E::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 3,
                total: 7,
            },
            E::WorkspaceCreated {
                change_id: "change-a".to_string(),
                workspace: "ws-a".to_string(),
            },
            E::WorkspaceStatusUpdated {
                change_id: "change-a".to_string(),
                workspace_name: "ws-a".to_string(),
                status: crate::vcs::WorkspaceStatus::Applying,
            },
            E::WorkspaceResumed {
                change_id: "change-a".to_string(),
                workspace: "ws-a".to_string(),
            },
            E::WorkspacePreserved {
                change_id: "change-a".to_string(),
                workspace_name: "ws-a".to_string(),
            },
            E::CleanupStarted {
                workspace: "ws-a".to_string(),
            },
            E::CleanupCompleted {
                workspace: "ws-a".to_string(),
            },
            E::MergeStarted {
                revisions: vec!["rev-1".to_string()],
            },
            E::MergeCompleted {
                change_id: "change-a".to_string(),
                revision: "rev-2".to_string(),
            },
            E::PushStarted {
                change_id: "change-a".to_string(),
                remote: "origin".to_string(),
                branch: "change-a".to_string(),
            },
            E::PushCompleted {
                change_id: "change-a".to_string(),
                remote: "origin".to_string(),
                branch: "change-a".to_string(),
            },
            E::PushFailed {
                change_id: "change-a".to_string(),
                remote: "origin".to_string(),
                branch: "change-a".to_string(),
                error: "push boom".to_string(),
            },
            E::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "base dirty".to_string(),
                auto_resumable: true,
            },
            E::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "resolve".to_string(),
            },
            E::ResolveCompleted {
                change_id: "change-a".to_string(),
                worktree_change_ids: None,
            },
            E::ResolveFailed {
                change_id: "change-a".to_string(),
                error: "resolve boom".to_string(),
            },
            E::ResolveOutput {
                change_id: "change-a".to_string(),
                output: "chunk".to_string(),
                iteration: Some(5),
            },
            E::MergeConflict {
                files: vec!["src/lib.rs".to_string()],
            },
            E::ConflictResolutionStarted,
            E::ConflictResolutionCompleted,
            E::ConflictResolutionFailed {
                error: "conflict boom".to_string(),
            },
            E::ChangeSkipped {
                change_id: "change-a".to_string(),
                reason: "dependency failed".to_string(),
            },
            E::DependencyBlocked {
                change_id: "change-a".to_string(),
                dependency_ids: vec!["dep-a".to_string()],
            },
            E::DependencyResolved {
                change_id: "change-a".to_string(),
            },
            E::AcceptanceGated {
                change_id: "change-a".to_string(),
                blocker: blocker(),
            },
            E::ExecutionBlocked {
                change_id: "change-a".to_string(),
                blocker: blocker(),
            },
            E::AnalysisStarted {
                remaining_changes: 2,
                attempt_id: "attempt-1".to_string(),
            },
            E::AnalysisOutput {
                output: "chunk".to_string(),
                iteration: 1,
            },
            E::AnalysisCompleted { groups_found: 2 },
            E::HookStarted {
                change_id: "change-a".to_string(),
                hook_type: "pre_apply".to_string(),
            },
            E::HookCompleted {
                change_id: "change-a".to_string(),
                hook_type: "post_apply".to_string(),
            },
            E::HookFailed {
                change_id: "change-a".to_string(),
                hook_type: "pre_archive".to_string(),
                error: "hook boom".to_string(),
            },
            E::Warning {
                title: "warning title".to_string(),
                message: "warning message".to_string(),
            },
            E::ParallelStartRejected {
                change_ids: vec!["change-a".to_string()],
                reason: "not eligible".to_string(),
            },
            E::Log(LogEntry::info("log line")),
            E::Stopping,
            E::Stopped,
            E::AllCompleted,
            E::Error {
                message: "process boom".to_string(),
            },
            E::ChangesRefreshed {
                changes: vec![change("change-a")],
                rejected_changes: vec![change("change-b")],
                committed_change_ids: std::collections::HashSet::new(),
                uncommitted_file_change_ids: std::collections::HashSet::new(),
                worktree_change_ids: std::collections::HashSet::new(),
                worktree_paths: std::collections::HashMap::new(),
                worktree_not_ahead_ids: std::collections::HashSet::new(),
                merge_wait_ids: std::collections::HashSet::new(),
            },
            E::WorktreesRefreshed {
                worktrees: vec![worktree()],
            },
            E::BranchMergeStarted {
                branch_name: "change-a".to_string(),
            },
            E::BranchMergeCompleted {
                branch_name: "change-a".to_string(),
            },
            E::BranchMergeFailed {
                branch_name: "change-a".to_string(),
                error: "branch boom".to_string(),
            },
            E::ChangeDequeued {
                change_id: "change-a".to_string(),
            },
            E::ChangeStopped {
                change_id: "change-a".to_string(),
            },
            E::ChangeStopFailed {
                change_id: "change-a".to_string(),
                error: "stop boom".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::ownership_fixtures::all_execution_events;
    use super::*;
    use std::collections::BTreeSet;

    /// The variant count `classify_event` covers.
    ///
    /// Adding an `ExecutionEvent` variant breaks the exhaustive match in
    /// `classify_event` at compile time; this constant then forces the fixture
    /// table — and therefore every ownership and projection assertion below —
    /// to grow with it instead of silently skipping the new variant.
    const EXECUTION_EVENT_VARIANTS: usize = 67;

    #[test]
    fn ownership_table_names_every_variant_exactly_once() {
        let events = all_execution_events();
        let names: BTreeSet<&'static str> = events.iter().map(event_variant_name).collect();

        assert_eq!(
            names.len(),
            events.len(),
            "the fixture table repeats a variant: {:?}",
            events.iter().map(event_variant_name).collect::<Vec<_>>()
        );
        assert_eq!(
            names.len(),
            EXECUTION_EVENT_VARIANTS,
            "a variant was added or removed without updating the ownership fixtures"
        );
    }

    #[test]
    fn only_the_log_variant_is_log_owned() {
        for event in all_execution_events() {
            let (name, ownership) = classify_event(&event);
            let is_log = matches!(ownership, EventOwnership::Log);
            assert_eq!(
                is_log,
                name == "Log",
                "{name} must not claim log ownership without a log payload"
            );
        }
    }

    /// A presentation event promises the operator snapshot cannot change, so it
    /// must not be change-addressed: a change-addressed event feeds timing,
    /// activity, and attention, all of which the snapshot publishes.
    #[cfg(feature = "web-monitoring")]
    #[test]
    fn presentation_events_are_never_change_addressed() {
        for event in all_execution_events() {
            let (name, ownership) = classify_event(&event);
            if !matches!(ownership, EventOwnership::Presentation) {
                continue;
            }
            let (_, change_id, _) =
                crate::web::remote_control_api::projection::describe_event(&event);
            assert!(
                change_id.is_none(),
                "{name} is presentation-only but addresses change {change_id:?}"
            );
        }
    }

    #[test]
    fn all_completed_never_overwrites_a_retained_terminal_mode() {
        assert!(!all_completed_may_overwrite_mode("error"));
        assert!(!all_completed_may_overwrite_mode("stopped"));
        for non_terminal in ["select", "running", "stopping"] {
            assert!(
                all_completed_may_overwrite_mode(non_terminal),
                "{non_terminal} is not a retained terminal mode"
            );
        }
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::ownership_fixtures::all_execution_events;
    use super::*;
    use crate::orchestration::state::{ExecutionMode, OrchestratorState};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Counts deliveries and remembers the dispatch identities it saw.
    #[derive(Default)]
    struct CountingSink {
        events: AtomicUsize,
        state_notifications: AtomicUsize,
        dispatch_ids: tokio::sync::Mutex<Vec<u64>>,
    }

    #[async_trait]
    impl EventSink for CountingSink {
        async fn on_event(&self, _event: &ExecutionEvent) {
            self.events.fetch_add(1, Ordering::SeqCst);
        }

        async fn on_state_changed(&self, _state: &OrchestratorState) {
            self.state_notifications.fetch_add(1, Ordering::SeqCst);
        }

        async fn on_dispatch(&self, dispatch: &EventDispatch<'_>) {
            self.dispatch_ids.lock().await.push(dispatch.id);
            self.on_event(dispatch.event).await;
            if let Some(state) = dispatch.state {
                self.on_state_changed(state).await;
            }
        }
    }

    fn parallel_state(ids: &[&str]) -> tokio::sync::RwLock<OrchestratorState> {
        tokio::sync::RwLock::new(OrchestratorState::with_mode(
            ids.iter().map(|id| id.to_string()).collect(),
            10,
            ExecutionMode::Parallel,
        ))
    }

    fn serial_state(ids: &[&str]) -> tokio::sync::RwLock<OrchestratorState> {
        tokio::sync::RwLock::new(OrchestratorState::with_mode(
            ids.iter().map(|id| id.to_string()).collect(),
            10,
            ExecutionMode::Serial,
        ))
    }

    /// One emitted event is one reducer transition and one delivery per sink,
    /// however many frontends are attached.
    #[tokio::test]
    async fn one_event_is_one_transition_and_one_delivery_per_frontend() {
        for state in [serial_state(&["change-a"]), parallel_state(&["change-a"])] {
            let first = Arc::new(CountingSink::default());
            let second = Arc::new(CountingSink::default());
            let sinks: Vec<Arc<dyn EventSink>> = vec![first.clone(), second.clone()];

            dispatch_event(
                &state,
                &sinks,
                ExecutionEvent::ApplyStarted {
                    change_id: "change-a".to_string(),
                    command: "apply".to_string(),
                },
            )
            .await;
            dispatch_event(
                &state,
                &sinks,
                ExecutionEvent::ApplyCompleted {
                    change_id: "change-a".to_string(),
                    revision: "rev-1".to_string(),
                },
            )
            .await;

            assert_eq!(
                state.read().await.apply_count("change-a"),
                1,
                "a counter must advance once, not once per attached frontend"
            );
            for sink in [&first, &second] {
                assert_eq!(sink.events.load(Ordering::SeqCst), 2);
                assert_eq!(
                    sink.state_notifications.load(Ordering::SeqCst),
                    2,
                    "a state-owning event carries the reducer output it produced"
                );
            }
        }
    }

    /// Every dispatch carries a distinct identity, which is what lets a frontend
    /// recognise a repeated delivery instead of guessing from event content.
    #[tokio::test]
    async fn every_dispatch_carries_a_distinct_identity() {
        let state = serial_state(&["change-a"]);
        let sink = Arc::new(CountingSink::default());
        let sinks: Vec<Arc<dyn EventSink>> = vec![sink.clone()];

        for _ in 0..3 {
            dispatch_event(&state, &sinks, ExecutionEvent::Log(LogEntry::info("same"))).await;
        }

        let ids = sink.dispatch_ids.lock().await.clone();
        assert_eq!(ids.len(), 3);
        let unique: std::collections::BTreeSet<u64> = ids.iter().copied().collect();
        assert_eq!(
            unique.len(),
            3,
            "two identical log events are still two dispatches"
        );
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids must be ordered");
    }

    /// Log and presentation events cannot change the snapshot, so dispatch does
    /// not pay to clone the reducer for them.
    #[tokio::test]
    async fn only_state_owned_events_carry_reducer_output() {
        let state = serial_state(&["change-a"]);
        let sink = Arc::new(CountingSink::default());
        let sinks: Vec<Arc<dyn EventSink>> = vec![sink.clone()];

        for event in all_execution_events() {
            let expected_state = matches!(event_ownership(&event), EventOwnership::State);
            let before = sink.state_notifications.load(Ordering::SeqCst);
            dispatch_event(&state, &sinks, event.clone()).await;
            let after = sink.state_notifications.load(Ordering::SeqCst);
            assert_eq!(
                after - before,
                usize::from(expected_state),
                "{} carried the wrong state ownership",
                event_variant_name(&event)
            );
        }
    }

    /// The bridge is the path a producer that can only speak `mpsc::Sender`
    /// takes into the single dispatch owner.
    #[tokio::test]
    async fn bridged_producers_reach_the_reducer_and_every_sink() {
        let sink = Arc::new(CountingSink::default());
        let dispatcher = Arc::new(EventDispatcher::new(
            Arc::new(serial_state(&["change-a"])),
            vec![sink.clone()],
        ));
        let (bridge, handle) = dispatcher.bridge(8);

        bridge
            .send(ExecutionEvent::ProcessingStarted("change-a".to_string()))
            .await
            .expect("bridge accepts events");
        bridge
            .send(ExecutionEvent::Log(LogEntry::info("hook output")))
            .await
            .expect("bridge accepts logs");
        drop(bridge);
        handle.await.expect("bridge task ends when senders drop");

        assert_eq!(sink.events.load(Ordering::SeqCst), 2);
        assert_eq!(
            sink.state_notifications.load(Ordering::SeqCst),
            1,
            "the log is observational; the lifecycle event is not"
        );
    }

    /// Producers that must record a hold in the reducer synchronously — parallel
    /// dispatch suppression cannot wait for a channel round trip — apply the
    /// event themselves *and* publish it, so the dispatch owner applies it a
    /// second time. That is the documented exception, and it is only safe
    /// because those reducer transitions are idempotent.
    #[tokio::test]
    async fn producer_preapplied_events_are_idempotent_in_the_reducer() {
        use crate::orchestration::state::OrchestratorState;
        let preapplied = [
            ExecutionEvent::ChangeArchived("change-a".to_string()),
            ExecutionEvent::AcceptanceGated {
                change_id: "change-a".to_string(),
                blocker: super::ownership_fixtures::blocker(),
            },
            ExecutionEvent::ExecutionBlocked {
                change_id: "change-a".to_string(),
                blocker: super::ownership_fixtures::blocker(),
            },
            ExecutionEvent::MergeCompleted {
                change_id: "change-a".to_string(),
                revision: "rev-1".to_string(),
            },
            ExecutionEvent::HookFailed {
                change_id: "change-a".to_string(),
                hook_type: "on_merged".to_string(),
                error: "boom".to_string(),
            },
            ExecutionEvent::RejectionReviewCompleted {
                change_id: "change-a".to_string(),
                outcome: RejectionOutcome::Confirm,
            },
            ExecutionEvent::RejectionReviewFailed {
                change_id: "change-a".to_string(),
                error: "boom".to_string(),
            },
        ];

        for event in preapplied {
            let mut once = OrchestratorState::with_mode(
                vec!["change-a".to_string()],
                10,
                ExecutionMode::Parallel,
            );
            let mut twice = OrchestratorState::with_mode(
                vec!["change-a".to_string()],
                10,
                ExecutionMode::Parallel,
            );

            once.apply_execution_event(&event);
            twice.apply_execution_event(&event);
            twice.apply_execution_event(&event);

            let name = event_variant_name(&event);
            assert_eq!(
                once.all_display_statuses(),
                twice.all_display_statuses(),
                "{name} changed display status on the second application"
            );
            assert_eq!(
                once.changes_processed(),
                twice.changes_processed(),
                "{name} double-counted processed changes"
            );
            assert_eq!(
                once.apply_count("change-a"),
                twice.apply_count("change-a"),
                "{name} double-counted applies"
            );
            assert_eq!(
                once.remaining_changes(),
                twice.remaining_changes(),
                "{name} double-counted remaining changes"
            );
        }
    }
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
    /// Operator-facing repair-stop diagnostics must be machine-readable so
    /// CLI/TUI/event consumers can explain the stop without parsing narrative
    /// logs — and must never read as completion, PASS, or archive readiness.
    #[test]
    fn acceptance_repair_stop_diagnostics_are_structured_and_non_authoritative() {
        use crate::orchestration::acceptance::{
            decide_repair_gate, FindingRepairLedger, RepairGateDecision,
        };

        let finding = crate::acceptance::AcceptanceFinding::structured(
            crate::acceptance::RepositoryFinding {
                id: "acceptance-secret-value-scan".to_string(),
                severity: crate::acceptance::FindingSeverity::Minor,
                summary: "Challenge and proof leakage is not tested by value".to_string(),
                evidence: vec!["relay exposes counts but not issued values".to_string()],
                required_changes: vec![crate::acceptance::FindingFileExpectation {
                    file: "tests/support/relay.ts".to_string(),
                    description: "Expose issued challenge and presented proof values".to_string(),
                }],
                verification: vec![crate::acceptance::FindingFileExpectation {
                    file: "runtime/recovery.integration.test.ts".to_string(),
                    description: "Assert recorded values are absent from audit output".to_string(),
                }],
            },
        );

        let RepairGateDecision::Stop(stop) = decide_repair_gate(
            "change-a",
            &[finding],
            &FindingRepairLedger::default(),
            Some("fail-rev"),
            Some("apply-rev"),
            &["tests/calibration.test.ts".to_string()],
            &["adjusted calibration threshold".to_string()],
        ) else {
            panic!("missing declared coverage must hold");
        };

        let json = stop.to_json();
        // Every operator diagnostic the contract requires.
        assert_eq!(json["change_id"], "change-a");
        assert_eq!(json["stop_reason"], "acceptance_remediation_mismatch");
        assert_eq!(json["findings"][0]["id"], "acceptance-secret-value-scan");
        assert_eq!(json["findings"][0]["severity"], "minor");
        assert!(json["findings"][0]["evidence"].is_array());
        assert!(json["findings"][0]["required_changes"].is_array());
        assert!(json["findings"][0]["verification"].is_array());
        assert!(json["finding_occurrences"].is_array());
        assert_eq!(json["fail_revision"], "fail-rev");
        assert_eq!(json["apply_revision"], "apply-rev");
        assert_eq!(json["required_files"][0], "tests/support/relay.ts");
        assert_eq!(
            json["verification_files"][0],
            "runtime/recovery.integration.test.ts"
        );
        assert_eq!(json["changed_files"][0], "tests/calibration.test.ts");
        assert_eq!(json["unrelated_files"][0], "tests/calibration.test.ts");
        assert_eq!(json["coverage_complete"], false);
        assert!(json["uncovered_files"].as_array().unwrap().len() == 2);
        assert_eq!(
            json["remediation_evidence"][0],
            "adjusted calibration threshold"
        );
        assert_eq!(json["resumable"], true);
        assert!(json["next_action"].as_str().unwrap().contains("retry"));

        // Explicitly not authority for anything the constitution reserves.
        assert_eq!(json["proves_completion"], false);
        assert_eq!(json["proves_acceptance_pass"], false);
        assert_eq!(json["proves_archive_readiness"], false);

        // The one-line summary an operator sees carries the same evidence.
        let summary = stop.summary();
        assert!(
            summary.contains("acceptance_remediation_mismatch"),
            "{summary}"
        );
        assert!(summary.contains("tests/support/relay.ts"), "{summary}");
    }

    /// A repeated-finding stop keeps occurrence counts so operators can see the
    /// automatic repair budget was already spent.
    #[test]
    fn repeated_finding_diagnostics_report_occurrence_counts() {
        use crate::orchestration::acceptance::{
            normalize_findings, repeated_finding_stop, FindingRepairDecision, FindingRepairLedger,
        };

        let findings = crate::acceptance::legacy_findings(["Missing retry test at src/run.rs:10"]);
        let normalized = normalize_findings(&findings);
        let mut ledger = FindingRepairLedger::default();
        let FindingRepairDecision::Repair { identities } = ledger.observe_fail(&normalized) else {
            panic!("first observation must allow one repair");
        };
        ledger.record_repair_dispatched(&identities);
        let FindingRepairDecision::Stop {
            repeated_identities,
            ..
        } = ledger.observe_fail(&normalized)
        else {
            panic!("the repeated identity must stop automatic repair");
        };

        let json = repeated_finding_stop(
            "change-a",
            &findings,
            &ledger,
            repeated_identities,
            Some("fail-rev"),
            Some("apply-rev"),
            &["src/run.rs".to_string()],
            &[],
        )
        .to_json();

        assert_eq!(json["stop_reason"], "repeated_acceptance_finding");
        assert_eq!(
            json["finding_occurrences"][0]["identity"],
            "repository|src/run.rs|verification"
        );
        assert_eq!(json["finding_occurrences"][0]["occurrences"], 2);
        assert_eq!(
            json["repeated_identities"][0],
            "repository|src/run.rs|verification"
        );
        assert_eq!(json["resumable"], true);
        // A legacy finding declares no path set, so it is reported as such rather
        // than being held to strict coverage.
        assert_eq!(
            json["legacy_findings_without_declared_paths"][0],
            "Missing retry test at src/run.rs:10"
        );
    }
}
