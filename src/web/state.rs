//! Web monitoring state management.
//!
//! Provides thread-safe state access and broadcasting for WebSocket clients.

use crate::events::ExecutionEvent;
use crate::openspec::Change;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

/// State update message sent to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    /// Type of update message
    #[serde(rename = "type")]
    pub msg_type: String,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// List of changes with current status
    pub changes: Vec<ChangeStatus>,
}

/// Change status for WebSocket updates
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// Whether the change is approved
    pub is_approved: bool,
    /// Dependencies on other changes
    pub dependencies: Vec<String>,
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
            is_approved: change.is_approved,
            dependencies: change.dependencies.clone(),
        }
    }
}

/// Full orchestrator state for REST API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorState {
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
}

impl OrchestratorState {
    /// Create a new state from a list of changes
    pub fn from_changes(changes: &[Change]) -> Self {
        let change_statuses: Vec<ChangeStatus> = changes.iter().map(ChangeStatus::from).collect();

        let completed = change_statuses
            .iter()
            .filter(|c| c.status == "complete")
            .count();
        let in_progress = change_statuses
            .iter()
            .filter(|c| c.status == "in_progress")
            .count();
        let pending = change_statuses
            .iter()
            .filter(|c| c.status == "pending")
            .count();

        Self {
            total_changes: change_statuses.len(),
            completed_changes: completed,
            in_progress_changes: in_progress,
            pending_changes: pending,
            changes: change_statuses,
            last_updated: chrono::Utc::now().to_rfc3339(),
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

fn refresh_summary(state: &mut OrchestratorState) {
    state.total_changes = state.changes.len();
    state.completed_changes = state
        .changes
        .iter()
        .filter(|change| change.status == "complete")
        .count();
    state.in_progress_changes = state
        .changes
        .iter()
        .filter(|change| change.status == "in_progress")
        .count();
    state.pending_changes = state
        .changes
        .iter()
        .filter(|change| change.status == "pending")
        .count();
    state.last_updated = chrono::Utc::now().to_rfc3339();
}

/// Shared web state with broadcast channel for updates
pub struct WebState {
    /// Current orchestrator state (thread-safe)
    state: RwLock<OrchestratorState>,
    /// Broadcast channel for state updates
    tx: broadcast::Sender<StateUpdate>,
}

impl WebState {
    /// Create a new WebState with initial changes
    pub fn new(initial_changes: &[Change]) -> Self {
        let (tx, _) = broadcast::channel(100);
        let state = OrchestratorState::from_changes(initial_changes);

        Self {
            state: RwLock::new(state),
            tx,
        }
    }

    /// Get a read lock on the current state
    pub async fn get_state(&self) -> OrchestratorState {
        self.state.read().await.clone()
    }

    /// Update state with new changes and broadcast to WebSocket clients.
    /// Only broadcasts if there are actual changes from the previous state.
    pub async fn update(&self, changes: &[Change]) {
        let new_state = OrchestratorState::from_changes(changes);

        // Check if state has actually changed
        let has_changes = {
            let old_state = self.state.read().await;
            !self
                .compute_diff(&old_state.changes, &new_state.changes)
                .is_empty()
        };

        // Update internal state
        {
            let mut state = self.state.write().await;
            *state = new_state.clone();
        }

        // Only broadcast if there were changes
        if has_changes {
            self.broadcast_snapshot(new_state.changes);
        }
    }

    /// Apply an execution event to the web state and broadcast updates.
    pub async fn apply_execution_event(&self, event: &ExecutionEvent) {
        let mut changes_snapshot = None;

        {
            let mut state = self.state.write().await;
            let mut updated = false;

            match event {
                ExecutionEvent::ProcessingStarted(change_id) => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.status = "in_progress".to_string();
                        change.progress_percent =
                            progress_percent(change.completed_tasks, change.total_tasks);
                        updated = true;
                    }
                }
                ExecutionEvent::ProcessingCompleted(change_id)
                | ExecutionEvent::ChangeArchived(change_id) => {
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
                ExecutionEvent::ProgressUpdated {
                    change_id,
                    completed,
                    total,
                } => {
                    if let Some(change) = state.changes.iter_mut().find(|c| c.id == *change_id) {
                        change.completed_tasks = *completed;
                        change.total_tasks = *total;
                        change.progress_percent = progress_percent(*completed, *total);
                        change.status = status_from_progress(*completed, *total).to_string();
                        updated = true;
                    }
                }
                _ => {}
            }

            if updated {
                refresh_summary(&mut state);
                changes_snapshot = Some(state.changes.clone());
            }
        }

        if let Some(changes) = changes_snapshot {
            self.broadcast_snapshot(changes);
        }
    }

    fn broadcast_snapshot(&self, changes: Vec<ChangeStatus>) {
        let update = StateUpdate {
            msg_type: "state_update".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            changes,
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

    /// Approve a change and broadcast the update to WebSocket clients
    ///
    /// # Arguments
    /// * `id` - The ID of the change to approve
    ///
    /// # Returns
    /// The updated change status, or an error if the change is not found
    pub async fn approve_change(
        &self,
        id: &str,
    ) -> Result<ChangeStatus, Box<dyn std::error::Error + Send + Sync>> {
        use crate::approval;

        // Verify change exists in state
        let change_exists = {
            let state = self.state.read().await;
            state.changes.iter().any(|c| c.id == id)
        };

        if !change_exists {
            return Err(format!("Change '{}' not found", id).into());
        }

        // Perform approval operation
        approval::approve_change(id)?;

        // Update the approval status in state and get the updated change
        let (updated_change, changes_snapshot) = {
            let mut state = self.state.write().await;
            if let Some(index) = state.changes.iter().position(|c| c.id == id) {
                state.changes[index].is_approved = true;
                refresh_summary(&mut state);
                (state.changes[index].clone(), state.changes.clone())
            } else {
                return Err(format!("Change '{}' not found after approval", id).into());
            }
        };

        // Broadcast the update
        self.broadcast_snapshot(changes_snapshot);

        Ok(updated_change)
    }

    /// Unapprove a change and broadcast the update to WebSocket clients
    ///
    /// # Arguments
    /// * `id` - The ID of the change to unapprove
    ///
    /// # Returns
    /// The updated change status, or an error if the change is not found
    pub async fn unapprove_change(
        &self,
        id: &str,
    ) -> Result<ChangeStatus, Box<dyn std::error::Error + Send + Sync>> {
        use crate::approval;

        // Verify change exists in state
        let change_exists = {
            let state = self.state.read().await;
            state.changes.iter().any(|c| c.id == id)
        };

        if !change_exists {
            return Err(format!("Change '{}' not found", id).into());
        }

        // Perform unapproval operation
        approval::unapprove_change(id)?;

        // Update the approval status in state and get the updated change
        let (updated_change, changes_snapshot) = {
            let mut state = self.state.write().await;
            if let Some(index) = state.changes.iter().position(|c| c.id == id) {
                state.changes[index].is_approved = false;
                refresh_summary(&mut state);
                (state.changes[index].clone(), state.changes.clone())
            } else {
                return Err(format!("Change '{}' not found after unapproval", id).into());
            }
        };

        // Broadcast the update
        self.broadcast_snapshot(changes_snapshot);

        Ok(updated_change)
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

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
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
        assert!(status.is_approved);
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
    fn test_orchestrator_state_from_changes() {
        let changes = vec![
            create_test_change("change-a", 0, 3),
            create_test_change("change-b", 2, 5),
            create_test_change("change-c", 4, 4),
        ];

        let state = OrchestratorState::from_changes(&changes);

        assert_eq!(state.total_changes, 3);
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
}
