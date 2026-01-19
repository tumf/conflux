//! Progress update event handler
//!
//! Handles ProgressUpdated event

use super::super::AppState;

impl AppState {
    /// Handle ProgressUpdated event
    pub(super) fn handle_progress_updated(
        &mut self,
        change_id: String,
        completed: u32,
        total: u32,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            // Update progress for all states when valid data is available.
            // Only update if total > 0 to avoid resetting progress on retrieval failure.
            // Progress retrieval failure (0/0) should preserve existing progress.
            if total > 0 {
                change.completed_tasks = completed;
                change.total_tasks = total;
            }
            // Never modify queue_status here.
            // In Stopped mode, task completion does not trigger auto-queue.
        }
    }
}
