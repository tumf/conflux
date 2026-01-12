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
                    // Only update progress, never modify queue_status.
                    // In Stopped mode, task completion does not trigger auto-queue.
                    // After Completed, the tasks.md file may be moved/archived.
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
                // Reset any in-flight change back to Queued (same as force stop)
                for change in &mut self.changes {
                    if matches!(
                        change.queue_status,
                        QueueStatus::Processing | QueueStatus::Archiving
                    ) {
                        // Record elapsed time before resetting status
                        if let Some(started) = change.started_at {
                            change.elapsed_time = Some(started.elapsed());
                        }
                        change.queue_status = QueueStatus::Queued;
                    }
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
    ///
    /// IMPORTANT: This method only updates task progress (completed_tasks, total_tasks).
    /// It does NOT modify queue_status. In Stopped mode, task completion does not
    /// trigger auto-queue. Changes are only queued through explicit user action (Space key).
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
                let was_archived = existing.queue_status == QueueStatus::Archived;

                if was_archived {
                    existing.queue_status = if fetched.is_complete() {
                        QueueStatus::Completed
                    } else {
                        QueueStatus::NotQueued
                    };
                    existing.completed_tasks = fetched.completed_tasks;
                    existing.total_tasks = fetched.total_tasks;
                } else if fetched.total_tasks > 0 {
                    // Only update progress if we have valid data (total > 0)
                    // and the change is NOT being processed (parallel mode uses workspace)
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
    use crate::tui::events::TuiCommand;

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

    // === Tests for fix-stopped-task-complete-queued ===

    #[test]
    fn test_task_completion_in_stopped_mode_does_not_auto_queue() {
        // Scenario: Task completion in Stopped mode does not auto-queue
        // WHEN TUI is in Stopped mode
        // AND a change's tasks are updated (e.g., all tasks marked complete)
        // THEN the change queue_status SHALL remain unchanged
        // AND the change SHALL NOT be automatically added to the queue

        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);

        // Start processing and then stop
        app.start_processing();
        app.mode = AppMode::Stopped;

        // Simulate that change was in Queued status before tasks were updated
        // (This is the expected state after stop - Processing changes go back to Queued)
        app.changes[0].queue_status = QueueStatus::NotQueued;

        // Simulate task completion via update_changes (auto-refresh)
        let fetched = vec![create_approved_change("change-a", 5, 5)]; // All tasks complete
        app.update_changes(fetched);

        // Queue status should remain NotQueued (not auto-queued)
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        // Tasks should be updated
        assert_eq!(app.changes[0].completed_tasks, 5);
        assert_eq!(app.changes[0].total_tasks, 5);
    }

    #[test]
    fn test_explicit_queue_addition_in_stopped_mode_works() {
        // Scenario: Explicit queue addition in Stopped mode works
        // WHEN TUI is in Stopped mode
        // AND user presses Space on a not-queued change (even if tasks are 100% complete)
        // THEN the change SHALL be added to the queue
        // AND the change queue_status SHALL become Queued

        let changes = vec![create_approved_change("change-a", 5, 5)]; // 100% complete
        let mut app = AppState::new(changes);

        // Start processing and then stop
        app.start_processing();
        app.mode = AppMode::Stopped;

        // Set to NotQueued (simulating user dequeued or it was never queued)
        app.changes[0].queue_status = QueueStatus::NotQueued;
        app.changes[0].selected = false;

        // User explicitly adds to queue via Space key
        let cmd = app.toggle_selection();

        // Should return AddToQueue command
        assert!(matches!(cmd, Some(TuiCommand::AddToQueue(_))));
        // Queue status should become Queued
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);
        assert!(app.changes[0].selected);
    }

    #[test]
    fn test_progress_update_in_stopped_mode_preserves_not_queued_status() {
        // Test that ProgressUpdated event in Stopped mode doesn't change queue status

        let changes = vec![create_approved_change("change-a", 2, 5)];
        let mut app = AppState::new(changes);

        // Enter Stopped mode
        app.mode = AppMode::Stopped;
        app.changes[0].queue_status = QueueStatus::NotQueued;

        // Receive ProgressUpdated event (simulating tasks being completed externally)
        app.handle_orchestrator_event(OrchestratorEvent::ProgressUpdated {
            id: "change-a".to_string(),
            completed: 5,
            total: 5,
        });

        // Queue status should remain NotQueued
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        // Progress should be updated
        assert_eq!(app.changes[0].completed_tasks, 5);
    }

    #[test]
    fn test_changes_refreshed_in_stopped_mode_preserves_queue_status() {
        // Test that ChangesRefreshed event in Stopped mode doesn't change queue status

        let changes = vec![
            create_approved_change("change-a", 2, 5),
            create_approved_change("change-b", 0, 3),
        ];
        let mut app = AppState::new(changes);

        // Enter Stopped mode with specific queue statuses
        app.mode = AppMode::Stopped;
        app.changes[0].queue_status = QueueStatus::NotQueued;
        app.changes[1].queue_status = QueueStatus::Queued;

        // Receive ChangesRefreshed event with updated task progress
        let refreshed = vec![
            create_approved_change("change-a", 5, 5), // Now complete
            create_approved_change("change-b", 3, 3), // Now complete
        ];
        app.update_changes(refreshed);

        // Queue statuses should remain unchanged
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        assert_eq!(app.changes[1].queue_status, QueueStatus::Queued);
        // Progress should be updated
        assert_eq!(app.changes[0].completed_tasks, 5);
        assert_eq!(app.changes[1].completed_tasks, 3);
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
    fn test_update_changes_unarchives_when_change_still_exists() {
        let changes = vec![create_approved_change("change-a", 5, 5)];
        let mut app = AppState::new(changes);

        app.changes[0].queue_status = QueueStatus::Archived;

        let fetched = vec![create_approved_change("change-a", 5, 5)];
        app.update_changes(fetched);

        assert_eq!(app.changes[0].queue_status, QueueStatus::Completed);
    }

    /// Test that Archiving state preserves progress when tasks.md is not found (0/0)
    /// This is a regression test for the bug where archiving would reset progress to 0.
    #[test]
    fn test_archiving_preserves_progress_when_tasks_not_found() {
        // Setup: change with 5/7 tasks completed, in Archiving state
        let changes = vec![create_approved_change("change-a", 5, 7)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Archiving;
        app.changes[0].completed_tasks = 5;
        app.changes[0].total_tasks = 7;

        // Simulate auto-refresh returning 0/0 (tasks.md moved during archive)
        let fetched = vec![Change {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 0,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        }];
        app.update_changes(fetched);

        // Progress should be preserved (not reset to 0)
        assert_eq!(
            app.changes[0].completed_tasks, 5,
            "completed_tasks should be preserved during archiving"
        );
        assert_eq!(
            app.changes[0].total_tasks, 7,
            "total_tasks should be preserved during archiving"
        );
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archiving);
    }

    /// Test that Archiving state preserves progress when ProgressUpdated event has 0/0
    #[test]
    fn test_archiving_preserves_progress_on_progress_updated_event() {
        // Setup: change with 5/7 tasks completed, in Archiving state
        let changes = vec![create_approved_change("change-a", 5, 7)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Archiving;
        app.changes[0].completed_tasks = 5;
        app.changes[0].total_tasks = 7;

        // Simulate ProgressUpdated event with 0/0 (should not happen in practice, but test anyway)
        app.handle_orchestrator_event(OrchestratorEvent::ProgressUpdated {
            id: "change-a".to_string(),
            completed: 0,
            total: 0,
        });

        // Progress should be preserved
        assert_eq!(
            app.changes[0].completed_tasks, 5,
            "completed_tasks should be preserved during archiving"
        );
        assert_eq!(
            app.changes[0].total_tasks, 7,
            "total_tasks should be preserved during archiving"
        );
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

    #[test]
    fn test_stopped_event_cleans_up_processing_changes() {
        let changes = vec![
            create_approved_change("a", 0, 3),
            create_approved_change("b", 0, 2),
            create_approved_change("c", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Simulate processing state for multiple changes
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[0].started_at = Some(Instant::now());
        app.changes[1].queue_status = QueueStatus::Archiving;
        app.changes[1].started_at = Some(Instant::now());
        app.changes[2].queue_status = QueueStatus::Queued;

        // Handle Stopped event (graceful stop)
        app.handle_orchestrator_event(OrchestratorEvent::Stopped);

        // Processing and Archiving should be reset to Queued
        assert!(matches!(app.changes[0].queue_status, QueueStatus::Queued));
        assert!(matches!(app.changes[1].queue_status, QueueStatus::Queued));
        // Queued should remain Queued
        assert!(matches!(app.changes[2].queue_status, QueueStatus::Queued));
        // Mode should be Stopped
        assert_eq!(app.mode, AppMode::Stopped);
        // current_change should be cleared
        assert!(app.current_change.is_none());
    }

    #[test]
    fn test_stopped_event_records_elapsed_time() {
        let changes = vec![create_approved_change("a", 0, 3)];
        let mut app = AppState::new(changes);

        // Simulate processing state with started_at
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[0].started_at = Some(Instant::now());
        assert!(app.changes[0].elapsed_time.is_none());

        // Wait a tiny bit to ensure elapsed time is non-zero
        std::thread::sleep(std::time::Duration::from_millis(1));

        // Handle Stopped event
        app.handle_orchestrator_event(OrchestratorEvent::Stopped);

        // Elapsed time should be recorded
        assert!(app.changes[0].elapsed_time.is_some());
        assert!(app.changes[0].elapsed_time.unwrap().as_nanos() > 0);
    }
}
