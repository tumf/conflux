use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::openspec::Change;
use crate::tui::types::WorktreeInfo;

use super::AppState;

impl AppState {
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
