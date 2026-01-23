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
    /// Updates task progress (completed_tasks, total_tasks) from fetched changes and
    /// enriches change metadata from shared orchestration state when available (apply counts,
    /// pending/archived tracking).
    ///
    /// IMPORTANT: This method does NOT modify queue_status. In Stopped mode, task completion
    /// does not trigger auto-queue. Changes are only queued through explicit user action (Space key).
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
                let is_resolve_wait = existing.queue_status == QueueStatus::ResolveWait;

                if was_archived {
                    // If change still exists after archiving, it means archive failed
                    // Revert to NotQueued status
                    existing.queue_status = QueueStatus::NotQueued;
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
                } else if is_resolve_wait {
                    // Preserve ResolveWait status during auto-refresh
                    // ResolveWait is a persistent state indicating archive is complete
                    // and the change is waiting for resolve execution
                    // Update progress for ResolveWait changes
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
                            QueueStatus::Archiving
                            | QueueStatus::Resolving
                            | QueueStatus::Archived
                            | QueueStatus::Merged => {
                                // Use comprehensive fallback: worktree active -> worktree archive -> base active -> base archive
                                if let Ok(progress) = task_parser::parse_progress_with_fallback(
                                    &fetched.id,
                                    worktree_path,
                                ) {
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

        // Enrich change metadata from shared orchestration state if available
        // This provides apply counts (iteration_number) for display
        if let Some(shared_state) = &self.shared_orchestrator_state {
            // Attempt to read shared state, but don't block if lock is held
            if let Ok(guard) = shared_state.try_read() {
                for change in &mut self.changes {
                    // Set iteration_number from apply_count if available
                    let apply_count = guard.apply_count(&change.id);
                    if apply_count > 0 {
                        change.iteration_number = Some(apply_count);
                    }
                }
            }
        }

        // Remove changes that no longer exist (have been archived externally)
        let current_ids: HashSet<String> = fetched_changes.iter().map(|c| c.id.clone()).collect();
        self.changes.retain(|c| {
            // Keep if still exists, apply started in this session, or in a terminal state.
            current_ids.contains(&c.id)
                || c.started_at.is_some()
                || matches!(
                    c.queue_status,
                    QueueStatus::Archiving
                        | QueueStatus::Archived
                        | QueueStatus::Merged
                        | QueueStatus::MergeWait
                        | QueueStatus::Resolving
                        | QueueStatus::ResolveWait
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

    /// Apply ResolveWait status for changes detected in WorkspaceState::Archived.
    ///
    /// Sets ResolveWait for changes that:
    /// - Are currently in NotQueued status (from auto-refresh reset)
    /// - Have a worktree in WorkspaceState::Archived state
    pub fn apply_resolve_wait_status(
        &mut self,
        resolve_wait_ids: &std::collections::HashSet<String>,
    ) {
        for change in &mut self.changes {
            if resolve_wait_ids.contains(&change.id) {
                // Only set ResolveWait if currently NotQueued or Archived
                // (to avoid overwriting active processing states)
                if matches!(
                    change.queue_status,
                    QueueStatus::NotQueued | QueueStatus::Archived
                ) {
                    change.queue_status = QueueStatus::ResolveWait;
                }
            }
        }
    }
}
