//! Web monitoring state management.
//!
//! Provides thread-safe state access and broadcasting for WebSocket clients.

use crate::events::{EventSink, ExecutionEvent, LogEntry};
use crate::openspec::Change;
use crate::tui::types::WorktreeInfo;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

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
    /// Retry error changes
    Retry,
}

/// State update message sent to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct StateUpdate {
    /// Type of update message
    #[serde(rename = "type")]
    pub msg_type: String,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// List of changes with current status
    pub changes: Vec<ChangeStatus>,
    /// Log entries (optional, sent with log events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<LogEntry>>,
    /// Worktree list (optional, sent with worktree refresh events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktrees: Option<Vec<WorktreeInfo>>,
    /// Application mode (optional, sent with mode change events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_mode: Option<String>,
}

/// Change status for WebSocket updates
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// Aligned with TUI display status values: "not queued", "queued", "blocked", "processing",
    /// "accepting", "archiving", "archived", "merged", "merge wait", "resolving", "resolve pending", "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_status: Option<String>,
    /// Current iteration number for apply/archive loops
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_number: Option<u32>,
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
                    .is_some_and(|s| s == "archived" || s == "merged")
            })
            .count();
        let in_progress = change_statuses
            .iter()
            .filter(|c| {
                c.queue_status.as_ref().is_some_and(|s| {
                    s == "applying" || s == "accepting" || s == "archiving" || s == "resolving"
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
}

/// Shared web state with broadcast channel for updates
pub struct WebState {
    /// Current orchestrator state snapshot (thread-safe)
    state: RwLock<OrchestratorStateSnapshot>,
    /// Broadcast channel for state updates
    tx: broadcast::Sender<StateUpdate>,
    /// Control command channel (optional, only used when web control is enabled)
    /// Uses Mutex for interior mutability to allow setting after Arc creation
    control_tx: Mutex<Option<mpsc::UnboundedSender<ControlCommand>>>,
    /// Reference to shared orchestration state (for unified state tracking)
    /// Wrapped in RwLock for interior mutability (can be set after construction via Arc)
    shared_orchestrator_state: tokio::sync::RwLock<
        Option<std::sync::Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>>,
    >,
}

impl WebState {
    /// Create a new WebState with initial changes
    pub fn new(initial_changes: &[Change]) -> Self {
        let (tx, _) = broadcast::channel(100);
        let state = OrchestratorStateSnapshot::from_changes(initial_changes);

        Self {
            state: RwLock::new(state),
            tx,
            control_tx: Mutex::new(None),
            shared_orchestrator_state: tokio::sync::RwLock::new(None),
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

        // Check if state has actually changed
        let has_changes = !self
            .compute_diff(&old_changes, &new_state.changes)
            .is_empty();

        // Update internal state
        {
            let mut state = self.state.write().await;
            *state = new_state.clone();
        }

        // Only broadcast if there were changes
        if has_changes {
            self.broadcast_snapshot(new_state.changes).await;
        }
    }

    /// Update the state with new changes and explicit app_mode (for Run mode)
    pub async fn update_with_mode(&self, changes: &[Change], app_mode: &str) {
        let mut new_state = OrchestratorStateSnapshot::from_changes(changes);

        // Override app_mode from orchestrator execution state
        new_state.app_mode = app_mode.to_string();

        // Preserve progress, queue_status, and is_resolving from existing state
        let (old_changes, old_app_mode, old_is_resolving) = {
            let old_state = self.state.read().await;
            (
                old_state.changes.clone(),
                old_state.app_mode.clone(),
                old_state.is_resolving,
            )
        };

        // Preserve is_resolving to prevent overwriting runtime state
        new_state.is_resolving = old_is_resolving;

        for new_change in &mut new_state.changes {
            if let Some(existing) = old_changes.iter().find(|c| c.id == new_change.id) {
                // Preserve queue_status
                new_change.queue_status = existing.queue_status.clone();

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

        // Check if state has actually changed (changes OR app_mode)
        let has_changes = !self
            .compute_diff(&old_changes, &new_state.changes)
            .is_empty();
        let app_mode_changed = new_state.app_mode != old_app_mode;

        // Update internal state
        {
            let mut state = self.state.write().await;
            *state = new_state.clone();
        }

        // Broadcast if there were changes OR if app_mode changed
        if has_changes || app_mode_changed {
            self.broadcast_snapshot(new_state.changes).await;
        }
    }

    /// Apply an execution event to the web state and broadcast updates.
    pub async fn apply_execution_event(&self, event: &ExecutionEvent) {
        let mut broadcast_update = None;

        {
            let mut state = self.state.write().await;
            let mut updated = false;
            let mut log_broadcast = None;
            let mut worktree_broadcast = None;
            let mut mode_broadcast = None;

            match event {
                // Lifecycle events
                ExecutionEvent::ProcessingStarted(change_id) => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.status = "in_progress".to_string();
                        change.queue_status = Some("applying".to_string());
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                    state.app_mode = "running".to_string();
                    mode_broadcast = Some("running".to_string());
                }
                ExecutionEvent::ProcessingCompleted(change_id) => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        if change.completed_tasks < change.total_tasks {
                            change.completed_tasks = change.total_tasks;
                        }
                        change.status = "complete".to_string();
                        change.queue_status = Some("archiving".to_string());
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::ProcessingError { id, error: _ } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *id) {
                        change.status = "error".to_string();
                        change.queue_status = Some("error".to_string());
                        updated = true;
                    }
                    state.app_mode = "error".to_string();
                    mode_broadcast = Some("error".to_string());
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
                        change.queue_status = Some("accepting".to_string());
                        updated = true;
                    }
                }
                ExecutionEvent::AcceptanceCompleted { change_id } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.queue_status = Some("archiving".to_string());
                        updated = true;
                    }
                }

                // Archive events
                ExecutionEvent::ArchiveStarted {
                    change_id,
                    command: _,
                } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.queue_status = Some("archiving".to_string());
                        updated = true;
                    }
                }
                ExecutionEvent::ChangeArchived(change_id) => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.status = "archived".to_string();
                        change.queue_status = Some("archived".to_string());
                        updated = true;
                    }
                }
                ExecutionEvent::ArchiveOutput {
                    change_id,
                    iteration,
                    ..
                } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.iteration_number = Some(*iteration);
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

                // Merge events
                ExecutionEvent::MergeCompleted { change_id, .. } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.queue_status = Some("merged".to_string());
                        updated = true;
                    }
                }
                ExecutionEvent::ResolveStarted {
                    change_id,
                    command: _,
                } => {
                    state.is_resolving = true;
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.queue_status = Some("resolving".to_string());
                        updated = true;
                    }
                }
                ExecutionEvent::ResolveCompleted { change_id, .. } => {
                    state.is_resolving = false;
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.queue_status = Some("archiving".to_string());
                        updated = true;
                    }
                }
                ExecutionEvent::ResolveFailed {
                    change_id,
                    error: _,
                } => {
                    state.is_resolving = false;
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.queue_status = Some("error".to_string());
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
                        // If resolve is running, or the deferral is auto-resumable,
                        // transition to resolve pending; otherwise maintain merge wait.
                        change.queue_status = if is_resolving || *auto_resumable {
                            Some("resolve pending".to_string())
                        } else {
                            Some("merge wait".to_string())
                        };
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
                    log_broadcast = Some(vec![log_entry.clone()]);
                }

                // Changes refresh events
                ExecutionEvent::ChangesRefreshed {
                    changes,
                    committed_change_ids: _,
                    uncommitted_file_change_ids: _,
                    worktree_change_ids: _,
                    worktree_paths: _,
                    worktree_not_ahead_ids: _,
                    merge_wait_ids: _,
                } => {
                    // Update changes with new data
                    let mut new_change_statuses: Vec<ChangeStatus> =
                        changes.iter().map(ChangeStatus::from).collect();

                    // Preserve queue_status, iteration_number, and progress from existing state where applicable
                    for new_change in &mut new_change_statuses {
                        if let Some(existing) = state.changes.iter().find(|c| c.id == new_change.id)
                        {
                            new_change.queue_status = existing.queue_status.clone();
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
                    worktree_broadcast = Some(worktrees.clone());
                }

                // Dependency blocking events
                ExecutionEvent::DependencyBlocked {
                    change_id,
                    dependency_ids: _,
                } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.queue_status = Some("blocked".to_string());
                        updated = true;
                    }
                }
                ExecutionEvent::DependencyResolved { change_id } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.queue_status = Some("queued".to_string());
                        updated = true;
                    }
                }

                // Completion events
                ExecutionEvent::Stopping => {
                    state.app_mode = "stopping".to_string();
                    mode_broadcast = Some("stopping".to_string());
                }
                ExecutionEvent::Stopped => {
                    state.app_mode = "stopped".to_string();
                    mode_broadcast = Some("stopped".to_string());
                }
                ExecutionEvent::AllCompleted => {
                    state.app_mode = "select".to_string();
                    mode_broadcast = Some("select".to_string());
                }
                ExecutionEvent::Error { message } => {
                    state.app_mode = "error".to_string();
                    mode_broadcast = Some("error".to_string());
                    log_broadcast = Some(vec![LogEntry::error(message.clone())]);
                }

                _ => {}
            }

            if updated {
                refresh_summary(&mut state);
            }

            // Prepare broadcast message
            if updated
                || log_broadcast.is_some()
                || worktree_broadcast.is_some()
                || mode_broadcast.is_some()
            {
                broadcast_update = Some(StateUpdate {
                    msg_type: "state_update".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    changes: state.changes.clone(),
                    logs: log_broadcast,
                    worktrees: worktree_broadcast,
                    app_mode: mode_broadcast,
                });
            }
        }

        // Broadcast outside the lock
        if let Some(update) = broadcast_update {
            let _ = self.tx.send(update);
        }
    }

    async fn broadcast_snapshot(&self, changes: Vec<ChangeStatus>) {
        // Read current app_mode and worktrees from state
        let (current_app_mode, current_worktrees) = {
            let state = self.state.read().await;
            (state.app_mode.clone(), state.worktrees.clone())
        };

        let update = StateUpdate {
            msg_type: "state_update".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            changes,
            logs: None,
            worktrees: Some(current_worktrees),
            app_mode: Some(current_app_mode),
        };

        let _ = self.tx.send(update);
    }

    /// Compute the diff between old and new change lists.
    /// Returns only changes that are new or modified.
    fn compute_diff(&self, old: &[ChangeStatus], new: &[ChangeStatus]) -> Vec<ChangeStatus> {
        let mut diff = Vec::new();

        for new_change in new {
            // Check if this change existed before
            let old_change = old.iter().find(|c| c.id == new_change.id);

            match old_change {
                Some(old) if old != new_change => {
                    // Change was modified
                    diff.push(new_change.clone());
                }
                None => {
                    // New change
                    diff.push(new_change.clone());
                }
                _ => {
                    // No change
                }
            }
        }

        // Also detect removed changes (archived)
        for old_change in old {
            if !new.iter().any(|c| c.id == old_change.id) {
                // Mark as completed/archived by sending final status
                let mut archived = old_change.clone();
                archived.status = "archived".to_string();
                diff.push(archived);
            }
        }

        diff
    }

    /// Refresh state from disk by re-reading changes using native parser.
    /// This ensures the web state reflects the latest task progress from worktree.
    /// Preserves the existing app_mode to avoid overwriting runtime execution state.
    pub async fn refresh_from_disk(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::openspec;

        let repo_root =
            std::env::current_dir().map_err(|e| format!("Failed to resolve repo root: {}", e))?;

        // Read changes from disk using native parser
        let mut changes = openspec::list_changes_native()
            .map_err(|e| format!("Failed to refresh changes from disk: {}", e))?;

        // Enrich progress from worktrees (uncommitted tasks.md)
        // Use unified fallback helper: worktree → archive → base
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

        // Update state with refreshed changes, preserving app_mode
        self.update_with_mode(&changes, &current_app_mode).await;

        // Update worktrees in state and broadcast
        let worktrees_changed = {
            let mut state = self.state.write().await;
            let changed = state.worktrees != worktrees;
            state.worktrees = worktrees.clone();
            changed
        };

        // Broadcast worktrees update if changed
        if worktrees_changed {
            let update = StateUpdate {
                msg_type: "state_update".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                changes: self.state.read().await.changes.clone(),
                logs: None,
                worktrees: Some(worktrees),
                app_mode: None,
            };
            let _ = self.tx.send(update);
        }

        Ok(())
    }

    /// Subscribe to state updates
    pub fn subscribe(&self) -> broadcast::Receiver<StateUpdate> {
        self.tx.subscribe()
    }

    /// Get a specific change by ID
    pub async fn get_change(&self, id: &str) -> Option<ChangeStatus> {
        let state = self.state.read().await;
        state.changes.iter().find(|c| c.id == id).cloned()
    }

    /// Get list of all changes
    pub async fn list_changes(&self) -> Vec<ChangeStatus> {
        self.state.read().await.changes.clone()
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

        // Subscribe before update
        let mut rx = web_state.subscribe();

        // Update with new changes
        let changes = vec![create_test_change("new-change", 2, 4)];
        web_state.update(&changes).await;

        // Verify state was updated
        let state = web_state.get_state().await;
        assert_eq!(state.total_changes, 1);
        assert_eq!(state.changes[0].id, "new-change");

        // Verify broadcast was sent
        let update = rx.try_recv().unwrap();
        assert_eq!(update.msg_type, "state_update");
        assert_eq!(update.changes[0].id, "new-change");
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
        assert_eq!(state.changes[0].queue_status, Some("applying".to_string()));
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
        assert_eq!(state.changes[0].queue_status, Some("accepting".to_string()));
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
        assert_eq!(state.changes[0].queue_status, Some("archiving".to_string()));
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
    async fn test_web_state_get_change() {
        let changes = vec![
            create_test_change("change-a", 1, 3),
            create_test_change("change-b", 2, 5),
        ];
        let web_state = WebState::new(&changes);

        let change = web_state.get_change("change-b").await;
        assert!(change.is_some());
        assert_eq!(change.unwrap().id, "change-b");

        let missing = web_state.get_change("nonexistent").await;
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_compute_diff_no_changes() {
        let changes = vec![create_test_change("change-a", 2, 5)];
        let web_state = WebState::new(&changes);

        let mut rx = web_state.subscribe();

        // Update with identical changes
        web_state.update(&changes).await;

        // No broadcast should be sent
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_compute_diff_progress_update() {
        let initial = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 1, 5),
        ];
        let web_state = WebState::new(&initial);

        let mut rx = web_state.subscribe();

        // Update with progress change
        let updated = vec![
            create_test_change("change-a", 3, 5),
            create_test_change("change-b", 1, 5),
        ];
        web_state.update(&updated).await;

        // Broadcast should include full snapshot
        let update = rx.try_recv().unwrap();
        assert_eq!(update.changes.len(), 2);
        assert!(update.changes.iter().any(|change| change.id == "change-a"));
        assert!(update.changes.iter().any(|change| change.id == "change-b"));
        let updated_change = update
            .changes
            .iter()
            .find(|change| change.id == "change-a")
            .unwrap();
        assert_eq!(updated_change.completed_tasks, 3);
    }

    #[tokio::test]
    async fn test_compute_diff_archived_change() {
        let initial = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 3, 5),
        ];
        let web_state = WebState::new(&initial);

        let mut rx = web_state.subscribe();

        // Update with one change removed (archived)
        let updated = vec![create_test_change("change-a", 2, 5)];
        web_state.update(&updated).await;

        // Broadcast should include the latest full list
        let update = rx.try_recv().unwrap();
        assert_eq!(update.changes.len(), 1);
        assert_eq!(update.changes[0].id, "change-a");
        assert_eq!(update.changes[0].status, "in_progress");
    }

    #[tokio::test]
    async fn test_compute_diff_new_change() {
        let initial = vec![create_test_change("change-a", 2, 5)];
        let web_state = WebState::new(&initial);

        let mut rx = web_state.subscribe();

        // Update with new change added
        let updated = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 0, 3),
        ];
        web_state.update(&updated).await;

        // Broadcast should include the latest full list
        let update = rx.try_recv().unwrap();
        assert_eq!(update.changes.len(), 2);
        assert!(update.changes.iter().any(|change| change.id == "change-a"));
        assert!(update.changes.iter().any(|change| change.id == "change-b"));
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
        assert_eq!(
            state.changes[0].queue_status,
            Some("archiving".to_string()),
            "queue_status should be set to archiving"
        );
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
        assert_eq!(
            state.changes[0].queue_status,
            Some("archiving".to_string()),
            "queue_status should be preserved"
        );
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
        assert_eq!(
            state.changes[0].queue_status,
            Some("resolving".to_string()),
            "queue_status should be preserved"
        );
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
        assert_eq!(
            state.changes[0].queue_status,
            Some("resolve pending".to_string()),
            "queue_status should be 'resolve pending' when resolve is running"
        );
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
        assert_eq!(
            state.changes[0].queue_status,
            Some("merge wait".to_string()),
            "queue_status should be 'merge wait' when resolve is not running and not auto-resumable"
        );
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
        assert_eq!(state.changes[0].queue_status, Some("resolving".to_string()));
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
        assert_eq!(state.changes[0].queue_status, Some("archiving".to_string()));
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
        assert_eq!(state.changes[0].queue_status, Some("error".to_string()));
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
        assert_eq!(
            state.changes[0].queue_status,
            Some("resolve pending".to_string()),
            "auto-resumable MergeDeferred must show 'resolve pending' even when resolve is not running"
        );
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
}
