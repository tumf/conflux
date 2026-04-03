use std::collections::HashSet;
use std::time::Instant;

use crate::task_parser;
use crate::tui::events::{LogEntry, TuiCommand};

use crate::tui::state::WarningPopup;

use super::AppState;

impl AppState {
    pub(crate) fn handle_apply_started(&mut self, change_id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.set_display_status_cache("applying");
            change.elapsed_time = None;
            change.iteration_number = None;
        }
        self.add_log(
            LogEntry::info(format!("Apply started: {}", change_id))
                .with_operation("apply")
                .with_change_id(&change_id),
        );
        self.add_log(
            LogEntry::info(format!("  Command: {}", command))
                .with_operation("apply")
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_archive_started(&mut self, id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.set_display_status_cache("archiving");
            change.iteration_number = None;
            let worktree_path = self.worktree_paths.get(&id).map(|p| p.as_path());
            if let Ok(progress) = task_parser::parse_progress_with_fallback(&id, worktree_path) {
                if progress.total > 0 {
                    change.completed_tasks = progress.completed;
                    change.total_tasks = progress.total;
                }
            }
        }
        self.add_log(
            LogEntry::info(format!("Archiving: {}", id))
                .with_operation("archive")
                .with_change_id(&id),
        );
        self.add_log(
            LogEntry::info(format!("  Command: {}", command))
                .with_operation("archive")
                .with_change_id(&id),
        );
    }

    pub(crate) fn handle_change_archived(&mut self, id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.set_display_status_cache("archived");
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            let worktree_path = self.worktree_paths.get(&id).map(|p| p.as_path());
            if let Ok(progress) = task_parser::parse_progress_with_fallback(&id, worktree_path) {
                if progress.total > 0 {
                    change.completed_tasks = progress.completed;
                    change.total_tasks = progress.total;
                }
            }
        }
        self.add_log(LogEntry::info(format!("Archived: {}", id)));
    }

    pub(crate) fn handle_resolve_started(&mut self, change_id: String, command: String) {
        self.is_resolving = true;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.set_display_status_cache("resolving");
            change.elapsed_time = None;
            change.iteration_number = None;
        }
        self.add_log(
            LogEntry::info(format!("Resolving merge for '{}'", change_id))
                .with_operation("resolve")
                .with_change_id(&change_id),
        );
        self.add_log(
            LogEntry::info(format!("  Command: {}", command))
                .with_operation("resolve")
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_analysis_started(&mut self, remaining_changes: usize) {
        self.add_log(LogEntry::info(format!(
            "Re-analyzing queued changes for dispatch (remaining: {})",
            remaining_changes
        )));
    }

    pub(crate) fn handle_resolve_completed(
        &mut self,
        change_id: String,
        worktree_change_ids: Option<HashSet<String>>,
    ) -> Option<TuiCommand> {
        self.is_resolving = false;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("merged");
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            let worktree_path = self.worktree_paths.get(&change_id).map(|p| p.as_path());
            if let Ok(progress) =
                task_parser::parse_progress_with_fallback(&change_id, worktree_path)
            {
                if progress.total > 0 {
                    change.completed_tasks = progress.completed;
                    change.total_tasks = progress.total;
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

        if let Some(next_change_id) = self.pop_from_resolve_queue() {
            self.add_log(LogEntry::info(format!(
                "Auto-starting resolve for '{}' from queue",
                next_change_id
            )));
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == next_change_id) {
                change.set_display_status_cache("resolve pending");
            }
            Some(TuiCommand::ResolveMerge(next_change_id))
        } else {
            self.try_transition_to_select();
            None
        }
    }

    pub(crate) fn handle_merge_completed(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("merged");
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            let worktree_path = self.worktree_paths.get(&change_id).map(|p| p.as_path());
            if let Ok(progress) =
                task_parser::parse_progress_with_fallback(&change_id, worktree_path)
            {
                if progress.total > 0 {
                    change.completed_tasks = progress.completed;
                    change.total_tasks = progress.total;
                }
            }
        }
        self.add_log(LogEntry::success(format!(
            "Merge completed for '{}'",
            change_id
        )));
    }

    pub(crate) fn handle_branch_merge_started(&mut self, branch_name: String) {
        self.add_log(LogEntry::info(format!(
            "merging branch '{}'...",
            branch_name
        )));
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = true;
        }
    }

    pub(crate) fn handle_branch_merge_completed(&mut self, branch_name: String) {
        self.add_log(LogEntry::success(format!(
            "merged branch '{}' successfully",
            branch_name
        )));
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = false;
            wt.has_commits_ahead = false;
        }
    }

    pub(crate) fn handle_acceptance_started(&mut self, change_id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.set_display_status_cache("accepting");
            change.iteration_number = None;
        }
        self.add_log(
            LogEntry::info(format!("Acceptance started: {}", change_id))
                .with_operation("acceptance")
                .with_change_id(&change_id),
        );
        self.add_log(
            LogEntry::info(format!("  Command: {}", command))
                .with_operation("acceptance")
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_acceptance_completed(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("archiving");
        }
        self.add_log(LogEntry::info(format!(
            "Acceptance completed: {}",
            change_id
        )));
    }

    pub(crate) fn handle_change_skipped(&mut self, change_id: String, reason: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_error_message_cache(reason.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::warn(format!("Skipped {}: {}", change_id, reason)));
    }

    pub(crate) fn handle_branch_merge_failed(&mut self, branch_name: String, error: String) {
        self.warning_popup = Some(WarningPopup {
            title: "Merge failed".to_string(),
            message: format!("Failed to merge '{}': {}", branch_name, error),
        });
        self.add_log(LogEntry::error(format!(
            "Merge failed for '{}': {}",
            branch_name, error
        )));
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = false;
        }
    }

    pub(crate) fn handle_change_stopped(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("not queued");
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::info(format!("Stopped: {}", change_id)));
    }

    pub(crate) fn handle_change_stop_failed(&mut self, change_id: String, error: String) {
        self.add_log(LogEntry::error(format!(
            "Failed to stop {}: {}",
            change_id, error
        )));
    }

    pub(crate) fn handle_dependency_blocked(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("blocked");
        }
        self.add_log(LogEntry::info(format!(
            "Change '{}' blocked by dependencies",
            change_id
        )));
    }

    pub(crate) fn handle_dependency_resolved(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.display_status_cache == "blocked" {
                change.set_display_status_cache("queued");
            }
        }
        self.add_log(LogEntry::info(format!(
            "Change '{}' dependencies resolved",
            change_id
        )));
    }
}
