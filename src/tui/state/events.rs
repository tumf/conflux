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
            OrchestratorEvent::ApplyStarted { change_id } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    if change.started_at.is_none() {
                        change.started_at = Some(Instant::now());
                    }
                    change.queue_status = QueueStatus::Processing;
                    change.elapsed_time = None;
                }
                self.add_log(LogEntry::info(format!("Apply started: {}", change_id)));
            }
            OrchestratorEvent::ProgressUpdated {
                change_id,
                completed,
                total,
            } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    // Only update progress, never modify queue_status.
                    // In Stopped mode, task completion does not trigger auto-queue.
                    // After Completed, the tasks.md file may be moved/archived.
                    match change.queue_status {
                        QueueStatus::Completed
                        | QueueStatus::Archiving
                        | QueueStatus::Archived
                        | QueueStatus::MergeWait
                        | QueueStatus::Resolving => {
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
            OrchestratorEvent::ApplyFailed { change_id, error } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.queue_status = QueueStatus::Error(error.clone());
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                }
                self.add_log(LogEntry::error(format!(
                    "Apply failed for {}: {}",
                    change_id, error
                )));
            }
            OrchestratorEvent::ArchiveFailed { change_id, error } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.queue_status = QueueStatus::Error(error.clone());
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                }
                self.add_log(LogEntry::error(format!(
                    "Archive failed for {}: {}",
                    change_id, error
                )));
            }
            OrchestratorEvent::ArchiveStarted(id) => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    if change.started_at.is_none() {
                        change.started_at = Some(Instant::now());
                    }
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
            OrchestratorEvent::MergeDeferred { change_id, reason } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.queue_status = QueueStatus::MergeWait;
                }
                self.add_log(LogEntry::warn(format!(
                    "Merge deferred for {}: {}",
                    change_id, reason
                )));
            }
            OrchestratorEvent::ResolveStarted { change_id } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    if change.started_at.is_none() {
                        change.started_at = Some(Instant::now());
                    }
                    change.queue_status = QueueStatus::Resolving;
                    change.elapsed_time = None;
                }
                self.add_log(LogEntry::info(format!(
                    "Resolving merge for '{}'",
                    change_id
                )));
            }
            OrchestratorEvent::ResolveCompleted {
                change_id,
                worktree_change_ids,
            } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.queue_status = QueueStatus::Archived;
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                }
                if let Some(ids) = worktree_change_ids {
                    self.apply_worktree_status(&ids);
                }
                self.add_log(LogEntry::success(format!(
                    "Merge resolved for '{}'",
                    change_id
                )));
            }
            OrchestratorEvent::MergeCompleted {
                change_id,
                revision: _,
            } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.queue_status = QueueStatus::Archived;
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                }
                self.add_log(LogEntry::success(format!(
                    "Merge completed for '{}'",
                    change_id
                )));
            }
            OrchestratorEvent::ResolveFailed { change_id, error } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.queue_status = QueueStatus::MergeWait;
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                }
                let message = format!("Failed to resolve merge for '{}': {}", change_id, error);
                self.warning_popup = Some(super::WarningPopup {
                    title: "Merge resolve failed".to_string(),
                    message: message.clone(),
                });
                self.add_log(LogEntry::error(message));
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
            OrchestratorEvent::Error { message } => {
                self.add_log(LogEntry::error(message.clone()));
                self.mode = AppMode::Error;
                self.error_change_id = None;
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
            OrchestratorEvent::Warning { title, message } => {
                self.warning_popup = Some(super::WarningPopup {
                    title: title.clone(),
                    message: message.clone(),
                });
                self.add_log(LogEntry::warn(message));
            }
            OrchestratorEvent::Log(entry) => {
                self.add_log(entry);
            }
            OrchestratorEvent::ChangesRefreshed {
                changes,
                committed_change_ids,
                worktree_change_ids,
            } => {
                self.update_changes(changes);
                self.apply_parallel_eligibility(&committed_change_ids);
                self.apply_worktree_status(&worktree_change_ids);
            }
            // Output events - add to log
            OrchestratorEvent::ApplyOutput { change_id, output } => {
                self.add_log(LogEntry::info(format!("[{}] {}", change_id, output)));
            }
            OrchestratorEvent::ArchiveOutput { change_id, output } => {
                self.add_log(LogEntry::info(format!("[{}] {}", change_id, output)));
            }
            OrchestratorEvent::AnalysisOutput { output } => {
                self.add_log(LogEntry::info(format!("[Analysis] {}", output)));
            }
            OrchestratorEvent::ResolveOutput { output } => {
                self.add_log(LogEntry::info(format!("[Resolve] {}", output)));
            }
            // Ignore other parallel-specific events that don't affect TUI state
            _ => {
                // Other events (workspace, merge, group events) are for status tracking
                // and don't need to be displayed in the log
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
                    // If change still exists after archiving, it means archive failed
                    // Revert to appropriate status based on completion
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
                        | QueueStatus::MergeWait
                        | QueueStatus::Resolving
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
            }
        }

        // Track all known IDs (new + existing)
        self.known_change_ids.extend(new_ids);

        self.new_change_count = self.changes.iter().filter(|c| c.is_new).count();
        self.last_refresh = Instant::now();

        // Remove changes that no longer exist (have been archived externally)
        let current_ids: HashSet<String> = fetched_changes.iter().map(|c| c.id.clone()).collect();
        self.changes.retain(|c| {
            // Keep if still exists, apply started in this session, or in a terminal state.
            current_ids.contains(&c.id)
                || c.started_at.is_some()
                || matches!(
                    c.queue_status,
                    QueueStatus::Completed
                        | QueueStatus::Archiving
                        | QueueStatus::Archived
                        | QueueStatus::MergeWait
                        | QueueStatus::Resolving
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
    use std::collections::HashSet;

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
    fn test_apply_started_sets_started_at() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        let initial_log_count = app.logs.len();

        app.handle_orchestrator_event(OrchestratorEvent::ApplyStarted {
            change_id: "change-a".to_string(),
        });

        assert!(app.changes[0].started_at.is_some());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);
        assert!(app.changes[0].elapsed_time.is_none());
        assert_eq!(app.logs.len(), initial_log_count + 1);
        assert!(app.logs.last().unwrap().message.contains("Apply started"));
    }

    #[test]
    fn test_apply_started_preserves_existing_started_at() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        let started_at = Instant::now();
        app.changes[0].started_at = Some(started_at);

        app.handle_orchestrator_event(OrchestratorEvent::ApplyStarted {
            change_id: "change-a".to_string(),
        });

        assert_eq!(app.changes[0].started_at, Some(started_at));
    }

    #[test]
    fn test_archive_started_sets_started_at_when_none() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::ArchiveStarted("change-a".to_string()));

        assert!(app.changes[0].started_at.is_some());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archiving);
    }

    #[test]
    fn test_archive_started_preserves_started_at() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        let started_at = Instant::now();
        app.changes[0].started_at = Some(started_at);

        app.handle_orchestrator_event(OrchestratorEvent::ArchiveStarted("change-a".to_string()));

        assert_eq!(app.changes[0].started_at, Some(started_at));
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archiving);
    }

    #[test]
    fn test_parallel_execution_elapsed_time_flow() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::ApplyStarted {
            change_id: "change-a".to_string(),
        });

        let started_at = app.changes[0].started_at;
        assert!(started_at.is_some());

        std::thread::sleep(std::time::Duration::from_millis(1));

        app.handle_orchestrator_event(OrchestratorEvent::ArchiveStarted("change-a".to_string()));
        assert_eq!(app.changes[0].started_at, started_at);

        app.handle_orchestrator_event(OrchestratorEvent::ChangeArchived("change-a".to_string()));

        assert_eq!(app.changes[0].queue_status, QueueStatus::Archived);
        assert!(app.changes[0].elapsed_time.is_some());
        assert!(app.changes[0].elapsed_time.unwrap().as_nanos() > 0);
    }

    #[test]
    fn test_merge_deferred_sets_merge_wait_status() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::MergeDeferred {
            change_id: "change-a".to_string(),
            reason: "Base working tree dirty".to_string(),
        });

        assert_eq!(app.changes[0].queue_status, QueueStatus::MergeWait);
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Merge deferred")));
    }

    #[test]
    fn test_resolve_started_sets_resolving_status() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::MergeWait;

        app.handle_orchestrator_event(OrchestratorEvent::ResolveStarted {
            change_id: "change-a".to_string(),
        });

        assert_eq!(app.changes[0].queue_status, QueueStatus::Resolving);
        assert!(app.changes[0].started_at.is_some());
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Resolving merge")));
    }

    #[test]
    fn test_resolve_completed_sets_archived_and_updates_worktrees() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.changes[0].started_at = Some(Instant::now());

        let mut ids = HashSet::new();
        ids.insert("change-a".to_string());

        app.handle_orchestrator_event(OrchestratorEvent::ResolveCompleted {
            change_id: "change-a".to_string(),
            worktree_change_ids: Some(ids),
        });

        assert_eq!(app.changes[0].queue_status, QueueStatus::Archived);
        assert!(app.changes[0].elapsed_time.is_some());
        assert!(app.changes[0].has_worktree);
    }

    #[test]
    fn test_resolve_failed_restores_merge_wait_and_warns() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Resolving;

        app.handle_orchestrator_event(OrchestratorEvent::ResolveFailed {
            change_id: "change-a".to_string(),
            error: "boom".to_string(),
        });

        assert_eq!(app.changes[0].queue_status, QueueStatus::MergeWait);
        assert!(app.warning_popup.is_some());
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Failed to resolve merge")));
    }

    #[test]
    fn test_merge_completed_sets_archived_status() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[0].started_at = Some(Instant::now());

        app.handle_orchestrator_event(OrchestratorEvent::MergeCompleted {
            change_id: "change-a".to_string(),
            revision: "abc123".to_string(),
        });

        assert_eq!(app.changes[0].queue_status, QueueStatus::Archived);
        assert!(app.changes[0].elapsed_time.is_some());
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Merge completed")));
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
            change_id: "change-a".to_string(),
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
            change_id: "change-a".to_string(),
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
    fn test_apply_failed_marks_change_error_without_error_mode() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].selected = true;
        app.start_processing();

        app.handle_orchestrator_event(OrchestratorEvent::ApplyFailed {
            change_id: "a".to_string(),
            error: "apply failed".to_string(),
        });

        assert!(matches!(app.changes[0].queue_status, QueueStatus::Error(_)));
        assert_eq!(app.mode, AppMode::Running);
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

    // === Tests for update-tui-archived-retention ===

    /// Test that archived changes remain in the list when they no longer exist in fetched_changes.
    /// This covers the case where a change has been successfully archived (moved to archive directory)
    /// and is no longer returned by the file system scan.
    #[test]
    fn test_archived_changes_retained_after_removal_from_filesystem() {
        // Setup: two changes, one archived
        let changes = vec![
            create_approved_change("change-a", 5, 5),
            create_approved_change("change-b", 3, 3),
        ];
        let mut app = AppState::new(changes);

        // Mark change-a as archived (simulating successful archive operation)
        app.changes[0].queue_status = QueueStatus::Archived;

        // Simulate refresh where archived change no longer exists in filesystem
        // (it has been moved to archive directory)
        let fetched = vec![create_approved_change("change-b", 3, 3)];
        app.update_changes(fetched);

        // Archived change should still be in the list
        assert_eq!(app.changes.len(), 2, "Archived change should be retained");
        assert_eq!(app.changes[0].id, "change-a");
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archived);

        // Non-archived change should still be present
        assert_eq!(app.changes[1].id, "change-b");
    }

    /// Test that multiple archived changes are all retained after filesystem removal.
    #[test]
    fn test_multiple_archived_changes_retained() {
        let changes = vec![
            create_approved_change("change-a", 5, 5),
            create_approved_change("change-b", 3, 3),
            create_approved_change("change-c", 2, 2),
        ];
        let mut app = AppState::new(changes);

        // Mark all as archived
        app.changes[0].queue_status = QueueStatus::Archived;
        app.changes[1].queue_status = QueueStatus::Archived;
        app.changes[2].queue_status = QueueStatus::Archived;

        // Simulate refresh with empty fetched list (all archived)
        let fetched: Vec<Change> = vec![];
        app.update_changes(fetched);

        // All archived changes should be retained
        assert_eq!(
            app.changes.len(),
            3,
            "All archived changes should be retained"
        );
        for change in &app.changes {
            assert_eq!(change.queue_status, QueueStatus::Archived);
        }
    }

    /// Test that archived changes preserve their display state (progress info).
    #[test]
    fn test_archived_changes_preserve_display_state() {
        let changes = vec![create_approved_change("change-a", 7, 10)];
        let mut app = AppState::new(changes);

        // Set progress and mark as archived
        app.changes[0].completed_tasks = 7;
        app.changes[0].total_tasks = 10;
        app.changes[0].queue_status = QueueStatus::Archived;
        app.changes[0].selected = true;
        app.changes[0].is_approved = true;

        // Simulate refresh with change no longer in filesystem
        let fetched: Vec<Change> = vec![];
        app.update_changes(fetched);

        // Archived change should retain all display state
        assert_eq!(app.changes.len(), 1);
        assert_eq!(app.changes[0].completed_tasks, 7);
        assert_eq!(app.changes[0].total_tasks, 10);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archived);
        assert!(app.changes[0].selected);
        assert!(app.changes[0].is_approved);
    }

    /// Test that Completed and Error states are also retained (terminal states).
    #[test]
    fn test_terminal_states_retained_after_removal() {
        let changes = vec![
            create_approved_change("archived", 5, 5),
            create_approved_change("completed", 3, 3),
            create_approved_change("error", 1, 2),
        ];
        let mut app = AppState::new(changes);

        app.changes[0].queue_status = QueueStatus::Archived;
        app.changes[1].queue_status = QueueStatus::Completed;
        app.changes[2].queue_status = QueueStatus::Error("Test error".to_string());

        // Simulate refresh with all changes removed from filesystem
        let fetched: Vec<Change> = vec![];
        app.update_changes(fetched);

        // All terminal state changes should be retained
        assert_eq!(
            app.changes.len(),
            3,
            "All terminal state changes should be retained"
        );
        assert!(matches!(app.changes[0].queue_status, QueueStatus::Archived));
        assert!(matches!(
            app.changes[1].queue_status,
            QueueStatus::Completed
        ));
        assert!(matches!(app.changes[2].queue_status, QueueStatus::Error(_)));
    }
    /// Test that started changes are retained when removed from fetched_changes.
    #[test]
    fn test_started_changes_retained_when_not_fetched() {
        let changes = vec![create_approved_change("started", 1, 3)];
        let mut app = AppState::new(changes);

        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[0].started_at = Some(Instant::now());

        // Simulate refresh with no changes present in filesystem
        let fetched: Vec<Change> = vec![];
        app.update_changes(fetched);

        assert_eq!(app.changes.len(), 1, "Started change should be retained");
        assert_eq!(app.changes[0].id, "started");
        assert!(app.changes[0].started_at.is_some());
    }

    /// Test that non-terminal state changes are removed when not in fetched_changes.
    #[test]
    fn test_non_terminal_changes_removed_when_not_fetched() {
        let changes = vec![
            create_approved_change("archived", 5, 5),
            create_approved_change("not-queued", 1, 3),
            create_approved_change("queued", 2, 4),
        ];
        let mut app = AppState::new(changes);

        app.changes[0].queue_status = QueueStatus::Archived;
        app.changes[1].queue_status = QueueStatus::NotQueued;
        app.changes[2].queue_status = QueueStatus::Queued;

        // Simulate refresh with only archived change present
        let fetched: Vec<Change> = vec![];
        app.update_changes(fetched);

        // Only archived change should remain; NotQueued and Queued should be removed
        assert_eq!(
            app.changes.len(),
            1,
            "Only archived change should be retained"
        );
        assert_eq!(app.changes[0].id, "archived");
    }

    /// Test that ApplyOutput events are logged correctly
    #[test]
    fn test_apply_output_event_logged() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        let initial_log_count = app.logs.len();

        // Send ApplyOutput event
        app.handle_orchestrator_event(OrchestratorEvent::ApplyOutput {
            change_id: "change-a".to_string(),
            output: "Test output line".to_string(),
        });

        // Log should be added
        assert_eq!(app.logs.len(), initial_log_count + 1);
        assert!(app.logs.last().unwrap().message.contains("change-a"));
        assert!(app
            .logs
            .last()
            .unwrap()
            .message
            .contains("Test output line"));
    }

    /// Test that ArchiveOutput events are logged correctly
    #[test]
    fn test_archive_output_event_logged() {
        let changes = vec![create_approved_change("change-b", 5, 5)];
        let mut app = AppState::new(changes);
        let initial_log_count = app.logs.len();

        // Send ArchiveOutput event
        app.handle_orchestrator_event(OrchestratorEvent::ArchiveOutput {
            change_id: "change-b".to_string(),
            output: "Archive output line".to_string(),
        });

        // Log should be added
        assert_eq!(app.logs.len(), initial_log_count + 1);
        assert!(app.logs.last().unwrap().message.contains("change-b"));
        assert!(app
            .logs
            .last()
            .unwrap()
            .message
            .contains("Archive output line"));
    }

    /// Test that AnalysisOutput events are logged correctly
    #[test]
    fn test_analysis_output_event_logged() {
        let changes = vec![create_approved_change("change-c", 0, 3)];
        let mut app = AppState::new(changes);
        let initial_log_count = app.logs.len();

        // Send AnalysisOutput event
        app.handle_orchestrator_event(OrchestratorEvent::AnalysisOutput {
            output: "Analyzing dependencies...".to_string(),
        });

        // Log should be added
        assert_eq!(app.logs.len(), initial_log_count + 1);
        assert!(app.logs.last().unwrap().message.contains("Analysis"));
        assert!(app
            .logs
            .last()
            .unwrap()
            .message
            .contains("Analyzing dependencies"));
    }

    /// Test that ResolveOutput events are logged correctly
    #[test]
    fn test_resolve_output_event_logged() {
        let changes = vec![create_approved_change("change-d", 1, 4)];
        let mut app = AppState::new(changes);
        let initial_log_count = app.logs.len();

        // Send ResolveOutput event
        app.handle_orchestrator_event(OrchestratorEvent::ResolveOutput {
            output: "Resolving conflicts...".to_string(),
        });

        // Log should be added
        assert_eq!(app.logs.len(), initial_log_count + 1);
        assert!(app.logs.last().unwrap().message.contains("Resolve"));
        assert!(app
            .logs
            .last()
            .unwrap()
            .message
            .contains("Resolving conflicts"));
    }

    /// Test that Log events with stdout/stderr content are processed correctly
    #[test]
    fn test_log_event_with_stdout_content() {
        let changes = vec![create_approved_change("change-e", 0, 2)];
        let mut app = AppState::new(changes);
        let initial_log_count = app.logs.len();

        // Send Log event with stdout content (simulating serial mode)
        app.handle_orchestrator_event(OrchestratorEvent::Log(LogEntry::info(
            "Claude output line 1".to_string(),
        )));
        app.handle_orchestrator_event(OrchestratorEvent::Log(LogEntry::info(
            "Claude output line 2".to_string(),
        )));

        // Logs should be added
        assert_eq!(app.logs.len(), initial_log_count + 2);
        assert_eq!(app.logs[initial_log_count].message, "Claude output line 1");
        assert_eq!(
            app.logs[initial_log_count + 1].message,
            "Claude output line 2"
        );
    }
}
