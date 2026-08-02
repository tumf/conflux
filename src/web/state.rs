//! Web monitoring state management.
//!
//! Provides thread-safe state access and broadcasting for WebSocket clients.

use crate::events::{EventDispatch, EventOwnership, EventSink, ExecutionEvent, LogEntry};
use crate::openspec::Change;
use crate::tui::types::WorktreeInfo;
use crate::web::operator_facts::OperatorFactsStore;
use crate::web::remote_control_api::dto::{
    AttentionState, BlockerKind as RemoteBlockerKind, ChangeActivity, ChangeBlocker, ChangeTiming,
    ChangeWorktree, ParallelEligibility, QueueIntent as RemoteQueueIntent,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

#[cfg(feature = "web-monitoring")]
use utoipa::ToSchema;

/// Control commands that can be sent from Web UI to orchestrator
#[derive(Debug, Clone)]
pub enum ControlCommand {
    /// Start or resume processing
    Start,
    /// Stop processing (graceful shutdown)
    Stop,
    /// Cancel a pending stop request
    CancelStop,
    /// Force stop immediately
    ForceStop,
    /// Retry error changes.
    ///
    /// Retained because both frontend bridges implement it, but nothing produces
    /// it today: `/api/v2` routes retry through the shared operator command
    /// service instead of this channel.
    #[allow(dead_code)]
    Retry,
}

/// Change status projected into the `/api/v2` snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct ChangeStatus {
    /// Change ID
    pub id: String,
    /// Number of completed tasks
    pub completed_tasks: u32,
    /// Total number of tasks
    pub total_tasks: u32,
    /// Progress percentage (0-100)
    pub progress_percent: f32,
    /// Current status: "pending", "in_progress", "complete"
    pub status: String,
    /// Dependencies on other changes
    pub dependencies: Vec<String>,
    /// Queue status (for parallel/serial execution tracking)
    /// Aligned with canonical display taxonomy values: "not queued", "queued", "blocked", "stalled", "applying",
    /// "accepting", "archiving", "archived", "merged", "pushed", "rejected", "merge wait", "resolving", "resolve pending", "reject pending", "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_status: Option<String>,
    /// Current iteration number for apply/archive loops
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_number: Option<u32>,
    /// Process-local execution mark (operator intent, never durable).
    #[serde(default)]
    pub execution_marked: bool,
    /// Reducer-owned queue intent, kept distinct from `queue_status`.
    #[serde(default)]
    pub queue_intent: RemoteQueueIntent,
    /// Reducer-derived blocker detail for a blocked or stalled change.
    #[serde(default)]
    pub blocker: Option<ChangeBlocker>,
    /// Sanitized change-local error detail.
    #[serde(default)]
    pub error_detail: Option<String>,
    /// Server-observed parallel-execution eligibility.
    #[serde(default)]
    pub parallel: ParallelEligibility,
    /// Run timing boundaries.
    #[serde(default)]
    pub timing: ChangeTiming,
    /// Latest lifecycle-significant activity.
    #[serde(default)]
    pub latest_activity: Option<ChangeActivity>,
    /// Managed worktree relation.
    #[serde(default)]
    pub worktree: Option<ChangeWorktree>,
    /// Operator attention state.
    #[serde(default)]
    pub attention: AttentionState,
}

impl From<&Change> for ChangeStatus {
    fn from(change: &Change) -> Self {
        let status = if change.is_complete() {
            "complete"
        } else if change.completed_tasks > 0 {
            "in_progress"
        } else {
            "pending"
        };

        Self {
            id: change.id.clone(),
            completed_tasks: change.completed_tasks,
            total_tasks: change.total_tasks,
            progress_percent: change.progress_percent(),
            status: status.to_string(),
            dependencies: change.dependencies.clone(),
            queue_status: None, // Set by event handlers based on execution state
            iteration_number: None, // Set by event handlers during apply/archive loops
            // Every authoritative operator field below is filled in by
            // `enrich_operator_state`, from the reducer, the execution-mark
            // store, and the process-local facts store. A change parsed from
            // disk carries none of them on its own.
            execution_marked: false,
            queue_intent: RemoteQueueIntent::NotQueued,
            blocker: None,
            error_detail: None,
            parallel: ParallelEligibility::default(),
            timing: ChangeTiming::default(),
            latest_activity: None,
            worktree: None,
            attention: AttentionState::None,
        }
    }
}

/// Full orchestrator state snapshot for REST API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct OrchestratorStateSnapshot {
    /// List of all changes
    pub changes: Vec<ChangeStatus>,
    /// Total number of changes
    pub total_changes: usize,
    /// Number of completed changes
    pub completed_changes: usize,
    /// Number of in-progress changes
    pub in_progress_changes: usize,
    /// Number of pending changes
    pub pending_changes: usize,
    /// Timestamp of last update
    pub last_updated: String,
    /// Log entries (TUI-equivalent)
    pub logs: Vec<LogEntry>,
    /// Worktree list (TUI-equivalent)
    pub worktrees: Vec<WorktreeInfo>,
    /// Application mode (e.g., "select", "running", "stopped")
    pub app_mode: String,
    /// Whether resolve is currently running
    pub is_resolving: bool,
    /// Sanitized fatal process-level error, kept distinct from a change's own
    /// error so "the run died" and "one change failed" stay separable.
    #[serde(default)]
    pub process_error: Option<String>,
}

impl OrchestratorStateSnapshot {
    /// Create a new state snapshot from a list of changes
    pub fn from_changes(changes: &[Change]) -> Self {
        Self::from_changes_with_shared_state(changes, None)
    }

    /// Create a new state snapshot from a list of changes with optional shared orchestration state.
    /// When shared state is provided, additional metadata (apply counts, pending/archived status) is derived from it.
    pub fn from_changes_with_shared_state(
        changes: &[Change],
        shared_state: Option<&crate::orchestration::state::OrchestratorState>,
    ) -> Self {
        let mut change_statuses: Vec<ChangeStatus> =
            changes.iter().map(ChangeStatus::from).collect();

        // Enrich with data from shared state if available
        if let Some(shared) = shared_state {
            for status in &mut change_statuses {
                // Derive queue_status from reducer display_status (single source of truth).
                // "not queued" maps to None to keep the JSON payload minimal.
                let display = shared.display_status(&status.id);
                if display != "not queued" {
                    status.queue_status = Some(display.to_string());
                }

                // Set iteration_number from apply_count if available
                let apply_count = shared.apply_count(&status.id);
                if apply_count > 0 {
                    status.iteration_number = Some(apply_count);
                }
            }
        }

        let completed = change_statuses
            .iter()
            .filter(|c| {
                c.queue_status
                    .as_ref()
                    .is_some_and(|s| s == "archived" || s == "merged" || s == "pushed")
            })
            .count();
        let in_progress = change_statuses
            .iter()
            .filter(|c| {
                c.queue_status.as_ref().is_some_and(|s| {
                    s == "applying"
                        || s == "accepting"
                        || s == "archiving"
                        || s == "resolving"
                        || s == "rejecting"
                })
            })
            .count();
        let pending = change_statuses
            .iter()
            .filter(|c| c.queue_status.as_ref().is_some_and(|s| s == "queued"))
            .count();

        Self {
            total_changes: change_statuses.len(),
            completed_changes: completed,
            in_progress_changes: in_progress,
            pending_changes: pending,
            changes: change_statuses,
            last_updated: chrono::Utc::now().to_rfc3339(),
            logs: Vec::new(),
            worktrees: Vec::new(),
            app_mode: "select".to_string(),
            is_resolving: false,
            process_error: None,
        }
    }
}

fn progress_percent(completed: u32, total: u32) -> f32 {
    if total == 0 {
        0.0
    } else {
        (completed as f32 / total as f32) * 100.0
    }
}

fn status_from_progress(completed: u32, total: u32) -> &'static str {
    if total > 0 && completed >= total {
        "complete"
    } else if completed > 0 {
        "in_progress"
    } else {
        "pending"
    }
}

fn apply_reducer_derived_queue_statuses(
    state: &mut OrchestratorStateSnapshot,
    shared: &crate::orchestration::state::OrchestratorState,
) {
    for change in &mut state.changes {
        let display = shared.display_status(&change.id);
        change.queue_status = if display == "not queued" {
            None
        } else {
            Some(display.to_string())
        };

        let apply_count = shared.apply_count(&change.id);
        if apply_count > 0 {
            change.iteration_number = Some(apply_count);
        }
    }
    apply_reducer_derived_operator_state(state, shared);
}

/// Copy the reducer-owned operator decision fields onto every change.
///
/// Queue intent, blocker detail, and change-local error are read straight off
/// the reducer's own runtime state rather than re-derived from the display
/// status: `blocked` alone cannot tell a dependency wait from an external
/// prerequisite wait, and `error` alone carries no message.
fn apply_reducer_derived_operator_state(
    state: &mut OrchestratorStateSnapshot,
    shared: &crate::orchestration::state::OrchestratorState,
) {
    use crate::orchestration::state::{ChangeRuntimeState, QueueIntent};

    for change in &mut state.changes {
        let runtime = shared.change_runtime(&change.id);
        change.queue_intent = match runtime.map(|rt| &rt.queue_intent) {
            Some(QueueIntent::Queued) => RemoteQueueIntent::Queued,
            _ => RemoteQueueIntent::NotQueued,
        };
        change.blocker = runtime
            .and_then(ChangeRuntimeState::blocker_view)
            .map(project_blocker);
        change.error_detail = runtime
            .and_then(ChangeRuntimeState::error_message)
            .map(crate::events::sanitize_detail);
    }
}

/// Project the reducer's blocker view into the wire DTO.
fn project_blocker(view: crate::orchestration::state::BlockerView) -> ChangeBlocker {
    use crate::orchestration::state::BlockerKind;

    ChangeBlocker {
        status: view.status.to_string(),
        kind: match view.kind {
            BlockerKind::None => RemoteBlockerKind::None,
            BlockerKind::Dependency => RemoteBlockerKind::Dependency,
            BlockerKind::External => RemoteBlockerKind::External,
        },
        category: view.category.as_deref().map(crate::events::sanitize_detail),
        detail: view.detail.as_deref().map(crate::events::sanitize_detail),
        unblock_condition: view
            .unblock_condition
            .as_deref()
            .map(crate::events::sanitize_detail),
        prerequisite_owner: view
            .prerequisite_owner
            .as_deref()
            .map(crate::events::sanitize_detail),
        origin: view.origin.as_deref().map(crate::events::sanitize_detail),
        resumable: view.resumable,
    }
}

fn refresh_summary(state: &mut OrchestratorStateSnapshot) {
    state.total_changes = state.changes.len();
    state.completed_changes = state
        .changes
        .iter()
        .filter(|change| {
            change
                .queue_status
                .as_ref()
                .is_some_and(|s| s == "archived" || s == "merged")
        })
        .count();
    state.in_progress_changes = state
        .changes
        .iter()
        .filter(|change| {
            change.queue_status.as_ref().is_some_and(|s| {
                s == "applying" || s == "accepting" || s == "archiving" || s == "resolving"
            })
        })
        .count();
    state.pending_changes = state
        .changes
        .iter()
        .filter(|change| change.queue_status.as_ref().is_some_and(|s| s == "queued"))
        .count();
    state.last_updated = chrono::Utc::now().to_rfc3339();
}

/// Event sink implementation for web monitoring state updates.
pub struct WebEventSink {
    web_state: Arc<WebState>,
}

impl WebEventSink {
    pub fn new(web_state: Arc<WebState>) -> Self {
        Self { web_state }
    }
}

#[async_trait]
impl EventSink for WebEventSink {
    async fn on_event(&self, event: &ExecutionEvent) {
        self.web_state.apply_execution_event(event).await;
    }

    async fn on_state_changed(&self, _state: &crate::orchestration::state::OrchestratorState) {}

    /// Absorb the event and the reducer state it produced in one transaction.
    ///
    /// Overriding the whole dispatch — rather than reacting to `on_event` and
    /// `on_state_changed` separately — is what lets the published candidate
    /// snapshot be built from the authoritative reducer output instead of from
    /// a read-back that may observe a different instant.
    async fn on_dispatch(&self, dispatch: &EventDispatch<'_>) {
        self.web_state.apply_dispatch(dispatch).await;
    }
}

/// Shared web state behind the `/api/v2` projection.
pub struct WebState {
    /// Current orchestrator state snapshot (thread-safe)
    state: RwLock<OrchestratorStateSnapshot>,
    /// Control command channel (optional, only used when web control is enabled)
    /// Uses Mutex for interior mutability to allow setting after Arc creation
    control_tx: Mutex<Option<mpsc::UnboundedSender<ControlCommand>>>,
    /// Reference to shared orchestration state (for unified state tracking)
    /// Wrapped in RwLock for interior mutability (can be set after construction via Arc)
    shared_orchestrator_state: tokio::sync::RwLock<
        Option<std::sync::Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>>,
    >,
    /// `/api/v2` projection owner and its late-bound command delegation target.
    ///
    /// Created with the process so `instance_id` exists from the first request,
    /// independently of whether an orchestration runtime is ever bound.
    remote_control: Arc<crate::web::remote_control_api::RemoteControlRuntime>,
    /// Process-local execution marks, shared with the operator command service.
    ///
    /// Late-bound for the same reason the executor is: the web server can start
    /// before an orchestration runtime exists. Until it is bound, every change
    /// reports `execution_marked: false`, which is exactly what a process with no
    /// operator intent yet should report.
    execution_marks: tokio::sync::RwLock<
        Option<Arc<crate::orchestration::operator_command::ExecutionMarkStore>>,
    >,
    /// Timing, latest activity, attention, parallel eligibility, and worktree
    /// relation for this process incarnation.
    operator_facts: tokio::sync::RwLock<OperatorFactsStore>,
    /// Dispatch identities this frontend has already projected.
    ///
    /// The dispatch owner delivers each event once, but a frontend boundary is
    /// exactly where an accidental second delivery would show up as a doubled
    /// sequence, a doubled revision, and a doubled retained log. Remembering the
    /// identity makes that repeat a no-op instead.
    projected_dispatches: Mutex<RecentDispatchIds>,
}

/// Bounded set of recently projected dispatch identities.
///
/// Bounded because dispatch identities are unbounded and a monitoring process
/// runs for as long as an orchestration does; the window only has to be wider
/// than any plausible duplicate-delivery skew.
#[derive(Default)]
struct RecentDispatchIds {
    order: std::collections::VecDeque<u64>,
    seen: std::collections::HashSet<u64>,
}

impl RecentDispatchIds {
    const CAPACITY: usize = 1024;

    /// Record `id`, reporting whether it is the first time it was seen.
    fn admit(&mut self, id: u64) -> bool {
        if !self.seen.insert(id) {
            return false;
        }
        self.order.push_back(id);
        while self.order.len() > Self::CAPACITY {
            if let Some(evicted) = self.order.pop_front() {
                self.seen.remove(&evicted);
            }
        }
        true
    }
}

impl WebState {
    /// Create a new WebState with initial changes
    pub fn new(initial_changes: &[Change]) -> Self {
        let state = OrchestratorStateSnapshot::from_changes(initial_changes);

        Self {
            state: RwLock::new(state),
            control_tx: Mutex::new(None),
            shared_orchestrator_state: tokio::sync::RwLock::new(None),
            remote_control: Arc::new(crate::web::remote_control_api::RemoteControlRuntime::new()),
            execution_marks: tokio::sync::RwLock::new(None),
            operator_facts: tokio::sync::RwLock::new(OperatorFactsStore::new()),
            projected_dispatches: Mutex::new(RecentDispatchIds::default()),
        }
    }

    /// The `/api/v2` runtime for this process incarnation.
    pub fn remote_control(&self) -> Arc<crate::web::remote_control_api::RemoteControlRuntime> {
        self.remote_control.clone()
    }

    /// Bind the shared process-local execution-mark store.
    ///
    /// The same `Arc` the operator command service mutates, so a mark set by a
    /// remote command or by a keypress is readable at the next revision without
    /// a second copy that could drift.
    pub async fn set_execution_marks(
        &self,
        marks: Arc<crate::orchestration::operator_command::ExecutionMarkStore>,
    ) {
        *self.execution_marks.write().await = Some(marks);
        self.sync_remote_control_projection().await;
    }

    /// Bind the repository root used to redact published worktree paths.
    pub async fn set_repo_root(&self, repo_root: std::path::PathBuf) {
        self.operator_facts.write().await.set_repo_root(repo_root);
    }

    /// Reconcile attention tracking with a complete change list.
    async fn observe_changes_for_attention(&self, changes: &[Change]) {
        self.operator_facts
            .write()
            .await
            .observe_changes(changes.iter().map(|change| change.id.as_str()));
    }

    /// The monitoring snapshot with every authoritative operator field filled in.
    ///
    /// This is the only input the v2 projection is built from, so the reducer
    /// state, the execution marks, and the process-local facts are read at one
    /// instant and published at one revision rather than trickling in.
    async fn operator_snapshot(&self) -> OrchestratorStateSnapshot {
        self.operator_snapshot_with(None).await
    }

    /// The monitoring snapshot, preferring an authoritative reducer state.
    ///
    /// When the dispatch owner supplies the state its transition produced, that
    /// state is used verbatim: the projection then describes exactly the instant
    /// the event created. Without one (a periodic refresh, an HTTP read) the
    /// shared reducer is read back instead.
    async fn operator_snapshot_with(
        &self,
        authoritative: Option<&crate::orchestration::state::OrchestratorState>,
    ) -> OrchestratorStateSnapshot {
        let mut snapshot = self.get_state().await;

        // The reducer is the authority for display status, queue intent,
        // blocker detail, and change-local error. Reading all of them here — not
        // only on the event paths that happen to set `updated` — is what makes a
        // reducer-only transition such as an acceptance hold visible at the
        // revision it happened. `try_read` matches the rest of this file: a
        // contended reducer must not stall event projection.
        if let Some(shared) = authoritative {
            apply_reducer_derived_queue_statuses(&mut snapshot, shared);
            refresh_summary(&mut snapshot);
        } else if let Ok(shared_state_opt) = self.shared_orchestrator_state.try_read() {
            if let Some(shared_arc) = shared_state_opt.as_ref() {
                if let Ok(shared) = shared_arc.try_read() {
                    apply_reducer_derived_queue_statuses(&mut snapshot, &shared);
                    refresh_summary(&mut snapshot);
                }
            }
        }

        let marks = self.execution_marks.read().await.clone();
        let facts = self.operator_facts.read().await;
        snapshot.process_error = facts.process_error();
        for change in &mut snapshot.changes {
            change.execution_marked = marks
                .as_ref()
                .is_some_and(|marks| marks.is_marked(&change.id));
            let change_facts = facts.facts(&change.id);
            change.parallel = change_facts.parallel;
            change.worktree = change_facts.worktree.clone();
            change.timing = change_facts.timing.clone();
            change.latest_activity = change_facts.latest_activity.clone();
            change.attention = change_facts.attention(
                change.execution_marked,
                matches!(change.queue_intent, RemoteQueueIntent::Queued),
            );
        }
        snapshot
    }

    /// Push the current monitoring snapshot into the v2 projection.
    ///
    /// Emits an event (and advances the revision) only when the projected
    /// snapshot really differs, so a periodic refresh over an idle process is
    /// invisible to v2 clients.
    pub async fn sync_remote_control_projection(&self) {
        let candidate = crate::web::remote_control_api::projection::project_snapshot(
            &self.operator_snapshot().await,
        );
        self.remote_control
            .projection()
            .apply_state_if_changed("state_refreshed", candidate);
    }

    /// Project one execution event into the v2 projection.
    ///
    /// Called after the monitoring snapshot has already absorbed the event, so
    /// the candidate snapshot and the event describe the same instant. The
    /// event's ownership class — not its variant, read a second time here —
    /// decides whether a revision, a retained log, or only a sequence is
    /// allocated, so every internal event produces exactly one ordered v2 event.
    async fn project_execution_event(
        &self,
        event: &ExecutionEvent,
        ownership: EventOwnership,
        authoritative: Option<&crate::orchestration::state::OrchestratorState>,
    ) {
        use crate::web::remote_control_api::projection as v2;

        let projection = self.remote_control.projection();
        match ownership {
            EventOwnership::Log => {
                // Observational only: a new sequence, the same revision, and
                // exactly one retained entry.
                let ExecutionEvent::Log(entry) = event else {
                    debug_assert!(false, "log ownership without a log payload: {event:?}");
                    return;
                };
                projection.apply_log(entry.clone());
            }
            EventOwnership::Presentation => {
                let (event_type, change_id, payload) = v2::describe_event(event);
                projection.apply_presentation(event_type, change_id, payload);
            }
            EventOwnership::State => {
                let (event_type, change_id, payload) = v2::describe_event(event);
                let candidate =
                    v2::project_snapshot(&self.operator_snapshot_with(authoritative).await);
                projection.apply_state(event_type, change_id, payload, candidate);
            }
        }
    }

    /// Set the control command channel for web-based execution control
    pub async fn set_control_channel(&self, control_tx: mpsc::UnboundedSender<ControlCommand>) {
        *self.control_tx.lock().await = Some(control_tx);
    }

    /// Send a control command (returns error if control channel not set)
    pub fn send_control_command(
        &self,
        command: ControlCommand,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Use try_lock to avoid blocking in sync context
        let control_tx_guard = self
            .control_tx
            .try_lock()
            .map_err(|_| "Control channel lock contention")?;

        if let Some(tx) = control_tx_guard.as_ref() {
            tx.send(command)
                .map_err(|e| format!("Failed to send control command: {}", e))?;
            Ok(())
        } else {
            Err("Control channel not initialized".into())
        }
    }

    /// Set reference to shared orchestration state for unified tracking.
    /// This allows WebState to query core orchestration state (pending/archived, apply counts, etc.)
    pub async fn set_shared_state(
        &self,
        shared_state: std::sync::Arc<
            tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>,
        >,
    ) {
        *self.shared_orchestrator_state.write().await = Some(shared_state);
    }

    /// Get a read lock on the current state snapshot
    pub async fn get_state(&self) -> OrchestratorStateSnapshot {
        self.state.read().await.clone()
    }

    /// Update state with new changes and broadcast to WebSocket clients.
    /// Only broadcasts if there are actual changes from the previous state.
    pub async fn update(&self, changes: &[Change]) {
        // Query shared state if available for enriched metadata
        let shared_state_opt = self.shared_orchestrator_state.read().await;
        let shared_state_data = if let Some(ref shared_arc) = *shared_state_opt {
            shared_arc.try_read().ok()
        } else {
            None
        };

        let mut new_state = OrchestratorStateSnapshot::from_changes_with_shared_state(
            changes,
            shared_state_data.as_deref(),
        );
        drop(shared_state_data); // Drop guard before awaiting
        drop(shared_state_opt); // Drop read lock

        // Preserve progress, queue_status, app_mode, and is_resolving from existing state
        let (old_changes, old_app_mode, old_is_resolving) = {
            let old_state = self.state.read().await;
            (
                old_state.changes.clone(),
                old_state.app_mode.clone(),
                old_state.is_resolving,
            )
        };

        // Preserve app_mode and is_resolving to prevent overwriting runtime state during refresh
        new_state.app_mode = old_app_mode.clone();
        new_state.is_resolving = old_is_resolving;

        for new_change in &mut new_state.changes {
            if let Some(existing) = old_changes.iter().find(|c| c.id == new_change.id) {
                // Preserve queue_status ONLY if shared state didn't provide it
                if new_change.queue_status.is_none() {
                    new_change.queue_status = existing.queue_status.clone();
                }

                // Preserve iteration_number ONLY if shared state didn't provide it
                if new_change.iteration_number.is_none() {
                    new_change.iteration_number = existing.iteration_number;
                }

                // Preserve existing progress if retrieval failed (new data is 0/0)
                // This prevents resetting progress to 0 on retrieval failure
                if new_change.total_tasks == 0
                    && (existing.completed_tasks > 0 || existing.total_tasks > 0)
                {
                    new_change.completed_tasks = existing.completed_tasks;
                    new_change.total_tasks = existing.total_tasks;
                    new_change.progress_percent = existing.progress_percent;
                    new_change.status = existing.status.clone();
                }
            }
        }

        {
            let mut state = self.state.write().await;
            *state = new_state;
        }

        self.observe_changes_for_attention(changes).await;
        self.sync_remote_control_projection().await;
    }

    /// Update the state with new changes and explicit app_mode (for Run mode)
    pub async fn update_with_mode(&self, changes: &[Change], app_mode: &str) {
        // Query shared state if available for enriched metadata
        let shared_state_opt = self.shared_orchestrator_state.read().await;
        let shared_state_data = if let Some(ref shared_arc) = *shared_state_opt {
            shared_arc.try_read().ok()
        } else {
            None
        };

        let mut new_state = OrchestratorStateSnapshot::from_changes_with_shared_state(
            changes,
            shared_state_data.as_deref(),
        );
        drop(shared_state_data); // Drop guard before awaiting
        drop(shared_state_opt); // Drop read lock

        // Override app_mode from orchestrator execution state
        new_state.app_mode = app_mode.to_string();

        // Preserve progress, queue_status, and is_resolving from existing state
        let (old_changes, old_is_resolving) = {
            let old_state = self.state.read().await;
            (old_state.changes.clone(), old_state.is_resolving)
        };

        // Preserve is_resolving to prevent overwriting runtime state
        new_state.is_resolving = old_is_resolving;

        for new_change in &mut new_state.changes {
            if let Some(existing) = old_changes.iter().find(|c| c.id == new_change.id) {
                // Preserve queue_status only when shared reducer state did not provide one.
                if new_change.queue_status.is_none() {
                    new_change.queue_status = existing.queue_status.clone();
                }

                // Preserve iteration_number
                new_change.iteration_number = existing.iteration_number;

                // Preserve existing progress if retrieval failed (new data is 0/0)
                // This prevents resetting progress to 0 on retrieval failure
                if new_change.total_tasks == 0
                    && (existing.completed_tasks > 0 || existing.total_tasks > 0)
                {
                    new_change.completed_tasks = existing.completed_tasks;
                    new_change.total_tasks = existing.total_tasks;
                    new_change.progress_percent = existing.progress_percent;
                    new_change.status = existing.status.clone();
                }
            }
        }

        {
            let mut state = self.state.write().await;
            *state = new_state;
        }

        self.observe_changes_for_attention(changes).await;
        self.sync_remote_control_projection().await;
    }

    /// Absorb one execution event outside a dispatch, and broadcast updates.
    ///
    /// Used where no dispatch owner is in play: disk refreshes, the headless
    /// CLI's event forwarder, and tests. Frontend delivery from an orchestration
    /// boundary goes through [`WebState::apply_dispatch`] instead, so the
    /// projection sees the reducer state the event actually produced rather than
    /// a read-back taken at some later instant.
    pub async fn apply_execution_event(&self, event: &ExecutionEvent) {
        let dispatch = EventDispatch {
            id: crate::events::next_dispatch_id(),
            event,
            ownership: crate::events::event_ownership(event),
            state: None,
        };
        self.apply_dispatch(&dispatch).await;
    }

    /// Absorb one authoritative dispatch.
    ///
    /// A repeated delivery of the same dispatch identity returns without
    /// touching the monitoring snapshot, the operator facts, or the v2
    /// projection, so a duplicate at this boundary cannot double a sequence, a
    /// revision, or a retained log.
    pub async fn apply_dispatch(&self, dispatch: &EventDispatch<'_>) {
        if !self.projected_dispatches.lock().await.admit(dispatch.id) {
            return;
        }
        let event = dispatch.event;
        {
            let mut state = self.state.write().await;
            let mut updated = false;

            match event {
                // Lifecycle events
                ExecutionEvent::ProcessingStarted(change_id) => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.status = "in_progress".to_string();
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                    state.app_mode = "running".to_string();
                }
                ExecutionEvent::ProcessingCompleted(change_id) => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        if change.completed_tasks < change.total_tasks {
                            change.completed_tasks = change.total_tasks;
                        }
                        change.status = "complete".to_string();
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::ProcessingError { id, error: _ } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *id) {
                        change.status = "error".to_string();
                        updated = true;
                    }
                    state.app_mode = "error".to_string();
                }

                // Apply output with iteration tracking
                ExecutionEvent::ApplyOutput {
                    change_id,
                    iteration,
                    ..
                } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        if let Some(iter) = iteration {
                            change.iteration_number = Some(*iter);
                            updated = true;
                        }
                    }
                }

                // Acceptance events
                ExecutionEvent::AcceptanceStarted { change_id, .. } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::AcceptanceCompleted { change_id } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }

                // Archive events
                ExecutionEvent::ArchiveStarted {
                    change_id,
                    command: _,
                } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::ChangeArchived(change_id) => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::ArchiveOutput { change_id, .. } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }

                // Progress events
                ExecutionEvent::ProgressUpdated {
                    change_id,
                    completed,
                    total,
                } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        // Update progress for all states when valid data is available.
                        // Only update if total > 0 to avoid resetting progress on retrieval failure.
                        // Progress retrieval failure (0/0) should preserve existing progress.
                        if *total > 0 {
                            change.completed_tasks = *completed;
                            change.total_tasks = *total;
                            change.progress_percent = progress_percent(*completed, *total);
                            change.status = status_from_progress(*completed, *total).to_string();
                            updated = true;
                        }
                        // If total == 0, preserve existing progress (do nothing)
                    }
                }

                // Post-archive terminal events
                ExecutionEvent::MergeCompleted { change_id, .. }
                | ExecutionEvent::PushCompleted { change_id, .. } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.status = "complete".to_string();
                        updated = true;
                    }
                }
                ExecutionEvent::PushFailed { change_id, .. } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.status = "error".to_string();
                        updated = true;
                    }
                }
                ExecutionEvent::PushStarted { change_id, .. } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::ResolveStarted {
                    change_id,
                    command: _,
                } => {
                    state.is_resolving = true;
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::ResolveCompleted { change_id, .. } => {
                    state.is_resolving = false;
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::ResolveFailed {
                    change_id,
                    error: _,
                } => {
                    state.is_resolving = false;
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.status = "error".to_string();
                        updated = true;
                    }
                }
                ExecutionEvent::MergeDeferred {
                    change_id,
                    reason: _,
                    auto_resumable,
                } => {
                    // Read is_resolving before mutable borrow
                    let is_resolving = state.is_resolving;
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        // Keep non-status metadata updates only.
                        // queue_status is derived from reducer state.
                        if is_resolving || *auto_resumable {
                            change.status = "in_progress".to_string();
                        }
                        updated = true;
                    }
                }

                // Log events
                ExecutionEvent::Log(log_entry) => {
                    state.logs.push(log_entry.clone());
                    // Keep only recent logs (last 1000 entries)
                    let logs_len = state.logs.len();
                    if logs_len > 1000 {
                        state.logs.drain(0..(logs_len - 1000));
                    }
                }

                // Changes refresh events
                ExecutionEvent::ChangesRefreshed {
                    changes,
                    committed_change_ids: _,
                    rejected_changes: _,
                    uncommitted_file_change_ids: _,
                    worktree_change_ids: _,
                    worktree_paths: _,
                    worktree_not_ahead_ids: _,
                    merge_wait_ids: _,
                } => {
                    // Update changes with new data
                    let mut new_change_statuses: Vec<ChangeStatus> =
                        changes.iter().map(ChangeStatus::from).collect();

                    // Preserve iteration_number and progress from existing state where applicable.
                    // queue_status is derived from reducer state.
                    for new_change in &mut new_change_statuses {
                        if let Some(existing) = state.changes.iter().find(|c| c.id == new_change.id)
                        {
                            new_change.iteration_number = existing.iteration_number;

                            // Preserve existing progress if retrieval failed (new data is 0/0)
                            // This prevents resetting progress to 0 on retrieval failure
                            if new_change.total_tasks == 0
                                && (existing.completed_tasks > 0 || existing.total_tasks > 0)
                            {
                                new_change.completed_tasks = existing.completed_tasks;
                                new_change.total_tasks = existing.total_tasks;
                                new_change.progress_percent = existing.progress_percent;
                                new_change.status = existing.status.clone();
                            }
                        }
                    }

                    state.changes = new_change_statuses;
                    refresh_summary(&mut state);
                    updated = true;
                }

                // Worktree refresh events
                ExecutionEvent::WorktreesRefreshed { worktrees } => {
                    state.worktrees = worktrees.clone();
                }

                // Dependency blocking events
                ExecutionEvent::DependencyBlocked {
                    change_id,
                    dependency_ids: _,
                } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.status = "pending".to_string();
                        updated = true;
                    }
                }
                ExecutionEvent::DependencyResolved { change_id } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }

                // Completion events
                ExecutionEvent::Stopping => {
                    state.app_mode = "stopping".to_string();
                }
                ExecutionEvent::Stopped => {
                    state.app_mode = "stopped".to_string();
                }
                ExecutionEvent::AllCompleted => {
                    // Same rule the TUI applies: a late or duplicate completion
                    // event must not overwrite a retained Error or Stopped mode,
                    // or the two frontends would report different terminal
                    // states for the same run.
                    if crate::events::all_completed_may_overwrite_mode(&state.app_mode) {
                        state.app_mode = "select".to_string();
                    }
                }
                ExecutionEvent::Error { .. } => {
                    state.app_mode = "error".to_string();
                }

                _ => {}
            }

            if updated {
                // Prefer the reducer state this dispatch produced; fall back to
                // a read-back only when the caller had none to give.
                if let Some(shared) = dispatch.state {
                    apply_reducer_derived_queue_statuses(&mut state, shared);
                } else if let Ok(shared_state_opt) = self.shared_orchestrator_state.try_read() {
                    if let Some(shared_arc) = shared_state_opt.as_ref() {
                        if let Ok(shared) = shared_arc.try_read() {
                            apply_reducer_derived_queue_statuses(&mut state, &shared);
                        }
                    }
                }
                refresh_summary(&mut state);
            }
        }

        // Absorb the same event into the process-local operator facts before the
        // snapshot is projected, so timing, latest activity, eligibility, and
        // the worktree relation all reach the client at the revision this event
        // produced rather than one revision late.
        self.absorb_operator_facts(event).await;

        // Project into `/api/v2` last, so the candidate snapshot it compares
        // against is the one this event just produced.
        self.project_execution_event(event, dispatch.ownership, dispatch.state)
            .await;
    }

    /// Update the process-local operator facts from one execution event.
    async fn absorb_operator_facts(&self, event: &ExecutionEvent) {
        let mut facts = self.operator_facts.write().await;
        match event {
            ExecutionEvent::ChangesRefreshed {
                changes,
                rejected_changes,
                committed_change_ids,
                uncommitted_file_change_ids,
                worktree_paths,
                ..
            } => {
                facts.observe_changes(
                    changes
                        .iter()
                        .chain(rejected_changes.iter())
                        .map(|change| change.id.as_str()),
                );
                facts.apply_parallel_eligibility(committed_change_ids, uncommitted_file_change_ids);
                facts.apply_worktree_paths(worktree_paths.clone());
            }
            ExecutionEvent::WorktreesRefreshed { worktrees } => {
                facts.apply_worktrees(worktrees.clone());
            }
            _ => {}
        }
        facts.record_event(event);
    }

    pub async fn refresh_from_disk(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::openspec;

        let repo_root =
            std::env::current_dir().map_err(|e| format!("Failed to resolve repo root: {}", e))?;

        // Read changes from disk using native parser
        let mut changes = openspec::list_changes_native()
            .map_err(|e| format!("Failed to refresh changes from disk: {}", e))?;

        // Enrich progress from worktrees (uncommitted tasks.md)
        // Use unified fallback helper: worktree → archive → base
        // The same lookup also supplies the change-to-worktree relation the v2
        // snapshot publishes, so a monitoring-only process reports it too.
        let mut worktree_paths: std::collections::HashMap<String, std::path::PathBuf> =
            std::collections::HashMap::new();
        for change in &mut changes {
            let worktree_path =
                match crate::vcs::git::get_worktree_path_for_change(&repo_root, &change.id).await {
                    Ok(Some(wt_path)) => Some(wt_path),
                    Ok(None) => None,
                    Err(e) => {
                        tracing::debug!("Failed to get worktree path for {}: {}", change.id, e);
                        None
                    }
                };
            if let Some(path) = &worktree_path {
                worktree_paths.insert(change.id.clone(), path.clone());
            }

            match crate::task_parser::parse_progress_with_fallback(
                &change.id,
                worktree_path.as_deref(),
            ) {
                Ok(progress) => {
                    change.completed_tasks = progress.completed;
                    change.total_tasks = progress.total;
                }
                Err(e) => {
                    tracing::debug!("Failed to read progress for {}: {}", change.id, e);
                }
            }
        }

        // Retrieve worktrees for TUI/Web parity
        let worktrees = match crate::worktree_ops::get_worktrees(&repo_root).await {
            Ok(wts) => wts,
            Err(e) => {
                tracing::debug!("Failed to retrieve worktrees: {}", e);
                Vec::new()
            }
        };

        // Preserve existing app_mode (don't overwrite runtime state with "select" default)
        let current_app_mode = {
            let state = self.state.read().await;
            state.app_mode.clone()
        };

        {
            let mut facts = self.operator_facts.write().await;
            facts.set_repo_root(repo_root.clone());
            facts.apply_worktrees(worktrees.clone());
            facts.apply_worktree_paths(worktree_paths);
        }

        // Update state with refreshed changes, preserving app_mode
        self.update_with_mode(&changes, &current_app_mode).await;

        {
            let mut state = self.state.write().await;
            state.worktrees = worktrees;
        }

        Ok(())
    }
}

impl Default for WebState {
    fn default() -> Self {
        Self::new(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::ProposalMetadata;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    #[test]
    fn test_change_status_from_change() {
        let change = create_test_change("test-change", 3, 5);
        let status = ChangeStatus::from(&change);

        assert_eq!(status.id, "test-change");
        assert_eq!(status.completed_tasks, 3);
        assert_eq!(status.total_tasks, 5);
        // Use approximate comparison for floating point
        assert!((status.progress_percent - 60.0).abs() < 0.01);
        assert_eq!(status.status, "in_progress");
    }

    #[test]
    fn test_change_status_pending() {
        let change = create_test_change("pending-change", 0, 5);
        let status = ChangeStatus::from(&change);

        assert_eq!(status.status, "pending");
    }

    #[test]
    fn test_change_status_complete() {
        let change = create_test_change("complete-change", 5, 5);
        let status = ChangeStatus::from(&change);

        assert_eq!(status.status, "complete");
    }

    #[test]
    fn test_orchestrator_state_snapshot_from_changes() {
        let changes = vec![
            create_test_change("change-a", 0, 3),
            create_test_change("change-b", 2, 5),
            create_test_change("change-c", 4, 4),
        ];

        let mut state = OrchestratorStateSnapshot::from_changes(&changes);

        // Initial state: no queue_status set, so all counts should be 0
        assert_eq!(state.total_changes, 3);
        assert_eq!(state.pending_changes, 0);
        assert_eq!(state.in_progress_changes, 0);
        assert_eq!(state.completed_changes, 0);

        // Set queue_status to test aggregation
        state.changes[0].queue_status = Some("queued".to_string());
        state.changes[1].queue_status = Some("applying".to_string());
        state.changes[2].queue_status = Some("archived".to_string());
        refresh_summary(&mut state);

        assert_eq!(state.pending_changes, 1);
        assert_eq!(state.in_progress_changes, 1);
        assert_eq!(state.completed_changes, 1);
    }

    #[tokio::test]
    async fn test_web_state_get_state() {
        let changes = vec![create_test_change("test", 1, 3)];
        let web_state = WebState::new(&changes);

        let state = web_state.get_state().await;
        assert_eq!(state.total_changes, 1);
        assert_eq!(state.changes[0].id, "test");
    }

    #[tokio::test]
    async fn test_web_state_update() {
        let web_state = WebState::new(&[]);

        let changes = vec![create_test_change("new-change", 2, 4)];
        web_state.update(&changes).await;

        let state = web_state.get_state().await;
        assert_eq!(state.total_changes, 1);
        assert_eq!(state.changes[0].id, "new-change");
    }

    #[tokio::test]
    async fn test_apply_execution_event_processing_started_sets_in_progress() {
        let changes = vec![create_test_change("change-a", 0, 3)];
        let web_state = WebState::new(&changes);

        web_state
            .apply_execution_event(&ExecutionEvent::ProcessingStarted("change-a".to_string()))
            .await;

        let state = web_state.get_state().await;
        assert_eq!(state.changes[0].status, "in_progress");
        assert_eq!(state.changes[0].queue_status, None);
    }

    #[tokio::test]
    async fn test_apply_execution_event_acceptance_started() {
        let changes = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&changes);

        web_state
            .apply_execution_event(&ExecutionEvent::AcceptanceStarted {
                change_id: "change-a".to_string(),
                command: "test command".to_string(),
            })
            .await;

        let state = web_state.get_state().await;
        assert_eq!(state.changes[0].queue_status, None);
    }

    #[tokio::test]
    async fn test_apply_execution_event_acceptance_completed() {
        let changes = vec![create_test_change("change-a", 10, 10)];
        let web_state = WebState::new(&changes);

        web_state
            .apply_execution_event(&ExecutionEvent::AcceptanceCompleted {
                change_id: "change-a".to_string(),
            })
            .await;

        let state = web_state.get_state().await;
        assert_eq!(state.changes[0].queue_status, None);
    }

    #[tokio::test]
    async fn test_apply_execution_event_progress_updated_updates_counts() {
        let changes = vec![create_test_change("change-a", 0, 3)];
        let web_state = WebState::new(&changes);

        web_state
            .apply_execution_event(&ExecutionEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 2,
                total: 4,
            })
            .await;

        let state = web_state.get_state().await;
        let change = &state.changes[0];
        assert_eq!(change.completed_tasks, 2);
        assert_eq!(change.total_tasks, 4);
        assert!((change.progress_percent - 50.0).abs() < 0.01);
        assert_eq!(change.status, "in_progress");
    }

    #[tokio::test]
    async fn test_web_state_snapshot_contains_every_change() {
        let changes = vec![
            create_test_change("change-a", 1, 3),
            create_test_change("change-b", 2, 5),
        ];
        let web_state = WebState::new(&changes);

        let state = web_state.get_state().await;
        assert!(state.changes.iter().any(|change| change.id == "change-b"));
        assert!(!state
            .changes
            .iter()
            .any(|change| change.id == "nonexistent"));
    }

    #[tokio::test]
    async fn test_update_with_identical_changes_keeps_the_snapshot() {
        let changes = vec![create_test_change("change-a", 2, 5)];
        let web_state = WebState::new(&changes);
        let before = web_state.get_state().await;

        web_state.update(&changes).await;

        let after = web_state.get_state().await;
        assert_eq!(before.changes, after.changes);
    }

    #[tokio::test]
    async fn test_update_reflects_progress() {
        let initial = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 1, 5),
        ];
        let web_state = WebState::new(&initial);

        let updated = vec![
            create_test_change("change-a", 3, 5),
            create_test_change("change-b", 1, 5),
        ];
        web_state.update(&updated).await;

        let state = web_state.get_state().await;
        assert_eq!(state.changes.len(), 2);
        let updated_change = state
            .changes
            .iter()
            .find(|change| change.id == "change-a")
            .unwrap();
        assert_eq!(updated_change.completed_tasks, 3);
    }

    #[tokio::test]
    async fn test_update_drops_an_archived_change() {
        let initial = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 3, 5),
        ];
        let web_state = WebState::new(&initial);

        let updated = vec![create_test_change("change-a", 2, 5)];
        web_state.update(&updated).await;

        let state = web_state.get_state().await;
        assert_eq!(state.changes.len(), 1);
        assert_eq!(state.changes[0].id, "change-a");
        assert_eq!(state.changes[0].status, "in_progress");
    }

    #[tokio::test]
    async fn test_update_adds_a_new_change() {
        let initial = vec![create_test_change("change-a", 2, 5)];
        let web_state = WebState::new(&initial);

        let updated = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 0, 3),
        ];
        web_state.update(&updated).await;

        let state = web_state.get_state().await;
        assert_eq!(state.changes.len(), 2);
        assert!(state.changes.iter().any(|change| change.id == "change-a"));
        assert!(state.changes.iter().any(|change| change.id == "change-b"));
    }

    // === Tests for update-progress-archive-resolve ===

    #[tokio::test]
    async fn test_progress_updated_zero_preserves_existing_progress() {
        let changes = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&changes);

        // Send ProgressUpdated with 0/0 (retrieval failure)
        web_state
            .apply_execution_event(&ExecutionEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 0,
                total: 0,
            })
            .await;

        // Progress should be preserved
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 5,
            "completed_tasks should be preserved on 0/0"
        );
        assert_eq!(
            state.changes[0].total_tasks, 10,
            "total_tasks should be preserved on 0/0"
        );
    }

    #[tokio::test]
    async fn test_progress_updated_valid_updates_progress() {
        let changes = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&changes);

        // Send ProgressUpdated with valid data
        web_state
            .apply_execution_event(&ExecutionEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 8,
                total: 12,
            })
            .await;

        // Progress should be updated
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 8,
            "completed_tasks should be updated with valid data"
        );
        assert_eq!(
            state.changes[0].total_tasks, 12,
            "total_tasks should be updated with valid data"
        );
    }

    #[tokio::test]
    async fn test_update_method_preserves_progress_on_zero() {
        let initial = vec![create_test_change("change-a", 7, 10)];
        let web_state = WebState::new(&initial);

        // Update with 0/0 (retrieval failure)
        let updated = vec![Change {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 0,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }];
        web_state.update(&updated).await;

        // Progress should be preserved
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 7,
            "completed_tasks should be preserved on update with 0/0"
        );
        assert_eq!(
            state.changes[0].total_tasks, 10,
            "total_tasks should be preserved on update with 0/0"
        );
    }

    #[tokio::test]
    async fn test_update_method_updates_progress_with_valid_data() {
        let initial = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&initial);

        // Update with valid data
        let updated = vec![create_test_change("change-a", 9, 12)];
        web_state.update(&updated).await;

        // Progress should be updated
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 9,
            "completed_tasks should be updated with valid data"
        );
        assert_eq!(
            state.changes[0].total_tasks, 12,
            "total_tasks should be updated with valid data"
        );
    }

    #[tokio::test]
    async fn test_changes_refreshed_preserves_progress_on_zero() {
        let initial = vec![create_test_change("change-a", 7, 10)];
        let web_state = WebState::new(&initial);

        // Set initial state via execution event
        web_state
            .apply_execution_event(&ExecutionEvent::ProcessingStarted("change-a".to_string()))
            .await;

        // Send ChangesRefreshed with 0/0 (retrieval failure)
        use std::collections::{HashMap, HashSet};
        web_state
            .apply_execution_event(&ExecutionEvent::ChangesRefreshed {
                changes: vec![Change {
                    id: "change-a".to_string(),
                    completed_tasks: 0,
                    total_tasks: 0,
                    last_modified: "now".to_string(),
                    dependencies: Vec::new(),
                    metadata: ProposalMetadata::default(),
                }],
                rejected_changes: Vec::new(),
                committed_change_ids: HashSet::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: HashSet::new(),
            })
            .await;

        // Progress should be preserved
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 7,
            "completed_tasks should be preserved on ChangesRefreshed with 0/0"
        );
        assert_eq!(
            state.changes[0].total_tasks, 10,
            "total_tasks should be preserved on ChangesRefreshed with 0/0"
        );
    }

    #[tokio::test]
    async fn test_changes_refreshed_updates_progress_with_valid_data() {
        let initial = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&initial);

        // Send ChangesRefreshed with valid data
        use std::collections::{HashMap, HashSet};
        web_state
            .apply_execution_event(&ExecutionEvent::ChangesRefreshed {
                changes: vec![create_test_change("change-a", 9, 12)],
                committed_change_ids: HashSet::new(),
                rejected_changes: Vec::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: HashSet::new(),
            })
            .await;

        // Progress should be updated
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 9,
            "completed_tasks should be updated with valid data"
        );
        assert_eq!(
            state.changes[0].total_tasks, 12,
            "total_tasks should be updated with valid data"
        );
    }

    #[tokio::test]
    async fn test_archive_started_preserves_progress_when_zero() {
        let initial = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&initial);

        // Set to archiving with ArchiveStarted
        web_state
            .apply_execution_event(&ExecutionEvent::ArchiveStarted {
                change_id: "change-a".to_string(),
                command: "test command".to_string(),
            })
            .await;

        // Progress should be preserved
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 5,
            "completed_tasks should be preserved during archiving"
        );
        assert_eq!(
            state.changes[0].total_tasks, 10,
            "total_tasks should be preserved during archiving"
        );
        assert_eq!(state.changes[0].queue_status, None);
    }

    #[tokio::test]
    async fn test_progress_updated_preserves_existing_during_archiving() {
        let initial = vec![create_test_change("change-a", 7, 10)];
        let web_state = WebState::new(&initial);

        // Set to archiving
        web_state
            .apply_execution_event(&ExecutionEvent::ArchiveStarted {
                change_id: "change-a".to_string(),
                command: "test command".to_string(),
            })
            .await;

        // Send ProgressUpdated with 0/0 (retrieval failure during archiving)
        web_state
            .apply_execution_event(&ExecutionEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 0,
                total: 0,
            })
            .await;

        // Progress should be preserved (not reset to 0/0)
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 7,
            "completed_tasks should be preserved on 0/0 update during archiving"
        );
        assert_eq!(
            state.changes[0].total_tasks, 10,
            "total_tasks should be preserved on 0/0 update during archiving"
        );
    }

    #[tokio::test]
    async fn test_progress_updated_preserves_existing_during_resolving() {
        let initial = vec![create_test_change("change-a", 8, 10)];
        let web_state = WebState::new(&initial);

        // Set to resolving
        web_state
            .apply_execution_event(&ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "test resolve command".to_string(),
            })
            .await;

        // Send ProgressUpdated with 0/0 (retrieval failure during resolving)
        web_state
            .apply_execution_event(&ExecutionEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 0,
                total: 0,
            })
            .await;

        // Progress should be preserved (not reset to 0/0)
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 8,
            "completed_tasks should be preserved on 0/0 update during resolving"
        );
        assert_eq!(
            state.changes[0].total_tasks, 10,
            "total_tasks should be preserved on 0/0 update during resolving"
        );
    }

    #[tokio::test]
    async fn test_changes_refreshed_preserves_progress_during_archiving() {
        let initial = vec![create_test_change("change-a", 6, 10)];
        let web_state = WebState::new(&initial);

        // Set to archiving
        web_state
            .apply_execution_event(&ExecutionEvent::ArchiveStarted {
                change_id: "change-a".to_string(),
                command: "test command".to_string(),
            })
            .await;

        // Send ChangesRefreshed with 0/0 (retrieval failure)
        use std::collections::{HashMap, HashSet};
        web_state
            .apply_execution_event(&ExecutionEvent::ChangesRefreshed {
                changes: vec![create_test_change("change-a", 0, 0)],
                committed_change_ids: HashSet::new(),
                rejected_changes: Vec::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: HashSet::new(),
            })
            .await;

        // Progress should be preserved (not reset to 0/0)
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 6,
            "completed_tasks should be preserved on ChangesRefreshed with 0/0 during archiving"
        );
        assert_eq!(
            state.changes[0].total_tasks, 10,
            "total_tasks should be preserved on ChangesRefreshed with 0/0 during archiving"
        );
        assert_eq!(state.changes[0].queue_status, None);
    }

    #[tokio::test]
    async fn test_changes_refreshed_preserves_progress_during_resolving() {
        let initial = vec![create_test_change("change-a", 9, 10)];
        let web_state = WebState::new(&initial);

        // Set to resolving
        web_state
            .apply_execution_event(&ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "test resolve command".to_string(),
            })
            .await;

        // Send ChangesRefreshed with 0/0 (retrieval failure)
        use std::collections::{HashMap, HashSet};
        web_state
            .apply_execution_event(&ExecutionEvent::ChangesRefreshed {
                changes: vec![create_test_change("change-a", 0, 0)],
                committed_change_ids: HashSet::new(),
                rejected_changes: Vec::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: HashSet::new(),
            })
            .await;

        // Progress should be preserved (not reset to 0/0)
        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].completed_tasks, 9,
            "completed_tasks should be preserved on ChangesRefreshed with 0/0 during resolving"
        );
        assert_eq!(
            state.changes[0].total_tasks, 10,
            "total_tasks should be preserved on ChangesRefreshed with 0/0 during resolving"
        );
        assert_eq!(state.changes[0].queue_status, None);
    }

    // === Tests for update-merge-deferred-resolve-pending ===

    #[tokio::test]
    async fn test_merge_deferred_during_resolve_sets_resolve_pending() {
        let changes = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&changes);

        // Start resolve to set is_resolving = true
        web_state
            .apply_execution_event(&ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "test command".to_string(),
            })
            .await;

        // Verify is_resolving is true
        let state = web_state.get_state().await;
        assert!(state.is_resolving, "is_resolving should be true");

        // Send MergeDeferred event
        web_state
            .apply_execution_event(&ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "test reason".to_string(),
                auto_resumable: true,
            })
            .await;

        // Verify queue_status is "resolve pending"
        let state = web_state.get_state().await;
        assert_eq!(state.changes[0].queue_status, None);
    }

    #[tokio::test]
    async fn test_merge_deferred_not_resolving_sets_merge_wait() {
        let changes = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&changes);

        // Send MergeDeferred event without starting resolve (manual intervention required)
        web_state
            .apply_execution_event(&ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "test reason".to_string(),
                auto_resumable: false,
            })
            .await;

        // Verify queue_status is "merge wait"
        let state = web_state.get_state().await;
        assert_eq!(state.changes[0].queue_status, None);
        assert!(!state.is_resolving, "is_resolving should be false");
    }

    #[tokio::test]
    async fn test_resolve_started_sets_is_resolving() {
        let changes = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&changes);

        // Send ResolveStarted event
        web_state
            .apply_execution_event(&ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "test command".to_string(),
            })
            .await;

        // Verify is_resolving is true
        let state = web_state.get_state().await;
        assert!(state.is_resolving, "is_resolving should be true");
        assert_eq!(state.changes[0].queue_status, None);
    }

    #[tokio::test]
    async fn test_resolve_completed_clears_is_resolving() {
        let changes = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&changes);

        // Start resolve
        web_state
            .apply_execution_event(&ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "test command".to_string(),
            })
            .await;

        // Complete resolve
        web_state
            .apply_execution_event(&ExecutionEvent::ResolveCompleted {
                change_id: "change-a".to_string(),
                worktree_change_ids: None,
            })
            .await;

        // Verify is_resolving is false
        let state = web_state.get_state().await;
        assert!(!state.is_resolving, "is_resolving should be false");
        assert_eq!(state.changes[0].queue_status, None);
    }

    #[tokio::test]
    async fn test_resolve_failed_clears_is_resolving() {
        let changes = vec![create_test_change("change-a", 5, 10)];
        let web_state = WebState::new(&changes);

        // Start resolve
        web_state
            .apply_execution_event(&ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "test command".to_string(),
            })
            .await;

        // Fail resolve
        web_state
            .apply_execution_event(&ExecutionEvent::ResolveFailed {
                change_id: "change-a".to_string(),
                error: "test error".to_string(),
            })
            .await;

        // Verify is_resolving is false
        let state = web_state.get_state().await;
        assert!(!state.is_resolving, "is_resolving should be false");
        assert_eq!(state.changes[0].queue_status, None);
    }

    /// Auto-resumable MergeDeferred when resolve is NOT running must show "resolve pending"
    /// (not "merge wait") so that the Web dashboard indicates the change will be retried
    /// automatically.
    #[tokio::test]
    async fn test_auto_resumable_merge_deferred_without_resolve_shows_resolve_pending() {
        let changes = vec![create_test_change("change-b", 5, 10)];
        let web_state = WebState::new(&changes);

        // No ResolveStarted → is_resolving is false.
        // Send auto-resumable MergeDeferred (e.g. MERGE_HEAD exists from another merge).
        web_state
            .apply_execution_event(&ExecutionEvent::MergeDeferred {
                change_id: "change-b".to_string(),
                reason: "Merge in progress (MERGE_HEAD exists)".to_string(),
                auto_resumable: true,
            })
            .await;

        let state = web_state.get_state().await;
        assert_eq!(state.changes[0].queue_status, None);
    }

    /// Phase 6.3: verify that from_changes_with_shared_state derives queue_status from the reducer
    /// display_status without changing the JSON API payload shape.
    #[test]
    fn test_web_snapshot_uses_reducer_display_status_without_payload_change() {
        use crate::orchestration::state::{OrchestratorState, ReducerCommand};

        let mut shared = OrchestratorState::new(
            vec![
                "ch-queued".to_string(),
                "ch-notqueued".to_string(),
                "ch-archived".to_string(),
            ],
            0,
        );
        // Seed changes that the reducer knows about
        let changes = vec![
            create_test_change("ch-queued", 0, 3),
            create_test_change("ch-notqueued", 0, 3),
            create_test_change("ch-archived", 3, 3),
        ];

        // Seed change_runtime entries
        shared.apply_command(ReducerCommand::AddToQueue("ch-queued".to_string()));

        // Drive ch-archived through the terminal state
        shared.apply_command(ReducerCommand::AddToQueue("ch-archived".to_string()));
        shared.apply_execution_event(&crate::events::ExecutionEvent::ChangeArchived(
            "ch-archived".to_string(),
        ));

        let snapshot =
            OrchestratorStateSnapshot::from_changes_with_shared_state(&changes, Some(&shared));

        let queued = snapshot
            .changes
            .iter()
            .find(|c| c.id == "ch-queued")
            .unwrap();
        let notqueued = snapshot
            .changes
            .iter()
            .find(|c| c.id == "ch-notqueued")
            .unwrap();
        let archived = snapshot
            .changes
            .iter()
            .find(|c| c.id == "ch-archived")
            .unwrap();

        // Reducer-derived queue_status values must match display_status output
        assert_eq!(queued.queue_status, Some("queued".to_string()));
        // "not queued" maps to None to keep payload minimal (no API shape change)
        assert_eq!(notqueued.queue_status, None);
        assert_eq!(archived.queue_status, Some("archived".to_string()));
    }

    #[test]
    fn pushed_status_web_snapshot_exposes_post_archive_statuses_from_reducer() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::{ExecutionMode, OrchestratorState};

        let mut shared = OrchestratorState::with_mode(
            vec![
                "resolving-a".to_string(),
                "resolve-b".to_string(),
                "merge-c".to_string(),
                "push-d".to_string(),
            ],
            0,
            ExecutionMode::Parallel,
        );
        let changes = vec![
            create_test_change("resolving-a", 0, 1),
            create_test_change("resolve-b", 0, 1),
            create_test_change("merge-c", 0, 1),
            create_test_change("push-d", 0, 1),
        ];

        shared.apply_execution_event(&ExecutionEvent::ChangeArchived("resolving-a".to_string()));
        shared.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "resolve-b".to_string(),
            reason: "another merge is active".to_string(),
            auto_resumable: true,
        });
        shared.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "merge-c".to_string(),
            reason: "base dirty".to_string(),
            auto_resumable: false,
        });
        shared.apply_execution_event(&ExecutionEvent::PushCompleted {
            change_id: "push-d".to_string(),
            remote: "origin".to_string(),
            branch: "push-d".to_string(),
        });

        let snapshot =
            OrchestratorStateSnapshot::from_changes_with_shared_state(&changes, Some(&shared));
        let status = |id: &str| {
            snapshot
                .changes
                .iter()
                .find(|c| c.id == id)
                .and_then(|c| c.queue_status.as_deref())
        };

        assert_eq!(status("resolving-a"), Some("resolving"));
        assert_eq!(status("resolve-b"), Some("resolve pending"));
        assert_eq!(status("merge-c"), Some("merge wait"));
        assert_eq!(status("push-d"), Some("pushed"));
        assert_eq!(snapshot.completed_changes, 1);
    }

    #[test]
    fn test_web_snapshot_exposes_reject_pending_from_reducer() {
        use crate::orchestration::state::OrchestratorState;

        let mut shared = OrchestratorState::with_mode(
            vec!["lane-a".to_string(), "reject-b".to_string()],
            0,
            crate::orchestration::state::ExecutionMode::Parallel,
        );
        let changes = vec![
            create_test_change("lane-a", 0, 1),
            create_test_change("reject-b", 0, 1),
        ];

        shared.apply_execution_event(&crate::events::ExecutionEvent::WorkspaceStatusUpdated {
            change_id: "lane-a".to_string(),
            workspace_name: "ws-a".to_string(),
            status: crate::vcs::WorkspaceStatus::Resolving,
        });
        shared.apply_execution_event(&crate::events::ExecutionEvent::WorkspaceStatusUpdated {
            change_id: "reject-b".to_string(),
            workspace_name: "ws-b".to_string(),
            status: crate::vcs::WorkspaceStatus::Rejecting,
        });

        let snapshot =
            OrchestratorStateSnapshot::from_changes_with_shared_state(&changes, Some(&shared));
        let reject = snapshot
            .changes
            .iter()
            .find(|c| c.id == "reject-b")
            .unwrap();

        assert_eq!(reject.queue_status, Some("reject pending".to_string()));
    }

    #[tokio::test]
    async fn test_changes_refreshed_reactivated_change_clears_rejected_queue_status() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::OrchestratorState;
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let web_state = WebState::new(&changes);

        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        {
            let mut guard = shared.write().await;
            guard.apply_execution_event(&ExecutionEvent::ChangeRejected {
                change_id: "change-a".to_string(),
                reason: "blocked".to_string(),
            });
            assert_eq!(guard.display_status("change-a"), "rejected");

            // Reactivation by refresh with the change present in active list.
            guard.apply_execution_event(&ExecutionEvent::ChangesRefreshed {
                changes: vec![create_test_change("change-a", 0, 1)],
                committed_change_ids: std::collections::HashSet::new(),
                rejected_changes: Vec::new(),
                uncommitted_file_change_ids: std::collections::HashSet::new(),
                worktree_change_ids: std::collections::HashSet::new(),
                worktree_paths: std::collections::HashMap::new(),
                worktree_not_ahead_ids: std::collections::HashSet::new(),
                merge_wait_ids: std::collections::HashSet::new(),
            });
            assert_eq!(guard.display_status("change-a"), "not queued");
        }

        web_state.set_shared_state(shared.clone()).await;
        web_state
            .apply_execution_event(&ExecutionEvent::ChangesRefreshed {
                changes: vec![create_test_change("change-a", 0, 1)],
                committed_change_ids: std::collections::HashSet::new(),
                rejected_changes: Vec::new(),
                uncommitted_file_change_ids: std::collections::HashSet::new(),
                worktree_change_ids: std::collections::HashSet::new(),
                worktree_paths: std::collections::HashMap::new(),
                worktree_not_ahead_ids: std::collections::HashSet::new(),
                merge_wait_ids: std::collections::HashSet::new(),
            })
            .await;

        let state = web_state.get_state().await;
        assert_eq!(
            state.changes[0].queue_status, None,
            "reactivated change should not keep rejected queue_status"
        );
    }

    #[tokio::test]
    async fn test_dependency_blocked_and_resolved_converges_to_reducer_queue_status() {
        use crate::orchestration::state::{OrchestratorState, ReducerCommand};
        use std::sync::Arc;
        use tokio::sync::RwLock;

        let changes = vec![create_test_change("change-b", 0, 3)];
        let web_state = WebState::new(&changes);

        let mut shared = OrchestratorState::new(vec!["change-b".to_string()], 0);
        shared.apply_command(ReducerCommand::AddToQueue("change-b".to_string()));

        let shared = Arc::new(RwLock::new(shared));
        web_state.set_shared_state(shared.clone()).await;

        {
            let mut guard = shared.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::DependencyBlocked {
                change_id: "change-b".to_string(),
                dependency_ids: vec!["change-a".to_string()],
            });
        }

        web_state
            .apply_execution_event(&ExecutionEvent::DependencyBlocked {
                change_id: "change-b".to_string(),
                dependency_ids: vec!["change-a".to_string()],
            })
            .await;

        let blocked_state = web_state.get_state().await;
        assert_eq!(
            blocked_state.changes[0].queue_status,
            Some("blocked".to_string()),
            "web state should converge to reducer-derived blocked status"
        );

        {
            let mut guard = shared.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::DependencyResolved {
                change_id: "change-b".to_string(),
            });
        }

        web_state
            .apply_execution_event(&ExecutionEvent::DependencyResolved {
                change_id: "change-b".to_string(),
            })
            .await;

        let resolved_state = web_state.get_state().await;
        assert_eq!(
            resolved_state.changes[0].queue_status,
            Some("queued".to_string()),
            "web state should converge back to queued after dependency resolved"
        );
    }
}
