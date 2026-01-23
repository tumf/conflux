//! Stage event handlers
//!
//! Handles ApplyStarted, ArchiveStarted, ChangeArchived, ResolveStarted, ResolveCompleted, MergeCompleted
//! BranchMergeStarted, BranchMergeCompleted

use std::collections::HashSet;
use std::time::Instant;

use crate::task_parser;
use crate::tui::events::LogEntry;
use crate::tui::types::QueueStatus;

use super::super::AppState;

impl AppState {
    /// Handle ApplyStarted event
    pub(super) fn handle_apply_started(&mut self, change_id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Applying;
            change.elapsed_time = None;
        }
        self.add_log(LogEntry::info(format!("Apply started: {}", change_id)));
        self.add_log(LogEntry::info(format!("  Command: {}", command)));
    }

    /// Handle ArchiveStarted event
    pub(super) fn handle_archive_started(&mut self, id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Archiving;
            // Reload final progress from tasks.md to preserve it before archiving
            // Use comprehensive fallback to read from uncommitted changes
            let worktree_path = self.worktree_paths.get(&id).map(|p| p.as_path());
            match task_parser::parse_progress_with_fallback(&id, worktree_path) {
                Ok(progress) => {
                    // Only update if valid progress (not 0/0)
                    if progress.total > 0 {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If 0/0, preserve existing progress
                }
                Err(_) => {
                    // Preserve existing progress if parsing fails
                }
            }
        }
        self.add_log(LogEntry::info(format!("Archiving: {}", id)));
        self.add_log(LogEntry::info(format!("  Command: {}", command)));
    }

    /// Handle ChangeArchived event
    pub(super) fn handle_change_archived(&mut self, id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Archived;
            // Record final elapsed time
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            // Reload progress from archived tasks.md (with fallback guard)
            // Use comprehensive fallback to read from uncommitted archive
            let worktree_path = self.worktree_paths.get(&id).map(|p| p.as_path());
            match task_parser::parse_progress_with_fallback(&id, worktree_path) {
                Ok(progress) => {
                    // Only update if valid progress (not 0/0)
                    if progress.total > 0 {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If 0/0, preserve existing progress
                }
                Err(_) => {
                    // Preserve existing progress if parsing fails
                }
            }
        }
        self.add_log(LogEntry::info(format!("Archived: {}", id)));
    }

    /// Handle ResolveStarted event
    pub(super) fn handle_resolve_started(&mut self, change_id: String, command: String) {
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
        self.add_log(LogEntry::info(format!("  Command: {}", command)));
    }

    /// Handle ResolveCompleted event
    pub(super) fn handle_resolve_completed(
        &mut self,
        change_id: String,
        worktree_change_ids: Option<HashSet<String>>,
    ) {
        self.is_resolving = false;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Merged;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            // Reload progress from archived tasks.md (with fallback guard)
            // Use comprehensive fallback to read from uncommitted archive
            let worktree_path = self.worktree_paths.get(&change_id).map(|p| p.as_path());
            match task_parser::parse_progress_with_fallback(&change_id, worktree_path) {
                Ok(progress) => {
                    // Only update if valid progress (not 0/0)
                    if progress.total > 0 {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If 0/0, preserve existing progress
                }
                Err(_) => {
                    // Preserve existing progress if parsing fails
                }
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

    /// Handle MergeCompleted event
    pub(super) fn handle_merge_completed(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Merged;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            // Reload progress from archived tasks.md (with fallback guard)
            // Use comprehensive fallback to read from uncommitted archive
            let worktree_path = self.worktree_paths.get(&change_id).map(|p| p.as_path());
            match task_parser::parse_progress_with_fallback(&change_id, worktree_path) {
                Ok(progress) => {
                    // Only update if valid progress (not 0/0)
                    if progress.total > 0 {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If 0/0, preserve existing progress
                }
                Err(_) => {
                    // Preserve existing progress if parsing fails
                }
            }
        }
        self.add_log(LogEntry::success(format!(
            "Merge completed for '{}'",
            change_id
        )));
    }

    /// Handle BranchMergeStarted event
    pub(super) fn handle_branch_merge_started(&mut self, branch_name: String) {
        self.add_log(LogEntry::info(format!(
            "merging branch '{}'...",
            branch_name
        )));
        // Set is_merging flag on the worktree with this branch
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = true;
        }
    }

    /// Handle BranchMergeCompleted event
    pub(super) fn handle_branch_merge_completed(&mut self, branch_name: String) {
        self.add_log(LogEntry::success(format!(
            "merged branch '{}' successfully",
            branch_name
        )));
        // Clear is_merging flag and update has_commits_ahead
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = false;
            wt.has_commits_ahead = false; // Merged to base, so no longer ahead
        }
    }
}
