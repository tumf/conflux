use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::openspec::Change;
use crate::tui::events::LogEntry;
use crate::tui::types::WorktreeInfo;

use super::AppState;

fn is_refresh_merge_wait_terminal_status(status: &str) -> bool {
    matches!(status, "archived" | "merged" | "rejected")
}

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
        merge_wait_ids: HashSet<String>,
    ) {
        let terminal_merge_wait_statuses = self.terminal_merge_wait_statuses(&merge_wait_ids);

        self.worktree_paths = worktree_paths;
        self.update_changes(changes);
        self.apply_parallel_eligibility(&committed_change_ids, &uncommitted_file_change_ids);
        self.apply_worktree_status(&worktree_change_ids);
        self.apply_refresh_merge_wait_status(&merge_wait_ids, &terminal_merge_wait_statuses);
    }

    fn terminal_merge_wait_statuses(
        &self,
        merge_wait_ids: &HashSet<String>,
    ) -> HashMap<String, String> {
        self.changes
            .iter()
            .filter(|change| {
                merge_wait_ids.contains(&change.id)
                    && is_refresh_merge_wait_terminal_status(&change.display_status_cache)
            })
            .map(|change| (change.id.clone(), change.display_status_cache.clone()))
            .collect()
    }

    fn apply_refresh_merge_wait_status(
        &mut self,
        merge_wait_ids: &HashSet<String>,
        terminal_merge_wait_statuses: &HashMap<String, String>,
    ) {
        if merge_wait_ids.is_empty() {
            return;
        }

        for change in &mut self.changes {
            if !merge_wait_ids.contains(&change.id) {
                continue;
            }

            if let Some(terminal_status) = terminal_merge_wait_statuses.get(&change.id) {
                change.set_display_status_cache(terminal_status);
                continue;
            }

            if is_refresh_merge_wait_terminal_status(&change.display_status_cache) {
                continue;
            }

            change.set_display_status_cache("merge wait");
        }
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

    type RefreshSets = (
        HashSet<String>,
        HashSet<String>,
        HashSet<String>,
        HashMap<String, PathBuf>,
        HashSet<String>,
    );

    fn empty_refresh_sets() -> RefreshSets {
        (
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashMap::new(),
            HashSet::new(),
        )
    }

    #[test]
    fn merge_wait_refresh_corrects_stale_resolve_pending_row() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.changes[0].set_display_status_cache("resolve pending");

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![create_test_change("change-a")],
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from(["change-a".to_string()]),
        );

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
    }

    #[test]
    fn merge_wait_refresh_preserves_terminal_rows() {
        let mut app = AppState::new(vec![
            create_test_change("merged-change"),
            create_test_change("rejected-change"),
        ]);
        app.changes[0].set_display_status_cache("merged");
        app.changes[1].set_display_status_cache("rejected");

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![
                create_test_change("merged-change"),
                create_test_change("rejected-change"),
            ],
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from(["merged-change".to_string(), "rejected-change".to_string()]),
        );

        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert_eq!(app.changes[1].display_status_cache, "rejected");
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
