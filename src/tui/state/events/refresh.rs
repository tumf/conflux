//! Refresh event handlers
//!
//! Handles ChangesRefreshed and WorktreesRefreshed events

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::openspec::Change;
use crate::tui::types::WorktreeInfo;

use super::super::AppState;

impl AppState {
    /// Handle ChangesRefreshed event
    pub(super) fn handle_changes_refreshed(
        &mut self,
        changes: Vec<Change>,
        committed_change_ids: HashSet<String>,
        worktree_change_ids: HashSet<String>,
        worktree_paths: HashMap<String, PathBuf>,
        worktree_not_ahead_ids: HashSet<String>,
        resolve_wait_ids: HashSet<String>,
    ) {
        self.worktree_paths = worktree_paths;
        self.update_changes(changes);
        self.apply_parallel_eligibility(&committed_change_ids);
        self.apply_worktree_status(&worktree_change_ids);
        // Auto-clear MergeWait for changes whose worktrees don't exist or are not ahead
        self.auto_clear_merge_wait(&worktree_change_ids, &worktree_not_ahead_ids);
        // Apply ResolveWait status for archived changes waiting for resolve
        self.apply_resolve_wait_status(&resolve_wait_ids);
    }

    /// Handle WorktreesRefreshed event
    pub(super) fn handle_worktrees_refreshed(&mut self, worktrees: Vec<WorktreeInfo>) {
        self.worktrees = worktrees;

        // Adjust cursor if it's out of bounds
        if self.worktree_cursor_index >= self.worktrees.len() && !self.worktrees.is_empty() {
            self.worktree_cursor_index = self.worktrees.len() - 1;
        }
    }
}
