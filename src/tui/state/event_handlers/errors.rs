use crate::tui::events::{LogEntry, TuiCommand};

use crate::tui::state::WarningPopup;

use super::AppState;

impl AppState {
    pub(crate) fn handle_processing_error(&mut self, id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.set_error_message_cache(error.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!("Error in {}: {}", id, error)));
        self.error_change_id = Some(id.clone());
        self.current_change = None;
    }

    pub(crate) fn handle_apply_failed(&mut self, change_id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_error_message_cache(error.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!(
            "Apply failed for {}: {}",
            change_id, error
        )));
    }

    pub(crate) fn handle_archive_failed(&mut self, change_id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_error_message_cache(error.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!(
            "Archive failed for {}: {}",
            change_id, error
        )));
    }

    pub(crate) fn handle_resolve_failed(&mut self, change_id: String, error: String) {
        self.is_resolving = false;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.display_status_cache == "merged" {
                self.add_log(LogEntry::info(format!(
                    "Ignoring ResolveFailed for '{}': already Merged",
                    change_id
                )));
                return;
            }
            change.set_display_status_cache("merge wait");
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        let message = format!("Failed to resolve merge for '{}': {}", change_id, error);
        self.warning_popup = Some(WarningPopup {
            title: "Merge resolve failed".to_string(),
            message: message.clone(),
        });
        self.add_log(LogEntry::error(message));

        self.try_transition_to_select();
    }

    pub(crate) fn handle_change_stop_failed(&mut self, change_id: String, error: String) {
        self.add_log(LogEntry::error(format!(
            "Failed to stop {}: {}",
            change_id, error
        )));
    }

    pub(crate) fn handle_merge_deferred(
        &mut self,
        change_id: String,
        reason: String,
        auto_resumable: bool,
    ) -> Option<TuiCommand> {
        if self.is_resolving {
            let is_current_resolving = self
                .changes
                .iter()
                .any(|c| c.id == change_id && c.display_status_cache == "resolving");

            if is_current_resolving {
                self.add_log(LogEntry::warn(format!(
                    "Merge deferred for '{}' (currently resolving, not queued): {}",
                    change_id, reason
                )));
            } else {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.set_display_status_cache("resolve pending");
                }
                if self.add_to_resolve_queue(&change_id) {
                    self.add_log(LogEntry::warn(format!(
                        "Merge deferred for '{}' (queued for resolve): {}",
                        change_id, reason
                    )));
                } else {
                    self.add_log(LogEntry::warn(format!(
                        "Merge deferred for '{}' (already queued): {}",
                        change_id, reason
                    )));
                }
            }
            None
        } else if auto_resumable {
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                change.set_display_status_cache("resolve pending");
            }
            self.add_to_resolve_queue(&change_id);
            self.add_log(LogEntry::warn(format!(
                "Merge deferred for '{}' (auto-resumable, starting resolve): {}",
                change_id, reason
            )));
            Some(TuiCommand::ResolveMerge(change_id))
        } else {
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                change.set_display_status_cache("merge wait");
            }
            self.add_log(LogEntry::warn(format!(
                "Merge deferred for {}: {}",
                change_id, reason
            )));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::tui::events::OrchestratorEvent;
    use crate::tui::types::AppMode;

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
    fn processing_error_keeps_app_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Running;
        app.current_change = Some("test-change".to_string());
        app.changes[0].selected = true;

        app.handle_processing_error("test-change".to_string(), "Test error message".to_string());

        assert_eq!(app.mode, AppMode::Running);
        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert_eq!(change.display_status_cache, "error");
        assert!(!change.selected);
        assert_eq!(app.error_change_id, Some("test-change".to_string()));
        assert_eq!(app.current_change, None);
    }

    #[test]
    fn processing_error_from_select_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Select;
        app.changes[0].selected = true;

        app.handle_processing_error("test-change".to_string(), "Test error message".to_string());

        assert_eq!(app.mode, AppMode::Select);
        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert_eq!(change.display_status_cache, "error");
        assert!(!change.selected);
    }

    #[test]
    fn handle_resolve_failed_does_not_demote_merged() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "merged".to_string();

        app.handle_orchestrator_event(OrchestratorEvent::ResolveFailed {
            change_id: "change-a".to_string(),
            error: "archive check failed".to_string(),
        });

        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("already Merged")));
    }

    #[test]
    fn merge_deferred_transitions_to_resolve_wait_when_resolving() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "resolving".to_string();
        app.is_resolving = true;
        app.changes[1].display_status_cache = "archived".to_string();

        app.handle_merge_deferred(
            "change-b".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(app.resolve_queue_set.contains("change-b"));
    }

    #[test]
    fn merge_deferred_does_not_queue_current_resolving_change() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "resolving".to_string();
        app.is_resolving = true;

        app.handle_merge_deferred(
            "change-a".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        assert_eq!(app.changes[0].display_status_cache, "resolving");
        assert!(!app.resolve_queue_set.contains("change-a"));
    }

    #[test]
    fn merge_deferred_queues_other_change_while_resolving() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "resolving".to_string();
        app.is_resolving = true;
        app.changes[1].display_status_cache = "archived".to_string();

        app.handle_merge_deferred(
            "change-b".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(app.resolve_queue_set.contains("change-b"));
    }

    #[test]
    fn merge_deferred_maintains_merge_wait_when_not_resolving() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.is_resolving = false;
        app.changes[0].display_status_cache = "archived".to_string();

        app.handle_merge_deferred(
            "change-a".to_string(),
            "Base branch has uncommitted changes".to_string(),
            false,
        );

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
        assert!(!app.resolve_queue_set.contains("change-a"));
    }

    #[test]
    fn auto_resumable_merge_deferred_shows_resolve_wait_not_merge_wait() {
        let changes = vec![create_test_change("change-b", 0, 1)];
        let mut app = AppState::new(changes);

        app.is_resolving = false;
        app.changes[0].display_status_cache = "archived".to_string();

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Merge in progress (MERGE_HEAD exists)".to_string(),
            true,
        );

        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
        assert!(app.resolve_queue_set.contains("change-b"));
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(ref id)) if id == "change-b"));
    }

    #[test]
    fn auto_resumable_merge_deferred_starts_resolve_when_idle() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        app.is_resolving = false;
        app.changes[0].display_status_cache = "merged".to_string();
        app.changes[1].display_status_cache = "archived".to_string();

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Base is dirty: Merge in progress (MERGE_HEAD exists)".to_string(),
            true,
        );

        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(ref id)) if id == "change-b"));
        assert!(app.resolve_queue_set.contains("change-b"));
    }

    #[test]
    fn auto_resumable_merge_deferred_queues_when_resolving() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        app.is_resolving = true;
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "archived".to_string();

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Merge in progress (MERGE_HEAD exists)".to_string(),
            true,
        );

        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(cmd.is_none());
        assert!(app.resolve_queue_set.contains("change-b"));
    }

    #[test]
    fn manual_resolve_merge_in_progress_tui_shows_resolve_wait() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "merge wait".to_string();
        app.is_resolving = false;

        app.handle_merge_deferred(
            "change-a".to_string(),
            "Base is dirty: Merge in progress (MERGE_HEAD exists)".to_string(),
            true,
        );

        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
    }

    #[test]
    fn manual_resolve_uncommitted_changes_tui_shows_merge_wait() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "merge wait".to_string();
        app.is_resolving = false;

        app.handle_resolve_failed(
            "change-a".to_string(),
            "Base is dirty: Working tree has uncommitted changes".to_string(),
        );

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
    }

    #[test]
    fn resolve_failed_transitions_to_select_when_no_active() {
        let changes = vec![create_test_change("change-a", 3, 3)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "resolving".to_string();
        app.is_resolving = true;

        app.handle_resolve_failed("change-a".to_string(), "conflict".to_string());

        assert_eq!(app.mode, AppMode::Select);
    }
}
