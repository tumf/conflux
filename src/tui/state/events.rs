//! Event handling for AppState
//!
//! Contains orchestrator event handling and change refresh logic.

use std::collections::HashSet;
use std::time::Instant;

use crate::openspec::Change;
use crate::task_parser;

use super::super::events::{LogEntry, OrchestratorEvent};
use super::super::types::{AppMode, QueueStatus, StopMode};
use super::change::ChangeState;
use super::{AppState, WarningPopup};

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
            OrchestratorEvent::ProcessingCompleted(id) => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Completed;
                    // Reload final progress from tasks.md to preserve it
                    if let Ok(progress) = task_parser::parse_change(&id) {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
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
                    // Reload final progress from tasks.md to preserve it before archiving
                    if let Ok(progress) = task_parser::parse_change(&id) {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
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
                    // Reload progress from archived tasks.md (with fallback guard)
                    if let Ok(progress) = task_parser::parse_archived_change(&id) {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If reload fails, preserve existing progress (no action needed)
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
                self.is_resolving = true;
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
                self.is_resolving = false;
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.queue_status = QueueStatus::Merged;
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                    // Reload progress from archived tasks.md (with fallback guard)
                    if let Ok(progress) = task_parser::parse_archived_change(&change_id) {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If reload fails, preserve existing progress (no action needed)
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
                    change.queue_status = QueueStatus::Merged;
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                    // Reload progress from archived tasks.md (with fallback guard)
                    if let Ok(progress) = task_parser::parse_archived_change(&change_id) {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If reload fails, preserve existing progress (no action needed)
                }
                self.add_log(LogEntry::success(format!(
                    "Merge completed for '{}'",
                    change_id
                )));
            }
            OrchestratorEvent::ResolveFailed { change_id, error } => {
                self.is_resolving = false;
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
                // For uncommitted changes warnings in TUI, only log without popup
                if title != "Uncommitted Changes Detected" {
                    self.warning_popup = Some(super::WarningPopup {
                        title: title.clone(),
                        message: message.clone(),
                    });
                }
                self.add_log(LogEntry::warn(message));
            }
            OrchestratorEvent::Log(entry) => {
                self.add_log(entry);
            }
            OrchestratorEvent::WorktreesRefreshed { worktrees } => {
                self.worktrees = worktrees;

                // Adjust cursor if it's out of bounds
                if self.worktree_cursor_index >= self.worktrees.len() && !self.worktrees.is_empty()
                {
                    self.worktree_cursor_index = self.worktrees.len() - 1;
                }
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
            OrchestratorEvent::ApplyOutput {
                change_id,
                output,
                iteration,
            } => {
                self.add_log(
                    LogEntry::info(output)
                        .with_change_id(change_id)
                        .with_operation("apply")
                        .with_iteration(iteration.unwrap_or(1)),
                );
            }
            OrchestratorEvent::ArchiveOutput {
                change_id,
                output,
                iteration,
            } => {
                let mut entry = LogEntry::info(output)
                    .with_change_id(change_id)
                    .with_operation("archive");
                if let Some(iter) = iteration {
                    entry = entry.with_iteration(iter);
                }
                self.add_log(entry);
            }
            OrchestratorEvent::AnalysisOutput { output, iteration } => {
                let mut entry = LogEntry::info(output).with_operation("analysis");
                if let Some(iter) = iteration {
                    entry = entry.with_iteration(iter);
                }
                self.add_log(entry);
            }
            OrchestratorEvent::ResolveOutput { output, iteration } => {
                let mut entry = LogEntry::info(output).with_operation("resolve");
                if let Some(iter) = iteration {
                    entry = entry.with_iteration(iter);
                }
                self.add_log(entry);
            }
            // Branch merge events (TUI worktree view)
            OrchestratorEvent::BranchMergeStarted { branch_name } => {
                self.add_log(LogEntry::info(format!(
                    "merging branch '{}'...",
                    branch_name
                )));
                // Set is_merging flag on the worktree with this branch
                if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == *branch_name) {
                    wt.is_merging = true;
                }
            }
            OrchestratorEvent::BranchMergeCompleted { branch_name } => {
                self.add_log(LogEntry::success(format!(
                    "merged branch '{}' successfully",
                    branch_name
                )));
                // Clear is_merging flag and update has_commits_ahead
                if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == *branch_name) {
                    wt.is_merging = false;
                    wt.has_commits_ahead = false; // Merged to base, so no longer ahead
                }
            }
            OrchestratorEvent::BranchMergeFailed { branch_name, error } => {
                self.warning_popup = Some(WarningPopup {
                    title: "Merge failed".to_string(),
                    message: format!("Failed to merge '{}': {}", branch_name, error),
                });
                self.add_log(LogEntry::error(format!(
                    "Merge failed for '{}': {}",
                    branch_name, error
                )));
                // Clear is_merging flag on failure
                if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == *branch_name) {
                    wt.is_merging = false;
                }
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
                let is_merge_wait = existing.queue_status == QueueStatus::MergeWait;

                if was_archived {
                    // If change still exists after archiving, it means archive failed
                    // Revert to appropriate status based on completion
                    existing.queue_status = if fetched.is_complete() {
                        QueueStatus::Completed
                    } else {
                        QueueStatus::NotQueued
                    };
                    // Update progress for unarchived changes
                    if fetched.total_tasks > 0 {
                        existing.completed_tasks = fetched.completed_tasks;
                        existing.total_tasks = fetched.total_tasks;
                    }
                    // If fetched.total_tasks == 0, preserve existing progress
                } else if is_merge_wait {
                    // Preserve MergeWait status during auto-refresh
                    // MergeWait is a persistent state that requires explicit user action (M key)
                    // to transition to Resolving, and should not be cleared by progress updates
                    // Update progress for all states (including MergeWait)
                    if fetched.total_tasks > 0 {
                        existing.completed_tasks = fetched.completed_tasks;
                        existing.total_tasks = fetched.total_tasks;
                    }
                    // If fetched.total_tasks == 0, preserve existing progress
                } else {
                    // Update progress for all other states when valid data is available
                    // Only update if total > 0 to avoid resetting progress on retrieval failure
                    if fetched.total_tasks > 0 {
                        existing.completed_tasks = fetched.completed_tasks;
                        existing.total_tasks = fetched.total_tasks;
                    } else {
                        // fetched.total_tasks == 0: Retrieval failed, preserve existing progress
                        // For Archived/Merged changes with existing 0/0, try archive directory
                        match existing.queue_status {
                            QueueStatus::Archived | QueueStatus::Merged => {
                                if existing.completed_tasks == 0 && existing.total_tasks == 0 {
                                    // Try to read from archive as fallback
                                    if let Ok(progress) =
                                        task_parser::parse_archived_change(&fetched.id)
                                    {
                                        existing.completed_tasks = progress.completed;
                                        existing.total_tasks = progress.total;
                                    }
                                }
                            }
                            _ => {
                                // For all other states: preserve existing progress (do nothing)
                            }
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
                        | QueueStatus::Merged
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
    fn test_resolve_completed_sets_merged_and_updates_worktrees() {
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

        assert_eq!(app.changes[0].queue_status, QueueStatus::Merged);
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
    fn test_merge_completed_sets_merged_status() {
        let changes = vec![create_approved_change("change-a", 0, 5)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[0].started_at = Some(Instant::now());

        app.handle_orchestrator_event(OrchestratorEvent::MergeCompleted {
            change_id: "change-a".to_string(),
            revision: "abc123".to_string(),
        });

        assert_eq!(app.changes[0].queue_status, QueueStatus::Merged);
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

    /// Test that Completed, Archived, Merged, and Error states are retained (terminal states).
    #[test]
    fn test_terminal_states_retained_after_removal() {
        let changes = vec![
            create_approved_change("archived", 5, 5),
            create_approved_change("merged", 4, 4),
            create_approved_change("completed", 3, 3),
            create_approved_change("error", 1, 2),
        ];
        let mut app = AppState::new(changes);

        app.changes[0].queue_status = QueueStatus::Archived;
        app.changes[1].queue_status = QueueStatus::Merged;
        app.changes[2].queue_status = QueueStatus::Completed;
        app.changes[3].queue_status = QueueStatus::Error("Test error".to_string());

        // Simulate refresh with all changes removed from filesystem
        let fetched: Vec<Change> = vec![];
        app.update_changes(fetched);

        // All terminal state changes should be retained
        assert_eq!(
            app.changes.len(),
            4,
            "All terminal state changes should be retained"
        );
        assert!(matches!(app.changes[0].queue_status, QueueStatus::Archived));
        assert!(matches!(app.changes[1].queue_status, QueueStatus::Merged));
        assert!(matches!(
            app.changes[2].queue_status,
            QueueStatus::Completed
        ));
        assert!(matches!(app.changes[3].queue_status, QueueStatus::Error(_)));
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
            iteration: Some(1),
        });

        // Log should be added
        assert_eq!(app.logs.len(), initial_log_count + 1);
        let log = app.logs.last().unwrap();
        assert_eq!(log.change_id, Some("change-a".to_string()));
        assert_eq!(log.operation, Some("apply".to_string()));
        assert_eq!(log.iteration, Some(1));
        assert_eq!(log.message, "Test output line");
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
            iteration: Some(2),
        });

        // Log should be added
        assert_eq!(app.logs.len(), initial_log_count + 1);
        let log = app.logs.last().unwrap();
        assert_eq!(log.change_id, Some("change-b".to_string()));
        assert_eq!(log.operation, Some("archive".to_string()));
        assert_eq!(log.iteration, Some(2));
        assert_eq!(log.message, "Archive output line");
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
            iteration: Some(1),
        });

        // Log should be added
        assert_eq!(app.logs.len(), initial_log_count + 1);
        let log = app.logs.last().unwrap();
        assert_eq!(log.operation, Some("analysis".to_string()));
        assert_eq!(log.iteration, Some(1));
        assert!(log.message.contains("Analyzing dependencies"));
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
            iteration: Some(1),
        });

        // Log should be added
        assert_eq!(app.logs.len(), initial_log_count + 1);
        let log = app.logs.last().unwrap();
        assert_eq!(log.operation, Some("resolve".to_string()));
        assert_eq!(log.iteration, Some(1));
        assert!(log.message.contains("Resolving conflicts"));
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

    // === Tests for update-tui-task-progress-retention ===

    /// Test that ProcessingCompleted reloads and preserves final progress
    #[test]
    fn test_processing_completed_preserves_progress() {
        let changes = vec![create_approved_change("change-a", 3, 5)];
        let mut app = AppState::new(changes);

        // Simulate processing with progress updates
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[0].completed_tasks = 3;
        app.changes[0].total_tasks = 5;

        // Handle ProcessingCompleted event
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingCompleted(
            "change-a".to_string(),
        ));

        // Status should be Completed
        assert_eq!(app.changes[0].queue_status, QueueStatus::Completed);

        // Progress should be preserved (or reloaded from tasks.md if file exists)
        // Since we're in a test environment without actual files, we expect the existing progress to be kept
        assert!(app.changes[0].completed_tasks > 0 || app.changes[0].total_tasks > 0);
    }

    /// Test that ArchiveStarted reloads and preserves final progress
    #[test]
    fn test_archive_started_preserves_progress() {
        let changes = vec![create_approved_change("change-a", 5, 5)];
        let mut app = AppState::new(changes);

        // Set initial progress
        app.changes[0].completed_tasks = 5;
        app.changes[0].total_tasks = 5;

        // Handle ArchiveStarted event
        app.handle_orchestrator_event(OrchestratorEvent::ArchiveStarted("change-a".to_string()));

        // Status should be Archiving
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archiving);

        // Progress should be preserved
        assert!(app.changes[0].completed_tasks > 0 || app.changes[0].total_tasks > 0);
    }

    /// Test that Archived change is retained when removed from filesystem
    /// This is the production scenario where archived changes are moved to archive directory
    #[test]
    fn test_archived_status_retained_when_removed_from_fetched() {
        let changes = vec![create_approved_change("change-a", 7, 10)];
        let mut app = AppState::new(changes);

        // Set up archived change with progress
        app.changes[0].queue_status = QueueStatus::Archived;
        app.changes[0].completed_tasks = 7;
        app.changes[0].total_tasks = 10;

        // Simulate refresh where archived change no longer exists in filesystem
        // (it has been moved to archive directory)
        let fetched: Vec<Change> = vec![];
        app.update_changes(fetched);

        // Archived change should still be in the list with preserved progress
        assert_eq!(app.changes.len(), 1);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archived);
        assert_eq!(app.changes[0].completed_tasks, 7);
        assert_eq!(app.changes[0].total_tasks, 10);
    }

    /// Test that Merged change is retained when removed from filesystem
    #[test]
    fn test_merged_status_retained_when_removed_from_fetched() {
        let changes = vec![create_approved_change("change-a", 5, 5)];
        let mut app = AppState::new(changes);

        // Set up merged change with progress
        app.changes[0].queue_status = QueueStatus::Merged;
        app.changes[0].completed_tasks = 5;
        app.changes[0].total_tasks = 5;

        // Simulate refresh where merged change no longer exists in active changes
        let fetched: Vec<Change> = vec![];
        app.update_changes(fetched);

        // Merged change should still be in the list with preserved progress
        assert_eq!(app.changes.len(), 1);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Merged);
        assert_eq!(app.changes[0].completed_tasks, 5);
        assert_eq!(app.changes[0].total_tasks, 5);
    }

    /// Test that Archiving status preserves progress when fetched data is 0/0
    /// This scenario occurs when tasks.md is moved during archiving
    #[test]
    fn test_archiving_status_preserves_progress_on_zero_zero_update() {
        let changes = vec![create_approved_change("change-a", 7, 10)];
        let mut app = AppState::new(changes);

        // Set up archiving change with progress
        app.changes[0].queue_status = QueueStatus::Archiving;
        app.changes[0].completed_tasks = 7;
        app.changes[0].total_tasks = 10;

        // Simulate refresh with 0/0 (tasks.md is being moved during archive)
        let fetched = vec![create_approved_change("change-a", 0, 0)];
        app.update_changes(fetched);

        // Progress should be preserved (Archiving status blocks updates)
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archiving);
        assert_eq!(app.changes[0].completed_tasks, 7);
        assert_eq!(app.changes[0].total_tasks, 10);
    }

    // === Tests for update-tui-resolve-wait-status ===

    /// Test that MergeWait status is preserved during auto-refresh
    #[test]
    fn test_merge_wait_status_preserved_on_refresh() {
        // GIVEN: A change in MergeWait status
        let changes = vec![create_approved_change("change-a", 5, 10)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::MergeWait;

        // WHEN: Auto-refresh updates the change list
        let fetched = vec![create_approved_change("change-a", 7, 10)];
        app.update_changes(fetched);

        // THEN: MergeWait status is preserved (not changed to NotQueued)
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::MergeWait,
            "MergeWait status should be preserved during auto-refresh"
        );
        // AND: Progress is updated
        assert_eq!(app.changes[0].completed_tasks, 7);
        assert_eq!(app.changes[0].total_tasks, 10);
    }

    /// Test that MergeWait status is preserved even when tasks are 0/0
    #[test]
    fn test_merge_wait_status_preserved_with_zero_tasks() {
        // GIVEN: A change in MergeWait status with progress
        let changes = vec![create_approved_change("change-a", 5, 10)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::MergeWait;
        app.changes[0].completed_tasks = 5;
        app.changes[0].total_tasks = 10;

        // WHEN: Auto-refresh returns 0/0 (e.g., file moved)
        let fetched = vec![create_approved_change("change-a", 0, 0)];
        app.update_changes(fetched);

        // THEN: MergeWait status is preserved
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::MergeWait,
            "MergeWait status should be preserved even when fetched has 0/0"
        );
        // AND: Progress is preserved (not updated to 0/0)
        assert_eq!(app.changes[0].completed_tasks, 5);
        assert_eq!(app.changes[0].total_tasks, 10);
    }

    /// Test that MergeWait changes are retained when removed from filesystem
    #[test]
    fn test_merge_wait_changes_retained_after_removal() {
        // GIVEN: A change in MergeWait status
        let changes = vec![
            create_approved_change("change-a", 5, 10),
            create_approved_change("change-b", 3, 5),
        ];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::MergeWait;

        // WHEN: Auto-refresh no longer includes the MergeWait change
        let fetched = vec![create_approved_change("change-b", 3, 5)];
        app.update_changes(fetched);

        // THEN: MergeWait change is retained (not removed)
        assert_eq!(app.changes.len(), 2, "MergeWait change should be retained");
        assert_eq!(app.changes[0].id, "change-a");
        assert_eq!(app.changes[0].queue_status, QueueStatus::MergeWait);
        assert_eq!(app.changes[1].id, "change-b");
    }

    /// Test that ProgressUpdated event does not modify MergeWait status
    #[test]
    fn test_progress_updated_preserves_merge_wait_status() {
        // GIVEN: A change in MergeWait status
        let changes = vec![create_approved_change("change-a", 5, 10)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::MergeWait;

        // WHEN: ProgressUpdated event is received
        app.handle_orchestrator_event(OrchestratorEvent::ProgressUpdated {
            change_id: "change-a".to_string(),
            completed: 8,
            total: 10,
        });

        // THEN: MergeWait status is preserved (not changed)
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::MergeWait,
            "MergeWait status should not be modified by ProgressUpdated"
        );
        // AND: Progress IS updated (per update-progress-archive-resolve spec)
        assert_eq!(
            app.changes[0].completed_tasks, 8,
            "Progress should be updated for all states including MergeWait"
        );
        assert_eq!(
            app.changes[0].total_tasks, 10,
            "Total tasks should be updated for all states including MergeWait"
        );
    }

    /// Test scenario: resolve待ち状態の表示を維持する
    #[test]
    fn test_scenario_merge_wait_display_preserved() {
        // GIVEN: 変更が merge 待機状態として記録されている
        let changes = vec![create_approved_change("change-a", 10, 10)];
        let mut app = AppState::new(changes);

        // Set MergeWait status via event
        app.handle_orchestrator_event(OrchestratorEvent::MergeDeferred {
            change_id: "change-a".to_string(),
            reason: "Base working tree dirty".to_string(),
        });

        assert_eq!(app.changes[0].queue_status, QueueStatus::MergeWait);

        // WHEN: TUI が変更リストを再描画する (auto-refresh)
        let fetched = vec![create_approved_change("change-a", 10, 10)];
        app.update_changes(fetched);

        // THEN: 変更のステータスは resolve待ちとして表示される
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::MergeWait,
            "Status should remain as MergeWait"
        );
        // AND: NotQueued として表示されない (implicitly tested by above assertion)
    }

    /// Test scenario: resolve待ち状態は自動更新で保持される
    #[test]
    fn test_scenario_merge_wait_preserved_on_auto_update() {
        // GIVEN: 変更が resolve待ち状態である
        let changes = vec![create_approved_change("change-a", 10, 10)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::MergeWait;

        // WHEN: TUI が変更一覧を更新する (multiple auto-refreshes)
        for _ in 0..3 {
            let fetched = vec![create_approved_change("change-a", 10, 10)];
            app.update_changes(fetched);
        }

        // THEN: 変更の状態は resolve待ちのまま保持される
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::MergeWait,
            "MergeWait status should persist across multiple auto-refreshes"
        );
        // AND: ユーザー操作がない限りキューから外れた表示にならない
        // (MergeWait is not NotQueued, verified by above assertion)
    }

    // === Tests for update-tui-processing-progress ===

    /// Test scenario: Processing中に進捗が更新される
    #[test]
    fn test_scenario_processing_progress_updates() {
        // GIVEN: TUI が Processing 中の変更を表示している
        let changes = vec![create_approved_change("change-a", 2, 10)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Processing;

        // AND: worktree の tasks.md から進捗が取得できる (simulated by fetched data)
        // WHEN: 自動リフレッシュが実行される
        let fetched = vec![create_approved_change("change-a", 5, 10)]; // Progress updated from 2/10 to 5/10
        app.update_changes(fetched);

        // THEN: TUI は completed/total の表示を更新する
        assert_eq!(
            app.changes[0].completed_tasks, 5,
            "Completed tasks should be updated during Processing"
        );
        assert_eq!(
            app.changes[0].total_tasks, 10,
            "Total tasks should remain consistent"
        );
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Processing,
            "Status should remain Processing"
        );
    }

    /// Test scenario: Processing中に進捗取得が失敗した場合は保持する
    #[test]
    fn test_scenario_processing_progress_preserved_on_failure() {
        // GIVEN: TUI が Processing 中の変更を表示している
        let changes = vec![create_approved_change("change-a", 5, 10)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[0].completed_tasks = 5;
        app.changes[0].total_tasks = 10;

        // AND: tasks.md の読み取りに失敗する (simulated by 0/0 from auto-refresh)
        // WHEN: 自動リフレッシュが実行される
        let fetched = vec![Change {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 0, // Parse failed, no progress data
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        }];
        app.update_changes(fetched);

        // THEN: TUI は直前の completed/total 表示を維持する
        assert_eq!(
            app.changes[0].completed_tasks, 5,
            "Completed tasks should be preserved when parse fails"
        );
        assert_eq!(
            app.changes[0].total_tasks, 10,
            "Total tasks should be preserved when parse fails"
        );
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Processing,
            "Status should remain Processing"
        );
    }

    /// Test: ProgressUpdated event updates Processing changes
    #[test]
    fn test_progress_updated_event_updates_processing() {
        // GIVEN: Processing中の変更
        let changes = vec![create_approved_change("change-a", 2, 10)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Processing;

        // WHEN: ProgressUpdated イベントを受信
        app.handle_orchestrator_event(OrchestratorEvent::ProgressUpdated {
            change_id: "change-a".to_string(),
            completed: 7,
            total: 10,
        });

        // THEN: 進捗が更新される
        assert_eq!(app.changes[0].completed_tasks, 7);
        assert_eq!(app.changes[0].total_tasks, 10);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);
    }

    // === Tests for update-progress-archive-resolve ===

    /// Test that ProgressUpdated with 0/0 preserves existing progress (all states)
    #[test]
    fn test_progress_updated_zero_preserves_existing_all_states() {
        let states = vec![
            QueueStatus::NotQueued,
            QueueStatus::Queued,
            QueueStatus::Processing,
            QueueStatus::Completed,
            QueueStatus::Archiving,
            QueueStatus::Archived,
            QueueStatus::Merged,
            QueueStatus::MergeWait,
            QueueStatus::Resolving,
        ];

        for state in states {
            let changes = vec![create_approved_change("change-a", 5, 10)];
            let mut app = AppState::new(changes);
            app.changes[0].queue_status = state.clone();

            // Send ProgressUpdated with 0/0 (retrieval failure)
            app.handle_orchestrator_event(OrchestratorEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 0,
                total: 0,
            });

            // Progress should be preserved
            assert_eq!(
                app.changes[0].completed_tasks, 5,
                "State {:?}: completed_tasks should be preserved",
                state
            );
            assert_eq!(
                app.changes[0].total_tasks, 10,
                "State {:?}: total_tasks should be preserved",
                state
            );
        }
    }

    /// Test that ProgressUpdated with valid data updates all states
    #[test]
    fn test_progress_updated_valid_updates_all_states() {
        let states = vec![
            QueueStatus::NotQueued,
            QueueStatus::Queued,
            QueueStatus::Processing,
            QueueStatus::Completed,
            QueueStatus::Archiving,
            QueueStatus::Archived,
            QueueStatus::Merged,
            QueueStatus::MergeWait,
            QueueStatus::Resolving,
        ];

        for state in states {
            let changes = vec![create_approved_change("change-a", 5, 10)];
            let mut app = AppState::new(changes);
            app.changes[0].queue_status = state.clone();

            // Send ProgressUpdated with valid data
            app.handle_orchestrator_event(OrchestratorEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 8,
                total: 12,
            });

            // Progress should be updated
            assert_eq!(
                app.changes[0].completed_tasks, 8,
                "State {:?}: completed_tasks should be updated",
                state
            );
            assert_eq!(
                app.changes[0].total_tasks, 12,
                "State {:?}: total_tasks should be updated",
                state
            );
        }
    }

    /// Test that update_changes with 0/0 preserves existing progress (all states)
    #[test]
    fn test_update_changes_zero_preserves_progress_all_states() {
        let states = vec![
            QueueStatus::NotQueued,
            QueueStatus::Queued,
            QueueStatus::Processing,
            QueueStatus::Completed,
            QueueStatus::Archiving,
            QueueStatus::Archived,
            QueueStatus::Merged,
            QueueStatus::MergeWait,
            QueueStatus::Resolving,
        ];

        for state in states {
            let changes = vec![create_approved_change("change-a", 7, 10)];
            let mut app = AppState::new(changes);
            app.changes[0].queue_status = state.clone();

            // Simulate refresh with 0/0 (retrieval failure)
            let fetched = vec![Change {
                id: "change-a".to_string(),
                completed_tasks: 0,
                total_tasks: 0,
                last_modified: "now".to_string(),
                is_approved: true,
                dependencies: Vec::new(),
            }];
            app.update_changes(fetched);

            // Progress should be preserved
            assert_eq!(
                app.changes[0].completed_tasks, 7,
                "State {:?}: completed_tasks should be preserved on 0/0 refresh",
                state
            );
            assert_eq!(
                app.changes[0].total_tasks, 10,
                "State {:?}: total_tasks should be preserved on 0/0 refresh",
                state
            );
        }
    }

    /// Test that update_changes with valid data updates all states
    #[test]
    fn test_update_changes_valid_updates_all_states() {
        let states = vec![
            QueueStatus::NotQueued,
            QueueStatus::Queued,
            QueueStatus::Processing,
            QueueStatus::Completed,
            QueueStatus::Archiving,
            QueueStatus::Archived,
            QueueStatus::Merged,
            QueueStatus::MergeWait,
            QueueStatus::Resolving,
        ];

        for state in states {
            let changes = vec![create_approved_change("change-a", 5, 10)];
            let mut app = AppState::new(changes);
            app.changes[0].queue_status = state.clone();

            // Simulate refresh with valid data
            let fetched = vec![create_approved_change("change-a", 9, 12)];
            app.update_changes(fetched);

            // Progress should be updated
            assert_eq!(
                app.changes[0].completed_tasks, 9,
                "State {:?}: completed_tasks should be updated with valid data",
                state
            );
            assert_eq!(
                app.changes[0].total_tasks, 12,
                "State {:?}: total_tasks should be updated with valid data",
                state
            );
        }
    }
}
