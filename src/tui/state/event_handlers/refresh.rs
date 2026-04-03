use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::openspec::Change;
use crate::tui::events::LogEntry;
use crate::tui::types::WorktreeInfo;

use super::AppState;

impl AppState {
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
