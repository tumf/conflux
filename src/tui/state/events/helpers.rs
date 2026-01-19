//! Helper methods for event handling
//!
//! Contains update_changes and auto_clear_merge_wait methods

use std::collections::HashSet;
use std::time::Instant;

use crate::openspec::Change;
use crate::task_parser;
use crate::tui::events::LogEntry;

use super::super::change::ChangeState;
use super::super::AppState;
use crate::tui::types::QueueStatus;

impl AppState {
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
                        // For archiving/resolving/archived/merged, try worktree fallback
                        let worktree_path =
                            self.worktree_paths.get(&fetched.id).map(|p| p.as_path());

                        match existing.queue_status {
                            QueueStatus::Archiving | QueueStatus::Resolving => {
                                // Try active location first, then archived location
                                if let Ok(progress) =
                                    task_parser::parse_change_with_worktree_fallback(
                                        &fetched.id,
                                        worktree_path,
                                    )
                                {
                                    // Only update if valid progress (not 0/0)
                                    if progress.total > 0 {
                                        existing.completed_tasks = progress.completed;
                                        existing.total_tasks = progress.total;
                                    }
                                    // If 0/0, preserve existing progress
                                } else if let Ok(progress) =
                                    task_parser::parse_archived_change_with_worktree_fallback(
                                        &fetched.id,
                                        worktree_path,
                                    )
                                {
                                    // Only update if valid progress (not 0/0)
                                    if progress.total > 0 {
                                        existing.completed_tasks = progress.completed;
                                        existing.total_tasks = progress.total;
                                    }
                                    // If 0/0, preserve existing progress
                                }
                                // If both fail or return 0/0, preserve existing progress
                            }
                            QueueStatus::Archived | QueueStatus::Merged => {
                                // Try archived location with worktree fallback
                                if let Ok(progress) =
                                    task_parser::parse_archived_change_with_worktree_fallback(
                                        &fetched.id,
                                        worktree_path,
                                    )
                                {
                                    // Only update if valid progress (not 0/0)
                                    if progress.total > 0 {
                                        existing.completed_tasks = progress.completed;
                                        existing.total_tasks = progress.total;
                                    }
                                    // If 0/0, preserve existing progress
                                }
                                // If fails or returns 0/0, preserve existing progress
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

    /// Auto-clear MergeWait status when conditions are met.
    ///
    /// Clears MergeWait to Queued when:
    /// - Worktree doesn't exist (not in worktree_change_ids), OR
    /// - Worktree exists but is not ahead of base (in worktree_not_ahead_ids)
    pub fn auto_clear_merge_wait(
        &mut self,
        worktree_change_ids: &std::collections::HashSet<String>,
        worktree_not_ahead_ids: &std::collections::HashSet<String>,
    ) {
        let mut cleared_changes = Vec::new();

        for change in &mut self.changes {
            if change.queue_status == QueueStatus::MergeWait {
                let has_worktree = worktree_change_ids.contains(&change.id);
                let not_ahead = worktree_not_ahead_ids.contains(&change.id);

                // Auto-clear conditions:
                // 1. Worktree doesn't exist
                // 2. Worktree exists but not ahead of base
                if !has_worktree || not_ahead {
                    change.queue_status = QueueStatus::Queued;
                    let reason = if !has_worktree {
                        "worktree removed"
                    } else {
                        "worktree merged to base"
                    };
                    cleared_changes.push((change.id.clone(), reason));
                }
            }
        }

        // Log after modifying changes to avoid borrow conflict
        for (id, reason) in cleared_changes {
            self.add_log(LogEntry::info(format!(
                "Auto-cleared MergeWait for '{}': {}",
                id, reason
            )));
        }
    }
}
