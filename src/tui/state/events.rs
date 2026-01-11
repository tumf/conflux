//! Event handling for AppState
//!
//! Contains orchestrator event handling and change refresh logic.

use std::collections::HashSet;
use std::time::Instant;

use crate::openspec::Change;

use super::super::events::{LogEntry, OrchestratorEvent};
use super::super::types::{AppMode, QueueStatus, StopMode};
use super::change::ChangeState;
use super::AppState;

impl AppState {
    /// Handle an event from the orchestrator
    pub fn handle_orchestrator_event(&mut self, event: OrchestratorEvent) {
        match event {
            OrchestratorEvent::ProcessingStarted(id) => {
                self.current_change = Some(id.clone());
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Processing;
                    change.started_at = Some(Instant::now());
                    change.elapsed_time = None;
                }
                self.add_log(LogEntry::info(format!("Processing: {}", id)));
            }
            OrchestratorEvent::ProgressUpdated {
                id,
                completed,
                total,
            } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    // Only update progress if the change is still being processed
                    // After Completed, the tasks.md file may be moved/archived
                    match change.queue_status {
                        QueueStatus::Completed | QueueStatus::Archiving | QueueStatus::Archived => {
                            // Don't update progress after completion - file may be moved
                        }
                        _ => {
                            change.completed_tasks = completed;
                            change.total_tasks = total;
                        }
                    }
                }
            }
            OrchestratorEvent::ProcessingCompleted(id) => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Completed;
                }
                self.add_log(LogEntry::success(format!("Completed: {}", id)));
            }
            OrchestratorEvent::ArchiveStarted(id) => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Archiving;
                }
                self.add_log(LogEntry::info(format!("Archiving: {}", id)));
            }
            OrchestratorEvent::ChangeArchived(id) => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Archived;
                    // Record final elapsed time
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                }
                self.add_log(LogEntry::info(format!("Archived: {}", id)));
            }
            OrchestratorEvent::ProcessingError { id, error } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Error(error.clone());
                    // Record elapsed time on error
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                }
                // Record orchestration elapsed time on error
                if let Some(started) = self.orchestration_started_at {
                    self.orchestration_elapsed = Some(started.elapsed());
                }
                self.add_log(LogEntry::error(format!("Error in {}: {}", id, error)));
                // Transition to Error mode
                self.mode = AppMode::Error;
                self.error_change_id = Some(id.clone());
                self.current_change = None;
            }
            OrchestratorEvent::AllCompleted => {
                self.mode = AppMode::Select;
                self.current_change = None;
                self.stop_mode = StopMode::None;
                // Record final orchestration time
                if let Some(started) = self.orchestration_started_at {
                    self.orchestration_elapsed = Some(started.elapsed());
                }
                self.add_log(LogEntry::success("All changes processed successfully"));
            }
            OrchestratorEvent::Stopped => {
                self.mode = AppMode::Stopped;
                self.current_change = None;
                self.stop_mode = StopMode::None;
                if let Some(started) = self.orchestration_started_at {
                    self.orchestration_elapsed = Some(started.elapsed());
                }
                self.add_log(LogEntry::warn("Processing stopped"));
            }
            OrchestratorEvent::Log(entry) => {
                self.add_log(entry);
            }
            OrchestratorEvent::ChangesRefreshed(changes) => {
                self.update_changes(changes);
            }
        }
    }

    /// Update changes from a refresh
    pub fn update_changes(&mut self, fetched_changes: Vec<Change>) {
        // Detect new changes
        let new_ids: Vec<String> = fetched_changes
            .iter()
            .filter(|c| !self.known_change_ids.contains(&c.id))
            .map(|c| c.id.clone())
            .collect();

        // Update existing changes
        for fetched in &fetched_changes {
            if let Some(existing) = self.changes.iter_mut().find(|c| c.id == fetched.id) {
                // Only update progress if we have valid data (total > 0)
                // and the change is NOT being processed (parallel mode uses workspace)
                if fetched.total_tasks > 0 {
                    match existing.queue_status {
                        QueueStatus::Completed
                        | QueueStatus::Archiving
                        | QueueStatus::Archived
                        | QueueStatus::Processing => {
                            // Don't update progress after completion or during processing
                            // - After completion: file may be moved
                            // - During processing: parallel mode uses workspace, not main repo
                        }
                        _ => {
                            existing.completed_tasks = fetched.completed_tasks;
                            existing.total_tasks = fetched.total_tasks;
                        }
                    }
                }
            }
        }

        // Add new changes
        for id in &new_ids {
            if let Some(fetched) = fetched_changes.iter().find(|c| &c.id == id) {
                let mut new_state = ChangeState::from_change(fetched, false); // New changes are not selected
                new_state.is_new = true;
                self.changes.push(new_state);
                self.known_change_ids.insert(id.clone());
                self.add_log(LogEntry::warn(format!("Discovered new change: {}", id)));
            }
        }

        self.new_change_count = self.changes.iter().filter(|c| c.is_new).count();
        self.last_refresh = Instant::now();

        // Remove changes that no longer exist (have been archived externally)
        let current_ids: HashSet<String> = fetched_changes.iter().map(|c| c.id.clone()).collect();
        self.changes.retain(|c| {
            // Keep if still exists, or if it's in a terminal state (completed/archiving/archived/error)
            current_ids.contains(&c.id)
                || matches!(
                    c.queue_status,
                    QueueStatus::Completed
                        | QueueStatus::Archiving
                        | QueueStatus::Archived
                        | QueueStatus::Error(_)
                )
        });

        // Ensure cursor is valid
        if self.cursor_index >= self.changes.len() && !self.changes.is_empty() {
            self.cursor_index = self.changes.len() - 1;
            self.list_state.select(Some(self.cursor_index));
        }
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
            last_modified: "now".to_string(),
            is_approved: false,
            dependencies: Vec::new(),
        }
    }

    fn create_approved_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn test_update_changes_detects_new() {
        let initial = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(initial);

        let fetched = vec![
            create_test_change("a", 1, 1), // Updated
            create_test_change("b", 0, 2), // New
        ];

        app.update_changes(fetched);

        assert_eq!(app.changes.len(), 2);
        assert_eq!(app.changes[0].completed_tasks, 1); // Updated
        assert!(app.changes[1].is_new);
        assert!(!app.changes[1].selected); // New changes are not selected
        assert_eq!(app.new_change_count, 1);
    }

    #[test]
    fn test_processing_error_transitions_to_error_mode() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        // Simulate processing error
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingError {
            id: "a".to_string(),
            error: "LLM error".to_string(),
        });

        // Mode should be Error
        assert_eq!(app.mode, AppMode::Error);
        assert_eq!(app.error_change_id, Some("a".to_string()));
        assert!(matches!(app.changes[0].queue_status, QueueStatus::Error(_)));
        assert!(app.current_change.is_none());
    }

    #[test]
    fn test_update_changes_marks_new_changes_correctly() {
        let initial_changes = vec![create_test_change("existing", 1, 2)];
        let mut app = AppState::new(initial_changes);

        // Simulate discovering new change
        let updated_changes = vec![
            create_test_change("existing", 1, 2),
            create_test_change("brand-new", 0, 3),
        ];

        app.update_changes(updated_changes);

        // Find the new change
        let brand_new = app.changes.iter().find(|c| c.id == "brand-new");
        assert!(brand_new.is_some());
        assert!(brand_new.unwrap().is_new);

        // Existing should not be marked as new
        let existing = app.changes.iter().find(|c| c.id == "existing");
        assert!(existing.is_some());
        assert!(!existing.unwrap().is_new);
    }

    #[test]
    fn test_new_change_count_tracking() {
        let initial_changes = vec![create_test_change("existing", 1, 2)];
        let mut app = AppState::new(initial_changes);

        // Initially no new changes
        assert_eq!(app.new_change_count, 0);

        // Add new changes
        let updated_changes = vec![
            create_test_change("existing", 1, 2),
            create_test_change("new1", 0, 1),
            create_test_change("new2", 0, 2),
        ];

        app.update_changes(updated_changes);

        // Should have 2 new changes
        assert_eq!(app.new_change_count, 2);
    }
}
