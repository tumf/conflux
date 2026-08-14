//! Unified event system for OpenSpec Orchestrator
//!
//! This module provides a single event type that unifies the previously separate
//! ParallelEvent (from parallel execution) and OrchestratorEvent (from TUI) types.
//!
//! The ExecutionEvent enum represents all possible events that can occur during
//! change processing across every frontend.

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
    /// Optional change_id for change-scoped logs
    pub change_id: Option<String>,
    /// Optional operation type (apply, archive, resolve)
    pub operation: Option<String>,
    /// Optional iteration number (for apply operations)
    pub iteration: Option<u32>,
    /// Optional workspace path (for logs with workspace context)
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

    /// Set change_id for change-scoped logs
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

    /// Set workspace path (for logs with workspace context)
    #[allow(dead_code)]
    pub fn with_workspace_path(mut self, workspace_path: impl Into<String>) -> Self {
        self.workspace_path = Some(workspace_path.into());
        self
    }
}

/// Unified event type for all execution events
///
/// This enum combines every execution event a run can publish,
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

/// Ephemeral presentation phase of the final Apply commit sequence.
///
/// This is *not* a lifecycle state. The canonical activity stays `applying` for
/// the whole sequence; the phase exists so an operator can tell "the agent is
/// working" apart from "repository hooks are running and nothing will move
/// until they finish". It is process-local, never persisted, and never read by
/// scheduling, resume, acceptance, archive, or merge decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyCommitPhase {
    /// The finalization sequence began: stage gate, WIP snapshot, hooks, commit.
    Started,
    /// The verified commit exists and the workspace is clean.
    Completed,
    /// Finalization stopped without a usable commit (stage gate, hook
    /// rejection, dirty post-commit workspace, or a terminal VCS failure).
    Failed,
}

impl ApplyCommitPhase {
    /// Stable machine-readable label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    /// Whether this phase leaves commit presentation active.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Started)
    }
}

/// Which standard stream one streamed final-commit line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitOutputStream {
    Stdout,
    Stderr,
}

impl CommitOutputStream {
    /// Stable machine-readable label carried into logs and events.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// A process-level decision fact one accepted operator command produced.
///
/// This vocabulary exists only for accepted effects that no *existing* execution
/// event already describes exactly. Graceful stop is still `Stopping`, a settled
/// force stop is still `Stopped`, and a successful target dequeue is still
/// `ChangeDequeued`; adding a second spelling for any of those would give the
/// same fact two authorities.
///
/// It deliberately carries no change lifecycle: the reducer owns that, and
/// [`crate::orchestration::state::OrchestratorState::apply_execution_event`]
/// applies nothing for this event. What travels here is the process-level
/// decision — mode, marks, queue intent, resolver ownership — that a frontend
/// would otherwise have to re-derive from its own cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorCommandEffect {
    /// A run was dispatched over exactly these targets.
    RunDispatched {
        /// Accepted targets, in request order.
        change_ids: Vec<String>,
        /// True when the run consumes reconciled holds instead of rerunning apply.
        explicit_retry: bool,
        /// True only when a new scheduler run was actually spawned.
        ///
        /// False means a scheduler that was already alive — including one parked
        /// in persistent-idle Ready — was merely woken. The queue intent is real
        /// either way, but nothing has been admitted for execution yet, so a
        /// frontend must not project Running off a wake. Carrying the fact here
        /// rather than letting each frontend re-derive it from its own cache is
        /// what keeps the TUI and `/api/v2` reporting the same run identically.
        scheduler_started: bool,
    },
    /// A pending graceful stop was withdrawn.
    StopCancelled,
    /// Force stop was accepted and the scheduler owns the terminal stop.
    ///
    /// The settled case emits `Stopped` instead; this variant is only the
    /// interval during which in-flight work must still reach its
    /// cancellation-safe boundary.
    ForceStopAwaitingBoundary {
        /// True when the activity snapshot justified reporting a *force* stop.
        force_stop: bool,
    },
    /// Execution marks changed for exactly these IDs, all to `marked`.
    MarkDelta {
        /// Only the IDs this command actually wrote.
        change_ids: Vec<String>,
        /// The value every named ID now carries.
        marked: bool,
    },
    /// Queue intent changed for one target.
    QueueDelta {
        /// Target change.
        change_id: String,
        /// True when the target is now queued.
        queued: bool,
    },
    /// A resolve reservation was taken.
    ResolveReserved {
        /// Target change.
        change_id: String,
        /// True when the change became the single active resolver.
        active: bool,
    },
}

impl OperatorCommandEffect {
    /// The single change this effect addresses, when it addresses one.
    ///
    /// A multi-target effect is deliberately process-addressed: naming one of
    /// several targets would make the remote stream's `change_id` slot describe
    /// an arbitrary member of the set.
    pub fn change_id(&self) -> Option<&str> {
        match self {
            Self::QueueDelta { change_id, .. } | Self::ResolveReserved { change_id, .. } => {
                Some(change_id)
            }
            Self::MarkDelta { change_ids, .. } if change_ids.len() == 1 => {
                Some(change_ids[0].as_str())
            }
            _ => None,
        }
    }

    /// Stable machine-readable label for logs and the remote payload.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RunDispatched { .. } => "run_dispatched",
            Self::StopCancelled => "stop_cancelled",
            Self::ForceStopAwaitingBoundary { .. } => "force_stop_awaiting_boundary",
            Self::MarkDelta { .. } => "mark_delta",
            Self::QueueDelta { .. } => "queue_delta",
            Self::ResolveReserved { .. } => "resolve_reserved",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    // Lifecycle events
    /// Processing started for a change.
    ///
    /// Consumed, not synthesized: a frontend must never fabricate this to signal
    /// that a command was accepted, because it would set a current change and
    /// activity before the scheduler actually started that target. The TUI's
    /// cancel-stop projection likewise stopped borrowing it as a "return to
    /// running" signal, so no production path emits it today. Core mode and every
    /// projection still classify and reduce it, so the reducer contract stays
    /// intact and a real producer needs no new wiring; dead-code analysis just
    /// sees no constructor.
    #[allow(dead_code)]
    ProcessingStarted(String),
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
    /// Ephemeral final-commit presentation phase for one change.
    ///
    /// Carries the finalization attempt so retries stay distinguishable. The
    /// reducer keeps it in process memory for rendering only; the canonical
    /// lifecycle remains `applying` throughout.
    ApplyCommitPhase {
        change_id: String,
        phase: ApplyCommitPhase,
        attempt: u32,
    },
    /// One streamed line of final Apply commit output.
    ///
    /// Emitted while the commit process is still running so long-running
    /// repository hooks stay observable. The complete raw streams and exit
    /// status are preserved separately for rejection and lock classification;
    /// these lines are presentation only.
    ApplyCommitOutput {
        change_id: String,
        attempt: u32,
        stream: CommitOutputStream,
        line: String,
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

    // Workspace events
    /// Managed workspace preparation started for a scheduler-admitted change.
    ///
    /// Emitted after the execution-slot permit is acquired and the stop/terminal
    /// gates pass, immediately before force-recreate cleanup, worktree
    /// creation/recreation, or `.wt/setup` — the work that would otherwise leave
    /// the change rendered as `queued` for minutes.
    ///
    /// Purely a process-local observability/orchestration transition: it is
    /// never persisted and never participates in resume routing, which stays
    /// derived from workspace files, Git state, and base-tree comparison.
    WorkspacePreparationStarted { change_id: String },
    /// Managed workspace preparation ended without a next-phase event.
    ///
    /// Clears an in-memory `preparing` activity so a dispatch that leaves before
    /// any `*Started` event — global cancellation, a terminal resume route, a
    /// pre-spawn early return — cannot leave the change displayed as `preparing`
    /// forever. A no-op once another transition already moved the change on.
    WorkspacePreparationEnded { change_id: String },
    /// A workspace was created
    #[allow(dead_code)]
    WorkspaceCreated {
        change_id: String,
        workspace: String,
    },
    /// Workspace status synchronization for a specific change
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

    // Merge events
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

    // Analysis events
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
    /// A persistent scheduler parked with no executable work.
    ///
    /// Emitted once per continuous idle episode, immediately before the
    /// scheduler enters its existing event-driven idle wait. It is a
    /// process-level presentation transition, not a completion: the scheduler
    /// stays alive, command-addressable, and resumable through its existing wake
    /// sources, so this event deliberately carries no change ID, no outcome, and
    /// no success claim.
    PersistentSchedulerIdle,
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
    /// One accepted operator command's process-level decision facts.
    ///
    /// Dispatched inside the same critical section that commits the command's
    /// staged reducer, mark, queue, resolve, and stop effects, which is what
    /// makes the accepted mode and the accepted decision state one revision.
    OperatorCommandApplied {
        /// What the command actually decided.
        effect: OperatorCommandEffect,
    },
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
        E::ProcessingError { .. } => ("ProcessingError", State),
        E::ApplyStarted { .. } => ("ApplyStarted", State),
        E::ApplyCompleted { .. } => ("ApplyCompleted", State),
        E::ApplyFailed { .. } => ("ApplyFailed", State),
        E::ApplyOutput { .. } => ("ApplyOutput", State),
        // The reducer keeps the commit phase in process memory for rendering,
        // so this event owns a state transition even though it never touches
        // the canonical lifecycle.
        E::ApplyCommitPhase { .. } => ("ApplyCommitPhase", State),
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
        E::WorkspacePreparationStarted { .. } => ("WorkspacePreparationStarted", State),
        E::WorkspacePreparationEnded { .. } => ("WorkspacePreparationEnded", State),
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
        // State-owning even though the reducer applies no workflow mutation for
        // it: frontend `app_mode` and the operator snapshot's
        // `persistent_scheduler_idle` field both change at this dispatch, so it
        // owns one coherent revision like every other process-level mode event.
        E::PersistentSchedulerIdle => ("PersistentSchedulerIdle", State),
        E::Error { .. } => ("Error", State),
        E::ChangesRefreshed { .. } => ("ChangesRefreshed", State),
        E::WorktreesRefreshed { .. } => ("WorktreesRefreshed", State),
        E::OperatorCommandApplied { .. } => ("OperatorCommandApplied", State),
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
        // Streamed commit lines are log detail: they carry no field the
        // operator snapshot is built from, and they arrive at hook volume.
        E::ApplyCommitOutput { .. } => ("ApplyCommitOutput", Presentation),
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
///
/// Lives in the crate (behind `cfg(test)`) rather than in one test file because
/// the ownership table and the remote projection tests name the *same* variants
/// from the *same* classifier: a variant renamed here must move in both places
/// at once. Production code never needs the name — it reads the ownership half
/// of [`classify_event`] through [`event_ownership`].
#[cfg(test)]
pub(crate) fn event_variant_name(event: &ExecutionEvent) -> &'static str {
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

/// The one frontend mode a persistent-idle transition is allowed to leave.
pub const PERSISTENT_IDLE_SOURCE_MODE: &str = "running";

/// Whether a persistent-idle transition may project Ready over `current_mode`.
///
/// Only an actively running frontend becomes Ready. Pre-run `select` is already
/// Ready and owns no idle episode; `stopping` describes an operator request that
/// is still pending; `stopped` and `error` are authoritative terminal modes. A
/// late idle event delivered in any of those must leave both the mode and the
/// idle-episode fact exactly as they are, or a frontend would advertise
/// live-scheduler controls for a run that already ended.
///
/// TUI and the `/api/v2` projection route the decision through here so they
/// cannot disagree about the same dispatch.
pub fn persistent_idle_may_project_ready(current_mode: &str) -> bool {
    current_mode == PERSISTENT_IDLE_SOURCE_MODE
}

/// The `app_mode` token a persistent-idle Ready episode is presented as.
pub const PERSISTENT_IDLE_READY_MODE: &str = "select";

/// Whether an accepted `RunDispatched` outcome opens the operator-visible run
/// episode over persistent-idle Ready.
///
/// `RunDispatched` distinguishes only whether a *new* scheduler was spawned, so
/// this reads the rest of the transition from the projection's own ordered
/// facts: Ready presented over a live parked scheduler, no new scheduler, and at
/// least one target the shared transaction actually committed. Together those
/// mean exactly one thing — shared run control revalidated the existing
/// scheduler, committed reducer queue or explicit-retry intent, and woke it —
/// which is what makes an accepted F5 visible immediately instead of after
/// dependency analysis.
///
/// Every publisher of this effect shape must therefore carry scheduler-wake
/// evidence for a non-empty committed target set. The one publisher that never
/// held any — a bare retry *plan*, which is a reducer transition rather than a
/// dispatch — publishes nothing rather than reusing this shape.
///
/// Raw key input, a refused Start, an empty target set, and a generic scheduler
/// notification all fail one of the conjuncts, so none of them can project
/// Running. Core, TUI, Web, and the lifecycle mirror route the decision through
/// here so they cannot disagree about the same dispatch.
pub fn accepted_start_opens_idle_run_episode(
    current_mode: &str,
    persistent_scheduler_idle: bool,
    scheduler_started: bool,
    change_ids: &[String],
) -> bool {
    current_mode == PERSISTENT_IDLE_READY_MODE
        && persistent_scheduler_idle
        && !scheduler_started
        && !change_ids.is_empty()
}

/// Whether this event is typed evidence that admitted work actually started.
///
/// This is the shared "the idle episode is over" trigger: it names the
/// state-owning events a scheduler emits only after work has crossed an
/// execution boundary — ordinary workspace preparation, the operation agents,
/// and scheduler-owned resolve/rejection/base-lane work.
///
/// Deliberately excluded: `AnalysisStarted`, queue notification, catalog
/// refresh, and a bare Start notification. None of them proves anything is
/// executing, so none of them may take a frontend out of persistent-idle Ready.
pub fn is_admitted_work_start(event: &ExecutionEvent) -> bool {
    use ExecutionEvent as E;

    match event {
        E::ProcessingStarted(_)
        | E::WorkspacePreparationStarted { .. }
        | E::ApplyStarted { .. }
        | E::AcceptanceStarted { .. }
        | E::ArchiveStarted { .. }
        | E::ArchiveResumed { .. }
        | E::ResolveStarted { .. }
        | E::PushStarted { .. } => true,
        // The scheduler-owned rejection-review lane has no start event of its
        // own: it announces itself by moving the workspace into an active
        // status. A wait status published by the same event is the opposite
        // evidence, so the active predicate — not the variant — decides.
        E::WorkspaceStatusUpdated { status, .. } => status.is_active(),
        _ => false,
    }
}

/// One authoritative dispatch of an execution event.
///
/// Produced by [`dispatch_event_with_marks`] after the reducer transition has
/// already happened, so a sink receives the event and the state it produced
/// together rather than reading the reducer back out on its own schedule.
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
#[cfg(test)]
pub struct NoopEventSink;

#[cfg(test)]
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
///
/// Test-only. Every production dispatch path binds the process-local mark
/// reconciler and therefore calls [`dispatch_event_with_marks`]; keeping a
/// mark-less shorthand reachable from production would be a second way to
/// dispatch an event that silently skips mark reconciliation.
#[cfg(test)]
pub async fn dispatch_event(
    state: &tokio::sync::RwLock<OrchestratorState>,
    sinks: &[std::sync::Arc<dyn EventSink>],
    event: ExecutionEvent,
) {
    dispatch_event_with_marks(state, sinks, event, None).await;
}

/// What one authoritative dispatch produced, for a caller that must bind a
/// command record to it.
///
/// The identity is the join key: a frontend that projects the dispatch records
/// the revision it allocated under the same identity, so the coordinator can ask
/// for *that* revision instead of sampling whatever the global state happens to
/// be afterwards.
pub trait OutcomeRevisions: Send + Sync {
    /// The projection revision the dispatch with this identity produced.
    ///
    /// `None` when the dispatch was never projected (a frontend that skipped it,
    /// or a window that has already been evicted).
    fn revision_for_dispatch(&self, dispatch_id: u64) -> Option<u64>;

    /// The current projection revision.
    fn current_revision(&self) -> u64;
}

/// The authoritative dispatch owner, with the shared execution-mark store bound.
///
/// The ordering is the whole point, and it is one operation under the shared
/// operator mutation guard:
///
/// 1. capture the target's reducer evidence *before* the event is applied;
/// 2. apply the event to the reducer;
/// 3. compare post-state and revoke only the affected change's shared mark;
/// 4. build the authoritative snapshot;
/// 5. release the guard and fan out to TUI, Web, and every other sink.
///
/// Reconciling before step 5 is what makes a failure event and
/// `execution_marked: false` land in the *same* state revision: no frontend can
/// observe the new reducer state alongside the stale mark, and no frontend has
/// to publish a correction afterwards.
///
/// `reconciler` is `None` for dispatchers with no operator mark store — CLI runs
/// and tests. That is an explicit no-mark binding, not a second store.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn dispatch_event_with_marks(
    state: &tokio::sync::RwLock<OrchestratorState>,
    sinks: &[std::sync::Arc<dyn EventSink>],
    event: ExecutionEvent,
    reconciler: Option<&crate::orchestration::mark_reconciliation::ExecutionMarkReconciler>,
) -> u64 {
    dispatch_event_fully(state, sinks, event, reconciler, None, None).await
}

/// The authoritative dispatch owner, with the process lifecycle mode bound.
///
/// Returns the dispatch identity so a caller that must bind a command record to
/// this exact dispatch can look its revision up through [`OutcomeRevisions`]
/// instead of reading global state back afterwards.
///
/// `mode` is applied *before* the sinks run: every frontend's projection of a
/// lifecycle event and the admission mode the next command is validated against
/// are then the same transition, rather than a frontend cache racing the core.
///
/// `facts` is the process-local observability store. It is updated inside the
/// same critical section, from the reducer state this transition produced, so a
/// typed fact dispatched by a worker before it exits is visible to a settlement
/// that observes the worker as terminated. It is an output, never an input: no
/// workflow decision reads it.
pub async fn dispatch_event_fully(
    state: &tokio::sync::RwLock<OrchestratorState>,
    sinks: &[std::sync::Arc<dyn EventSink>],
    event: ExecutionEvent,
    reconciler: Option<&crate::orchestration::mark_reconciliation::ExecutionMarkReconciler>,
    mode: Option<&crate::orchestration::operator_coordinator::CoreMode>,
    facts: Option<&crate::orchestration::execution_facts::ExecutionFactsStore>,
) -> u64 {
    let ownership = event_ownership(&event);
    let id = next_dispatch_id();

    // Held across pre-capture, application, and reconciliation so an operator
    // mark action cannot land between the transition and its mark decision.
    let mutation = match reconciler {
        Some(reconciler) => Some(reconciler.lock_mutations().await),
        None => None,
    };

    // The core lifecycle transition belongs to the same critical section as the
    // reducer transition: a frontend must never be able to observe the new
    // reducer state alongside the old admission mode.
    if let Some(mode) = mode {
        mode.apply_event(&event);
    }

    let state_snapshot = {
        let mut guard = state.write().await;
        let pre = reconciler.and_then(|reconciler| reconciler.capture(&event, &guard));
        guard.apply_execution_event(&event);
        if let (Some(reconciler), Some(pre)) = (reconciler, pre.as_ref()) {
            let revoked = reconciler.reconcile(&event, pre, &guard);
            for change_id in revoked {
                debug!(
                    "Execution mark revoked for '{}' by {}",
                    change_id,
                    classify_event(&event).0
                );
            }
        }
        // The observability facts are refreshed from the reducer state this
        // transition produced, while the write guard still excludes any other
        // transition. A store updated after the guard was released could
        // publish a phase two events old.
        if let Some(facts) = facts {
            facts.observe(
                id,
                &event,
                matches!(ownership, EventOwnership::State).then_some(&*guard),
                chrono::Utc::now(),
            );
        }
        // Only a state-owning event can change what a frontend publishes;
        // cloning the reducer for every log line would be pure waste.
        matches!(ownership, EventOwnership::State).then(|| guard.clone())
    };

    // Sinks read the reconciled marks; they never take this guard, so releasing
    // it here keeps a slow frontend from stalling operator commands.
    drop(mutation);

    let dispatch = EventDispatch {
        id,
        event: &event,
        ownership,
        state: state_snapshot.as_ref(),
    };

    for sink in sinks {
        sink.on_dispatch(&dispatch).await;
    }

    id
}

/// The process-lifetime authoritative dispatch owner.
///
/// Bundles the reducer state and the frontend sinks so every producer emits
/// through the same path, including producers that can only speak
/// `mpsc::Sender` (see [`EventDispatcher::bridge`]).
///
/// There is exactly one of these per command-capable process. Runner-local
/// producers, accepted operator-command outcomes, and every orchestration run
/// share it, which is what makes "one internal event, one reducer transition,
/// one core mode transition, one delivery per frontend" hold no matter which
/// producer raised the event.
pub struct EventDispatcher {
    state: std::sync::Arc<tokio::sync::RwLock<OrchestratorState>>,
    sinks: Vec<std::sync::Arc<dyn EventSink>>,
    /// The shared execution-mark store this boundary reconciles, when the
    /// process has one. `None` is an explicit no-mark binding.
    marks: Option<crate::orchestration::mark_reconciliation::ExecutionMarkReconciler>,
    /// The process lifecycle mode authoritative lifecycle events transition.
    ///
    /// `None` for a dispatcher with no command-capable core — a headless run,
    /// or a test that only observes the reducer.
    mode: Option<std::sync::Arc<crate::orchestration::operator_coordinator::CoreMode>>,
    /// The process-local execution-facts store this boundary feeds.
    ///
    /// `None` for a process with no observability consumer. It is a pure
    /// output: nothing in the dispatch path reads it back.
    facts: Option<std::sync::Arc<crate::orchestration::execution_facts::ExecutionFactsStore>>,
}

impl EventDispatcher {
    /// Own `state` and fan out to `sinks`.
    pub fn new(
        state: std::sync::Arc<tokio::sync::RwLock<OrchestratorState>>,
        sinks: Vec<std::sync::Arc<dyn EventSink>>,
    ) -> Self {
        Self {
            state,
            sinks,
            marks: None,
            mode: None,
            facts: None,
        }
    }

    /// Bind the process-local execution-mark reconciler.
    ///
    /// Every production dispatcher that can reach a TUI or Web frontend takes
    /// the *same* reconciler the operator command service mutates, so a mark
    /// revoked by an event and a mark set by a command are one value.
    pub fn with_mark_reconciler(
        mut self,
        marks: Option<crate::orchestration::mark_reconciliation::ExecutionMarkReconciler>,
    ) -> Self {
        self.marks = marks;
        self
    }

    /// Bind the process lifecycle mode this boundary transitions.
    pub fn with_core_mode(
        mut self,
        mode: Option<std::sync::Arc<crate::orchestration::operator_coordinator::CoreMode>>,
    ) -> Self {
        self.mode = mode;
        self
    }

    /// Bind the process-local execution-facts store this boundary feeds.
    ///
    /// The *same* store the `/api/v2` execution-status resource reads and
    /// stop-and-dequeue settlement consults, so "what was running" has one
    /// answer no matter who asks.
    pub fn with_execution_facts(
        mut self,
        facts: Option<std::sync::Arc<crate::orchestration::execution_facts::ExecutionFactsStore>>,
    ) -> Self {
        self.facts = facts;
        self
    }

    /// Apply one event and fan it out, reporting its dispatch identity.
    pub async fn dispatch(&self, event: ExecutionEvent) -> u64 {
        dispatch_event_fully(
            &self.state,
            &self.sinks,
            event,
            self.marks.as_ref(),
            self.mode.as_deref(),
            self.facts.as_deref(),
        )
        .await
    }

    /// A channel whose receiver forwards into this dispatch owner.
    ///
    /// Hook runners, output handlers, and the parallel scheduler can only emit
    /// through an `mpsc::Sender`. Handing them a bridge instead of a raw
    /// frontend channel is what stops boundary logs from reaching the TUI
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

/// Build a sink list with no frontend sink (reducer update only).
#[cfg(test)]
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
        // Workspace preparation is real admitted work, so the external lifecycle
        // reports it as working rather than waiting for the first agent command.
        ExecutionEvent::WorkspacePreparationStarted { change_id }
        | ExecutionEvent::ApplyStarted { change_id, .. }
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

        // `PersistentSchedulerIdle` is deliberately absent: the variant alone
        // carries no lifecycle meaning. Only its *accepted* Running-to-Ready
        // transition is idle, and that decision belongs to
        // [`LifecycleModeMirror`], which is what stops a late idle event from
        // reporting `idle` over a stopping, stopped, or failed run.

        // Every other event is progress detail without semantic lifecycle meaning.
        _ => return None,
    };

    Some(LifecycleEvent::StateChanged {
        state,
        context: context(change_id),
    })
}

/// The app-mode token a fresh process starts in.
const INITIAL_APP_MODE: &str = PERSISTENT_IDLE_READY_MODE;

/// What one absorbed event owes the external lifecycle adapter.
///
/// Only the two guarded *mode* transitions live here. Every other lifecycle
/// meaning is still derived per event by
/// [`lifecycle_event_for_execution_event`], so this stays the narrow set the
/// mirror alone can decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleModeTransition {
    /// The event carries no mirror-owned lifecycle transition.
    None,
    /// The guarded Running-to-Ready persistent-idle transition was accepted.
    Idle,
    /// An accepted Start opened the run episode over persistent-idle Ready.
    AcceptedIdleStart,
}

/// Process-local mirror of frontend execution mode for the dispatch lifecycle
/// projection.
///
/// The authoritative dispatch has no frontend to ask which mode an idle event
/// arrived in, so it derives the same answer TUI and Web derive, from the same
/// ordered events, through the same shared guards
/// ([`persistent_idle_may_project_ready`] and
/// [`accepted_start_opens_idle_run_episode`]). It is observation state only: it
/// never authorizes a command and never reaches the reducer.
#[derive(Debug, Clone)]
pub struct LifecycleModeMirror {
    mode: String,
    /// The idle-episode qualifier, mirrored for exactly the same reason the mode
    /// is: an accepted Start's `RunDispatched` says nothing about which kind of
    /// `select` it arrived in, and without that half the mirror would either
    /// miss the `working` edge or refuse the no-work `idle` edge that closes it.
    persistent_idle: bool,
}

impl Default for LifecycleModeMirror {
    fn default() -> Self {
        Self {
            mode: INITIAL_APP_MODE.to_string(),
            persistent_idle: false,
        }
    }
}

impl LifecycleModeMirror {
    /// The mirrored app-mode token.
    ///
    /// Production reads the mirror only through [`Self::absorb`]'s accept/reject
    /// answer; this accessor exists so the tests can pin the intermediate modes
    /// that answer is derived from.
    #[cfg(test)]
    pub fn app_mode(&self) -> &str {
        &self.mode
    }

    /// Absorb one event, reporting the guarded mode transition it performed.
    ///
    /// [`LifecycleModeTransition::Idle`] is the *only* thing that authorizes an
    /// `idle` lifecycle publication for a persistent scheduler: a late or
    /// duplicate idle event leaves the mirrored mode untouched and reports
    /// [`LifecycleModeTransition::None`].
    ///
    /// [`LifecycleModeTransition::AcceptedIdleStart`] is the same contract for
    /// the other end of the episode. Absorbing it is what lets the *next*
    /// no-work park be published: without it the mirror would still read
    /// `select`, the guard would reject that idle edge, and an adapter would sit
    /// at `working` for a scheduler that had already parked.
    pub fn absorb(&mut self, event: &ExecutionEvent) -> LifecycleModeTransition {
        match event {
            ExecutionEvent::PersistentSchedulerIdle => {
                if !persistent_idle_may_project_ready(&self.mode) {
                    return LifecycleModeTransition::None;
                }
                self.mode = INITIAL_APP_MODE.to_string();
                self.persistent_idle = true;
                return LifecycleModeTransition::Idle;
            }
            // The accepted operator outcome, read through the same shared gate
            // Core, TUI, and Web read. A refused Start publishes no outcome at
            // all, and a no-op one carries no committed target, so neither can
            // reach the transition below.
            ExecutionEvent::OperatorCommandApplied {
                effect:
                    OperatorCommandEffect::RunDispatched {
                        change_ids,
                        scheduler_started,
                        ..
                    },
            } => {
                if accepted_start_opens_idle_run_episode(
                    &self.mode,
                    self.persistent_idle,
                    *scheduler_started,
                    change_ids,
                ) {
                    self.mode = PERSISTENT_IDLE_SOURCE_MODE.to_string();
                    self.persistent_idle = false;
                    return LifecycleModeTransition::AcceptedIdleStart;
                }
            }
            ExecutionEvent::Stopping => self.mode = "stopping".to_string(),
            // Both mean "this run is no longer executing"; the guard treats them
            // the same, so they share one token rather than inviting a second
            // terminal vocabulary here.
            ExecutionEvent::Stopped | ExecutionEvent::AllCompleted => {
                self.mode = "stopped".to_string();
                self.persistent_idle = false;
            }
            ExecutionEvent::Error { .. } => {
                self.mode = "error".to_string();
                self.persistent_idle = false;
            }
            // A change-scoped failure is not a process mode. The failed change
            // still projects `blocked` through
            // [`lifecycle_event_for_execution_event`]; what must not happen is
            // the mirror recording a dead process while the scheduler is still
            // executing everything else. Kept as its own arm rather than left to
            // the fallthrough so the scope decision stays visible next to the
            // fatal one it is deliberately not sharing.
            ExecutionEvent::ProcessingError { .. } => {}
            // A pending graceful stop outranks work that started after it was
            // requested: the operator's stop is still owed. Every other mode
            // yields to typed evidence that something is really executing. The
            // idle-episode qualifier ends either way — the same rule Core, TUI,
            // and Web apply — because work really did start.
            event if is_admitted_work_start(event) => {
                self.persistent_idle = false;
                if self.mode != "stopping" {
                    self.mode = PERSISTENT_IDLE_SOURCE_MODE.to_string();
                }
            }
            _ => {}
        }
        LifecycleModeTransition::None
    }
}

/// `EventSink` adapter that projects orchestration events onto an external
/// lifecycle adapter.
///
/// Publishing is non-blocking and failure-isolated, so adding this sink cannot
/// change orchestration timing or routing.
pub struct LifecycleEventSink {
    handle: crate::lifecycle_integration::LifecycleHandle,
    workspace: Option<String>,
    /// Execution-mode mirror backing the guarded persistent-idle projection.
    mode: std::sync::Mutex<LifecycleModeMirror>,
}

impl LifecycleEventSink {
    /// Create a sink that forwards mapped events to `handle`.
    pub fn new(
        handle: crate::lifecycle_integration::LifecycleHandle,
        workspace: Option<String>,
    ) -> Self {
        Self {
            handle,
            workspace,
            mode: std::sync::Mutex::new(LifecycleModeMirror::default()),
        }
    }
}

#[async_trait]
impl EventSink for LifecycleEventSink {
    async fn on_event(&self, event: &ExecutionEvent) {
        // The mirror sees every event, so its answer for the next persistent-idle
        // dispatch is derived from the same ordered stream the frontends read.
        let transition = self
            .mode
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .absorb(event);

        // Both guarded transitions are published from the mirror's answer rather
        // than from the event variant, because the variant alone carries neither
        // meaning: an idle event delivered over a stopped run is not `idle`, and
        // a `RunDispatched` that woke nothing is not `working`.
        let mirrored_state = match transition {
            LifecycleModeTransition::Idle => {
                Some(crate::lifecycle_integration::LifecycleState::Idle)
            }
            LifecycleModeTransition::AcceptedIdleStart => {
                Some(crate::lifecycle_integration::LifecycleState::Working)
            }
            LifecycleModeTransition::None => None,
        };
        if let Some(state) = mirrored_state {
            use crate::lifecycle_integration::LifecycleEvent;
            self.handle.publish(LifecycleEvent::StateChanged {
                state,
                context: crate::lifecycle_integration::LifecycleContext {
                    workspace: self.workspace.clone(),
                    ..Default::default()
                },
            });
            return;
        }

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
            inspection: crate::worktree_ops::InspectionState::Checked,
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
            E::ApplyCommitPhase {
                change_id: "change-a".to_string(),
                phase: ApplyCommitPhase::Started,
                attempt: 2,
            },
            E::ApplyCommitOutput {
                change_id: "change-a".to_string(),
                attempt: 2,
                stream: CommitOutputStream::Stderr,
                line: "pre-commit running".to_string(),
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
            E::WorkspacePreparationStarted {
                change_id: "change-a".to_string(),
            },
            E::WorkspacePreparationEnded {
                change_id: "change-a".to_string(),
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
            E::PersistentSchedulerIdle,
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
    const EXECUTION_EVENT_VARIANTS: usize = 71;

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
    use crate::orchestration::state::OrchestratorState;
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

    fn reducer_state(ids: &[&str]) -> tokio::sync::RwLock<OrchestratorState> {
        tokio::sync::RwLock::new(OrchestratorState::new(
            ids.iter().map(|id| id.to_string()).collect(),
            10,
        ))
    }

    /// One emitted event is one reducer transition and one delivery per sink,
    /// however many frontends are attached.
    #[tokio::test]
    async fn one_event_is_one_transition_and_one_delivery_per_frontend() {
        {
            let state = reducer_state(&["change-a"]);
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
        let state = reducer_state(&["change-a"]);
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
        let state = reducer_state(&["change-a"]);
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
            Arc::new(reducer_state(&["change-a"])),
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
            let mut once = OrchestratorState::new(vec!["change-a".to_string()], 10);
            let mut twice = OrchestratorState::new(vec!["change-a".to_string()], 10);

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

    /// The idle event carries no unconditional lifecycle meaning of its own:
    /// only its accepted Running-to-Ready transition is `idle`.
    #[test]
    fn persistent_idle_event_alone_projects_no_lifecycle_state() {
        assert_eq!(state_of(&ExecutionEvent::PersistentSchedulerIdle), None);
    }

    /// Only an admitted-work start begins a run for the mirror; a Start
    /// notification, a queue notification, analysis, and a refresh do not.
    #[test]
    fn only_admitted_work_events_are_execution_evidence() {
        for event in [
            ExecutionEvent::WorkspacePreparationStarted {
                change_id: "change-a".to_string(),
            },
            ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            },
            ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "resolve".to_string(),
            },
            ExecutionEvent::WorkspaceStatusUpdated {
                change_id: "change-a".to_string(),
                workspace_name: "ws".to_string(),
                status: crate::vcs::WorkspaceStatus::Rejecting,
            },
        ] {
            assert!(
                is_admitted_work_start(&event),
                "{} must be admitted-work evidence",
                event_variant_name(&event)
            );
        }

        for event in [
            ExecutionEvent::AnalysisStarted {
                remaining_changes: 2,
                attempt_id: "a1".to_string(),
            },
            ExecutionEvent::WorktreesRefreshed { worktrees: vec![] },
            ExecutionEvent::Log(LogEntry::info("queued for the running scheduler")),
            // A workspace that just entered a wait is the opposite of a start.
            ExecutionEvent::WorkspaceStatusUpdated {
                change_id: "change-a".to_string(),
                workspace_name: "ws".to_string(),
                status: crate::vcs::WorkspaceStatus::MergeWait,
            },
        ] {
            assert!(
                !is_admitted_work_start(&event),
                "{} must not claim execution started",
                event_variant_name(&event)
            );
        }
    }

    /// Verification `persistent-idle-ready-regressions`: the authoritative
    /// dispatch publishes `idle` for a persistent scheduler that parked, and
    /// only when its guarded Running-to-Ready transition was accepted.
    #[tokio::test]
    async fn persistent_idle_lifecycle_is_idle() {
        // An accepted transition out of a working run publishes exactly one idle.
        let (dispatcher, sink) = mock_sink();
        sink.on_event(&ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-a".to_string(),
        })
        .await;
        sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
            .await;
        assert_eq!(
            dispatcher.states(),
            vec![LifecycleState::Working, LifecycleState::Idle],
            "a parked persistent scheduler must report idle"
        );

        // The idle context is process-level: no change is executing, so none is
        // attributed.
        match dispatcher.published().last().expect("an idle event") {
            LifecycleEvent::StateChanged { context, .. } => {
                assert_eq!(context.workspace.as_deref(), Some("/repo"));
                assert_eq!(context.change_id, None);
            }
            other => panic!("unexpected projection: {other:?}"),
        }

        // A duplicate idle edge and a no-op wake publish nothing further.
        sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
            .await;
        sink.on_event(&ExecutionEvent::AnalysisStarted {
            remaining_changes: 1,
            attempt_id: "attempt-1".to_string(),
        })
        .await;
        sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
            .await;
        assert_eq!(
            dispatcher.states(),
            vec![LifecycleState::Working, LifecycleState::Idle],
            "a duplicate idle edge or a no-op wake must not publish again"
        );

        // Admitted work ends the episode and reports working again.
        sink.on_event(&ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-b".to_string(),
        })
        .await;
        assert_eq!(
            dispatcher.states(),
            vec![
                LifecycleState::Working,
                LifecycleState::Idle,
                LifecycleState::Working
            ],
            "admitted work after idle must report working"
        );

        // A late event against every retained mode publishes no idle transition.
        for terminal in [
            ExecutionEvent::Stopping,
            ExecutionEvent::Stopped,
            ExecutionEvent::Error {
                message: "boom".to_string(),
            },
        ] {
            let (dispatcher, sink) = mock_sink();
            sink.on_event(&ExecutionEvent::WorkspacePreparationStarted {
                change_id: "change-a".to_string(),
            })
            .await;
            sink.on_event(&terminal).await;
            let before = dispatcher.states();
            sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
                .await;
            assert_eq!(
                dispatcher.states(),
                before,
                "a late idle event must not overwrite {}",
                event_variant_name(&terminal)
            );
        }

        // Pre-run Select is already Ready and owns no episode, so a late idle
        // event against a process that never ran publishes nothing either.
        let (dispatcher, sink) = mock_sink();
        sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
            .await;
        assert!(
            dispatcher.states().is_empty(),
            "an idle event observed in pre-run Select must publish nothing"
        );
    }

    /// The mirror's own token is the shared `app_mode` vocabulary, so its answer
    /// is comparable with the one TUI and `/api/v2` evaluate.
    #[test]
    fn lifecycle_mode_mirror_tracks_the_shared_app_mode_vocabulary() {
        let mut mirror = LifecycleModeMirror::default();
        assert_eq!(mirror.app_mode(), "select");

        assert_eq!(
            mirror.absorb(&ExecutionEvent::WorkspacePreparationStarted {
                change_id: "change-a".to_string(),
            }),
            LifecycleModeTransition::None
        );
        assert_eq!(mirror.app_mode(), "running");

        assert_eq!(
            mirror.absorb(&ExecutionEvent::PersistentSchedulerIdle),
            LifecycleModeTransition::Idle
        );
        assert_eq!(mirror.app_mode(), "select");

        // A stop requested from idle Ready leaves Stopping, and work starting
        // under a pending stop does not withdraw it.
        assert_eq!(
            mirror.absorb(&ExecutionEvent::Stopping),
            LifecycleModeTransition::None
        );
        assert_eq!(mirror.app_mode(), "stopping");
        assert_eq!(
            mirror.absorb(&ExecutionEvent::WorkspacePreparationStarted {
                change_id: "change-a".to_string(),
            }),
            LifecycleModeTransition::None
        );
        assert_eq!(mirror.app_mode(), "stopping");
    }

    /// The mirror mirrors a *process* mode, so a change-scoped failure leaves it
    /// alone from every mode it can be in.
    ///
    /// The exact before/after token is asserted rather than "not error", so a
    /// regression that moved the mode somewhere else — or that started reading
    /// the failure message to decide — fails here.
    #[test]
    fn processing_error_preserves_lifecycle_mode() {
        for arranged in ["select", "running", "stopping", "stopped", "error"] {
            let mut mirror = LifecycleModeMirror {
                mode: arranged.to_string(),
                persistent_idle: false,
            };

            for error in [
                "acceptance command attempts exhausted",
                // Deliberately fatal-sounding: scope is the typed variant, never
                // the text, so identical prose must not change the answer.
                "fatal: the orchestrator could not start",
            ] {
                assert_eq!(
                    mirror.absorb(&ExecutionEvent::ProcessingError {
                        id: "alpha".to_string(),
                        error: error.to_string(),
                    }),
                    LifecycleModeTransition::None,
                    "a change-scoped failure authorizes no idle publication"
                );
                assert_eq!(
                    mirror.app_mode(),
                    arranged,
                    "a change-scoped failure must not move the mirrored process mode"
                );
            }
        }
    }

    /// The fatal control for the test above: the global event still lands in
    /// Error, so the change cannot pass by suppressing both.
    #[test]
    fn processing_error_preserves_lifecycle_mode_fatal_control_still_transitions() {
        for arranged in ["select", "running", "stopping", "stopped"] {
            let mut mirror = LifecycleModeMirror {
                mode: arranged.to_string(),
                persistent_idle: false,
            };

            assert_eq!(
                mirror.absorb(&ExecutionEvent::Error {
                    message: "the orchestrator could not start".to_string(),
                }),
                LifecycleModeTransition::None
            );
            assert_eq!(
                mirror.app_mode(),
                "error",
                "a typed global Error is still process-fatal (from {arranged})"
            );
        }
    }

    /// One accepted Start's `RunDispatched`, shaped exactly as run control
    /// publishes it for a woken live scheduler.
    fn accepted_idle_start(change_ids: &[&str]) -> ExecutionEvent {
        ExecutionEvent::OperatorCommandApplied {
            effect: OperatorCommandEffect::RunDispatched {
                change_ids: change_ids.iter().map(|id| (*id).to_string()).collect(),
                explicit_retry: false,
                scheduler_started: false,
            },
        }
    }

    /// Verification `idle-start-running-regressions`: the lifecycle adapter
    /// leaves `idle` on the accepted Start outcome and returns to it when the
    /// woken scheduler admits nothing.
    ///
    /// Driven through the real sink over one ordered stream, because the defect
    /// this closes is a *sequence* defect: absorbing the accepted Start is what
    /// makes the later no-work idle edge publishable at all.
    #[tokio::test]
    async fn idle_start_running_lifecycle_publishes_working_then_returns_to_idle() {
        let (dispatcher, sink) = mock_sink();

        // A run that is really executing, then parks.
        sink.on_event(&ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-a".to_string(),
        })
        .await;
        sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
            .await;

        // The accepted Start opens the run episode.
        sink.on_event(&accepted_idle_start(&["change-a"])).await;

        // Nothing in between is another edge: a second identical outcome finds
        // the episode already closed, and analysis is not admitted work.
        sink.on_event(&accepted_idle_start(&["change-a"])).await;
        sink.on_event(&ExecutionEvent::AnalysisStarted {
            remaining_changes: 1,
            attempt_id: "attempt-1".to_string(),
        })
        .await;

        // No work was admitted, so the rearmed park closes the episode again —
        // once.
        sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
            .await;
        sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
            .await;

        assert_eq!(
            dispatcher.states(),
            vec![
                LifecycleState::Working,
                LifecycleState::Idle,
                LifecycleState::Working,
                LifecycleState::Idle,
            ],
            "accepted Start must publish working and the no-work park must close it"
        );
    }

    /// Verification `idle-start-running-regressions`: every shape that is not an
    /// accepted Start against an open idle episode publishes nothing.
    #[tokio::test]
    async fn idle_start_running_lifecycle_ignores_non_accepted_start_shapes() {
        let (dispatcher, sink) = mock_sink();
        sink.on_event(&ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-a".to_string(),
        })
        .await;
        sink.on_event(&ExecutionEvent::PersistentSchedulerIdle)
            .await;
        let baseline = dispatcher.states();

        for quiet in [
            // A committed-nothing dispatch is not an accepted Start.
            accepted_idle_start(&[]),
            // A queue delta admits work through a non-Start path; typed
            // admitted-work evidence remains its `working` trigger.
            ExecutionEvent::OperatorCommandApplied {
                effect: OperatorCommandEffect::QueueDelta {
                    change_id: "change-a".to_string(),
                    queued: true,
                },
            },
            ExecutionEvent::OperatorCommandApplied {
                effect: OperatorCommandEffect::MarkDelta {
                    change_ids: vec!["change-a".to_string()],
                    marked: true,
                },
            },
            // Analysis and a catalog refresh prove nothing is executing.
            ExecutionEvent::AnalysisStarted {
                remaining_changes: 1,
                attempt_id: "attempt-1".to_string(),
            },
            ExecutionEvent::WorktreesRefreshed {
                worktrees: Vec::new(),
            },
            // A rearmed no-work idle edge reaching an already-idle projection.
            ExecutionEvent::PersistentSchedulerIdle,
        ] {
            sink.on_event(&quiet).await;
            assert_eq!(
                dispatcher.states(),
                baseline,
                "{} must publish no lifecycle transition here",
                event_variant_name(&quiet)
            );
        }
    }

    /// Verification `idle-start-running-regressions`: the accepted Start gate is
    /// a conjunction, and each conjunct is load-bearing.
    #[test]
    fn idle_start_running_gate_requires_every_conjunct() {
        let targets = vec!["change-a".to_string()];
        assert!(accepted_start_opens_idle_run_episode(
            "select", true, false, &targets
        ));

        for (label, mode, idle, started, ids) in [
            (
                "a pre-run Select owns no idle episode",
                "select",
                false,
                false,
                targets.clone(),
            ),
            (
                "a live run is already Running",
                "running",
                true,
                false,
                targets.clone(),
            ),
            (
                "a pending stop is not a run episode",
                "stopping",
                true,
                false,
                targets.clone(),
            ),
            (
                "a spawned scheduler is the other projection",
                "select",
                true,
                true,
                targets.clone(),
            ),
            (
                "a dispatch that committed nothing",
                "select",
                true,
                false,
                Vec::new(),
            ),
        ] {
            assert!(
                !accepted_start_opens_idle_run_episode(mode, idle, started, &ids),
                "{label}"
            );
        }
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

    /// Tool-event summaries are sanitized and bounded by the producer before
    /// they reach CLI/TUI consumers, so `LogEntry` construction must be a no-op
    /// for them. A second pass that re-truncated would replace the truthful
    /// omitted-byte marker with one accounting only for prefix overflow.
    #[test]
    fn sanitizing_an_already_bounded_message_is_idempotent() {
        let complete = format!("[tool_result:tool_x] {}", "x".repeat(1_000_000));
        let once = sanitize_detail(&complete);
        assert_eq!(once.len(), 8_192);

        let twice = sanitize_detail(&once);
        assert_eq!(twice, once);

        let entry = LogEntry::info(once.clone());
        assert_eq!(entry.message, once);
        assert_eq!(entry.message.matches("…[truncated ").count(), 1);

        // The marker still reports what was omitted from the complete summary.
        let marker_start = once.find("…[truncated ").unwrap();
        let omitted = once[marker_start + "…[truncated ".len()..]
            .trim_end_matches(" bytes]")
            .parse::<usize>()
            .unwrap();
        assert_eq!(omitted, complete.len() - marker_start);
    }

    #[test]
    fn sanitizing_an_already_bounded_multibyte_message_is_idempotent() {
        let complete = format!("[tool_result:tool_cjk] {}", "漢".repeat(9_000));
        let once = sanitize_detail(&complete);

        assert!(once.len() <= 8_192);
        assert!(once.is_char_boundary(once.len()));
        assert_eq!(sanitize_detail(&once), once);
        assert_eq!(LogEntry::info(once.clone()).message, once);
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
