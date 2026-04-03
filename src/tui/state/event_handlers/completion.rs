use std::collections::HashSet;

use crate::task_parser;
use crate::tui::events::{LogEntry, TuiCommand};
use crate::tui::types::{AppMode, StopMode};

use crate::tui::state::WarningPopup;

use super::AppState;

impl AppState {
    pub(crate) fn handle_processing_completed(&mut self, id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.set_display_status_cache("archiving");
            if let Ok(progress) = task_parser::parse_change(&id) {
                change.completed_tasks = progress.completed;
                change.total_tasks = progress.total;
            }
        }
        self.add_log(LogEntry::success(format!("Completed: {}", id)));
    }

    pub(crate) fn handle_all_completed(&mut self) {
        if matches!(self.mode, AppMode::Stopped | AppMode::Error) {
            if let Some(started) = self.orchestration_started_at {
                self.orchestration_elapsed = Some(started.elapsed());
            }
            return;
        }

        for change in &mut self.changes {
            if matches!(change.display_status_cache.as_str(), "queued" | "blocked") {
                change.set_display_status_cache("not queued");
            }
        }

        self.mode = AppMode::Select;
        self.current_change = None;
        self.stop_mode = StopMode::None;
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }
        self.add_log(LogEntry::success("All changes processed successfully"));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    #[test]
    fn processing_completed_updates_status() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_processing_completed("test-change".to_string());

        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert_eq!(change.display_status_cache, "archiving");
    }

    #[test]
    fn all_completed_transitions_to_select() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Running;
        app.handle_all_completed();

        assert_eq!(app.mode, AppMode::Select);
        assert_eq!(app.current_change, None);
    }

    #[test]
    fn all_completed_preserves_error_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Error;
        app.handle_all_completed();

        assert_eq!(app.mode, AppMode::Error);
    }

    #[test]
    fn all_completed_keeps_stopped_mode() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;

        app.handle_all_completed();

        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn all_completed_resets_blocked_and_queued_to_not_queued() {
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[0].selected = true;
        app.changes[1].display_status_cache = "blocked".to_string();
        app.changes[1].selected = true;

        app.handle_all_completed();

        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert_eq!(app.changes[1].display_status_cache, "not queued");
        assert_eq!(app.mode, AppMode::Select);
    }
}
