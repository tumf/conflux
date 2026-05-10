use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::openspec::Change;
use crate::tui::events::LogEntry;
use crate::tui::types::WorktreeInfo;

use super::AppState;

impl AppState {
    pub(crate) fn handle_dependency_blocked(&mut self, change_id: String) {
        let was_already_blocked = self
            .changes
            .iter_mut()
            .find(|c| c.id == change_id)
            .map(|change| {
                let was_blocked = change.display_status_cache == "blocked";
                change.set_display_status_cache("blocked");
                was_blocked
            })
            .unwrap_or(false);

        if was_already_blocked {
            tracing::debug!(
                change_id = %change_id,
                "Suppressing repeated dependency-blocked TUI log"
            );
            return;
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
        self.reset_analysis_log_dedupe();
        self.add_log(LogEntry::info(format!(
            "Change '{}' dependencies resolved",
            change_id
        )));
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_changes_refreshed(
        &mut self,
        changes: Vec<Change>,
        committed_change_ids: HashSet<String>,
        uncommitted_file_change_ids: HashSet<String>,
        worktree_change_ids: HashSet<String>,
        worktree_paths: HashMap<String, PathBuf>,
        _worktree_not_ahead_ids: HashSet<String>,
        _merge_wait_ids: HashSet<String>,
    ) {
        self.worktree_paths = worktree_paths;
        self.update_changes(changes);
        self.apply_parallel_eligibility(&committed_change_ids, &uncommitted_file_change_ids);
        self.apply_worktree_status(&worktree_change_ids);
    }

    pub(crate) fn handle_worktrees_refreshed(&mut self, worktrees: Vec<WorktreeInfo>) {
        self.worktrees = worktrees;

        if self.worktree_cursor_index >= self.worktrees.len() && !self.worktrees.is_empty() {
            self.worktree_cursor_index = self.worktrees.len() - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    fn count_blocked_logs(app: &AppState, change_id: &str) -> usize {
        let message = format!("Change '{}' blocked by dependencies", change_id);
        app.logs
            .iter()
            .filter(|entry| entry.message == message)
            .count()
    }

    #[test]
    fn repeated_dependency_blocked_updates_status_without_duplicate_log() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        app.handle_dependency_blocked("change-a".to_string());
        app.handle_dependency_blocked("change-a".to_string());

        assert_eq!(app.changes[0].display_status_cache, "blocked");
        assert_eq!(count_blocked_logs(&app, "change-a"), 1);
    }

    #[test]
    fn dependency_resolved_then_reblocked_logs_again() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        app.handle_dependency_blocked("change-a".to_string());
        app.handle_dependency_resolved("change-a".to_string());
        app.handle_dependency_blocked("change-a".to_string());

        assert_eq!(app.changes[0].display_status_cache, "blocked");
        assert_eq!(count_blocked_logs(&app, "change-a"), 2);
    }
}
