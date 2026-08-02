use crate::parallel::dedup::DiagnosticDeduplicationKey;
use crate::tui::events::{LogEntry, TuiCommand};

use crate::tui::state::AppState;

impl AppState {
    pub(crate) fn handle_processing_error(&mut self, id: String, error: String) {
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.set_error_message_cache(error.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!("Error in {}: {}", id, error)).with_change_id(&id));
        self.error_change_id = Some(id.clone());
        self.current_change = None;
    }

    pub(crate) fn handle_apply_failed(&mut self, change_id: String, error: String) {
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_error_message_cache(error.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(
            LogEntry::error(format!("Apply failed for {}: {}", change_id, error))
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_archive_failed(&mut self, change_id: String, error: String) {
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_error_message_cache(error.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(
            LogEntry::error(format!("Archive failed for {}: {}", change_id, error))
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_resolve_failed(&mut self, change_id: String, error: String) {
        self.reset_analysis_log_dedupe();
        self.clear_resolving();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.display_status_cache == "merged" {
                self.add_log(
                    LogEntry::info(format!(
                        "Ignoring ResolveFailed for '{}': already Merged",
                        change_id
                    ))
                    .with_change_id(&change_id),
                );
                return;
            }
            change.set_display_status_cache("merge wait");
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        let message = format!("Failed to resolve merge for '{}': {}", change_id, error);
        self.show_warning_popup("Merge resolve failed", message.clone());
        self.add_log(LogEntry::error(message).with_change_id(&change_id));

        self.try_transition_to_select();
    }

    pub(crate) fn handle_change_stop_failed(&mut self, change_id: String, error: String) {
        self.add_log(
            LogEntry::error(format!("Failed to stop {}: {}", change_id, error))
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_hook_failed(
        &mut self,
        change_id: String,
        hook_type: String,
        error: String,
    ) {
        self.reset_analysis_log_dedupe();
        if hook_type == "on_merged" {
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                if change.display_status_cache != "merged" {
                    change.set_display_status_cache("merge wait");
                    change.selected = false;
                    if let Some(started) = change.started_at {
                        change.elapsed_time = Some(started.elapsed());
                    }
                }
            }
            let message = format!(
                "on_merged hook failed for '{}'; merged transition blocked: {}",
                change_id, error
            );
            self.show_warning_popup("on_merged hook failed", message.clone());
            self.add_log(LogEntry::error(message).with_change_id(&change_id));
            self.try_transition_to_select();
        } else {
            self.add_log(
                LogEntry::error(format!(
                    "Hook '{}' failed for {}: {}",
                    hook_type, change_id, error
                ))
                .with_change_id(&change_id),
            );
        }
    }

    fn add_merge_deferred_warning_log(
        &mut self,
        change_id: &str,
        reason: &str,
        auto_resumable: bool,
        message: String,
    ) {
        let key = DiagnosticDeduplicationKey::TuiMergeDeferred {
            change_id: change_id.to_string(),
            reason: reason.to_string(),
            auto_resumable,
        };

        if !self.diagnostic_dedup.should_emit(key) {
            tracing::debug!(
                change_id = %change_id,
                auto_resumable = auto_resumable,
                reason = %reason,
                "Suppressing repeated merge-deferred TUI diagnostic"
            );
            return;
        }

        self.add_log(LogEntry::warn(message).with_change_id(change_id));
    }

    /// True when a change other than `change_id` is actually inside the resolve
    /// lifecycle.
    ///
    /// `is_resolving` alone is not evidence of an active resolve: `resolve_merge()`
    /// reserves that project-scoped slot optimistically when `M` is pressed, before
    /// `ResolveStarted` arrives. Only the reducer-derived `resolving` display proves
    /// a resolve actually started.
    fn other_change_is_resolving(&self, change_id: &str) -> bool {
        self.changes
            .iter()
            .any(|c| c.id != change_id && c.display_status_cache == "resolving")
    }

    /// Apply a manual (non auto-resumable) merge deferral.
    ///
    /// The shared reducer has already demoted the change to `MergeWait` and cleared
    /// scheduler-owned `ResolveWait` membership, so any TUI-local resolve
    /// reservation/queue entry for it is stale. Keeping it would recreate a
    /// display-only `resolve pending` row that never drains, forcing a TUI restart
    /// before the row could be retried.
    fn apply_manual_merge_deferral(&mut self, change_id: &str, reason: &str) {
        self.remove_from_resolve_queue(change_id);
        if self.is_resolving() && !self.other_change_is_resolving(change_id) {
            self.clear_resolving();
        }
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("merge wait");
        }
        self.add_merge_deferred_warning_log(
            change_id,
            reason,
            false,
            format!("Merge deferred for {}: {}", change_id, reason),
        );
    }

    pub(crate) fn handle_merge_deferred(
        &mut self,
        change_id: String,
        reason: String,
        auto_resumable: bool,
    ) -> Option<TuiCommand> {
        if !auto_resumable {
            self.apply_manual_merge_deferral(&change_id, &reason);
            return None;
        }

        if self.is_resolving() {
            let is_current_resolving = self
                .changes
                .iter()
                .any(|c| c.id == change_id && c.display_status_cache == "resolving");

            if is_current_resolving {
                self.add_merge_deferred_warning_log(
                    &change_id,
                    &reason,
                    auto_resumable,
                    format!(
                        "Merge deferred for '{}' (currently resolving, not queued): {}",
                        change_id, reason
                    ),
                );
            } else {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.set_display_status_cache("resolve pending");
                }
                if self.add_to_resolve_queue(&change_id) {
                    self.add_merge_deferred_warning_log(
                        &change_id,
                        &reason,
                        auto_resumable,
                        format!(
                            "Merge deferred for '{}' (queued for resolve): {}",
                            change_id, reason
                        ),
                    );
                } else {
                    self.add_merge_deferred_warning_log(
                        &change_id,
                        &reason,
                        auto_resumable,
                        format!(
                            "Merge deferred for '{}' (already queued): {}",
                            change_id, reason
                        ),
                    );
                }
            }
            None
        } else {
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                change.set_display_status_cache("resolve pending");
            }
            // No reservation here: the emitted command reaches the shared
            // run-control service, which takes the one reservation this change
            // is allowed to hold. Reserving first would make that a duplicate.
            self.add_merge_deferred_warning_log(
                &change_id,
                &reason,
                auto_resumable,
                format!(
                    "Merge deferred for '{}' (auto-resumable, queued scheduler retry intent): {}",
                    change_id, reason
                ),
            );
            Some(TuiCommand::ResolveMerge(change_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::tui::events::OrchestratorEvent;
    use crate::tui::types::AppMode;
    use std::collections::HashMap;

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

    /// Replay what the shared run-control service does when it accepts a resolve.
    ///
    /// These tests exercise `AppState` event handling, not the service, so its
    /// three effects — reducer retry intent, the single reservation, and the
    /// adapter's row advance — are applied directly here.
    fn accept_resolve(
        app: &mut AppState,
        shared: &std::sync::Arc<
            tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>,
        >,
        change_id: &str,
    ) {
        shared.blocking_write().apply_command(
            crate::orchestration::state::ReducerCommand::ResolveMerge(change_id.to_string()),
        );
        app.set_resolving(change_id);
        if let Some(change) = app.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("resolve pending");
        }
    }

    fn change_ids_for_message(app: &AppState, needle: &str) -> Vec<Option<String>> {
        app.logs
            .iter()
            .filter(|entry| entry.message.contains(needle))
            .map(|entry| entry.change_id.clone())
            .collect()
    }

    #[test]
    fn proposal_error_logs_carry_structured_change_id() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_processing_error("change-a".to_string(), "boom".to_string());
        app.handle_apply_failed("change-a".to_string(), "boom".to_string());
        app.handle_archive_failed("change-a".to_string(), "boom".to_string());
        app.handle_resolve_failed("change-a".to_string(), "boom".to_string());
        app.handle_change_stop_failed("change-a".to_string(), "boom".to_string());
        app.handle_hook_failed(
            "change-a".to_string(),
            "on_applied".to_string(),
            "boom".to_string(),
        );

        for needle in [
            "Error in change-a: boom",
            "Apply failed for change-a: boom",
            "Archive failed for change-a: boom",
            "Failed to resolve merge for 'change-a': boom",
            "Failed to stop change-a: boom",
            "Hook 'on_applied' failed for change-a: boom",
        ] {
            assert_eq!(
                change_ids_for_message(&app, needle),
                vec![Some("change-a".to_string())],
                "expected structured change_id on {needle}"
            );
        }
    }

    #[test]
    fn on_merged_hook_failure_and_merge_deferred_logs_carry_structured_change_id() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_hook_failed(
            "change-a".to_string(),
            "on_merged".to_string(),
            "boom".to_string(),
        );
        let _ = app.handle_merge_deferred("change-a".to_string(), "conflict".to_string(), false);

        assert_eq!(
            change_ids_for_message(&app, "on_merged hook failed for 'change-a'"),
            vec![Some("change-a".to_string())]
        );
        assert_eq!(
            change_ids_for_message(&app, "Merge deferred for change-a: conflict"),
            vec![Some("change-a".to_string())]
        );
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
    fn handle_on_merged_hook_failed_surfaces_merge_wait_not_merged() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "archived".to_string();
        let hook_error =
            "git index lock held\nstderr: fatal: unable to lock index\nstdout: retry later";
        app.handle_orchestrator_event(OrchestratorEvent::HookFailed {
            change_id: "change-a".to_string(),
            hook_type: "on_merged".to_string(),
            error: hook_error.to_string(),
        });

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
        let popup = app.warning_popup.as_ref().expect("warning popup");
        assert!(popup.message.contains(hook_error));
        assert!(popup.message.contains("git index lock held\nstderr:"));
        let logged_message = app
            .logs
            .iter()
            .find(|log| log.message.contains("merged transition blocked"))
            .map(|log| log.message.as_str())
            .expect("on_merged log entry");
        assert!(logged_message.contains("git index lock held"));
        assert!(logged_message.contains("stderr: fatal: unable to lock index"));
        assert!(logged_message.contains("stdout: retry later"));
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
        app.set_resolving("__active__");
        app.changes[1].display_status_cache = "archived".to_string();

        app.handle_merge_deferred(
            "change-b".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(app.resolve_reservations().is_reserved("change-b"));
    }

    #[test]
    fn merge_deferred_does_not_queue_current_resolving_change() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "resolving".to_string();
        app.set_resolving("__active__");

        app.handle_merge_deferred(
            "change-a".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        assert_eq!(app.changes[0].display_status_cache, "resolving");
        assert!(!app.resolve_reservations().is_reserved("change-a"));
    }

    #[test]
    fn merge_deferred_queues_other_change_while_resolving() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "resolving".to_string();
        app.set_resolving("__active__");
        app.changes[1].display_status_cache = "archived".to_string();

        app.handle_merge_deferred(
            "change-b".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(app.resolve_reservations().is_reserved("change-b"));
    }

    #[test]
    fn merge_deferred_maintains_merge_wait_when_not_resolving() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.clear_resolving();
        app.changes[0].display_status_cache = "archived".to_string();

        app.handle_merge_deferred(
            "change-a".to_string(),
            "Base branch has uncommitted changes".to_string(),
            false,
        );

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
        assert!(!app.resolve_reservations().is_reserved("change-a"));
    }

    #[test]
    fn auto_resumable_merge_deferred_shows_resolve_wait_not_merge_wait() {
        let changes = vec![create_test_change("change-b", 0, 1)];
        let mut app = AppState::new(changes);

        app.clear_resolving();
        app.changes[0].display_status_cache = "archived".to_string();

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Merge in progress (MERGE_HEAD exists)".to_string(),
            true,
        );

        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(ref id)) if id == "change-b"));
        assert!(
            !app.resolve_reservations().is_reserved("change-b"),
            "an idle deferral emits the command and lets the shared service take \
             the single reservation; reserving here would make that a duplicate"
        );
    }

    #[test]
    fn auto_resumable_merge_deferred_starts_resolve_when_idle() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        app.clear_resolving();
        app.changes[0].display_status_cache = "merged".to_string();
        app.changes[1].display_status_cache = "archived".to_string();

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Base is dirty: Merge in progress (MERGE_HEAD exists)".to_string(),
            true,
        );

        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(ref id)) if id == "change-b"));
        assert!(
            !app.resolve_reservations().is_reserved("change-b"),
            "the emitted command, not this handler, is what reserves the resolver"
        );
    }

    #[test]
    fn auto_resumable_merge_deferred_queues_when_resolving() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        app.set_resolving("__active__");
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "archived".to_string();

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Merge in progress (MERGE_HEAD exists)".to_string(),
            true,
        );

        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(cmd.is_none());
        assert!(app.resolve_reservations().is_reserved("change-b"));
    }

    #[test]
    fn duplicate_merge_deferred_warning_is_suppressed() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_merge_deferred(
            "change-a".to_string(),
            "stale workspace path".to_string(),
            false,
        );
        app.handle_merge_deferred(
            "change-a".to_string(),
            "stale workspace path".to_string(),
            false,
        );

        let matching_logs = app
            .logs
            .iter()
            .filter(|log| log.message.contains("stale workspace path"))
            .count();
        assert_eq!(matching_logs, 1);
    }

    #[test]
    fn distinct_merge_deferred_reason_is_logged_after_suppressed_duplicate() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_merge_deferred(
            "change-a".to_string(),
            "stale workspace path".to_string(),
            false,
        );
        app.handle_merge_deferred(
            "change-a".to_string(),
            "stale workspace path".to_string(),
            false,
        );
        app.handle_merge_deferred(
            "change-a".to_string(),
            "Working tree has uncommitted changes".to_string(),
            false,
        );

        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("stale workspace path")));
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Working tree has uncommitted changes")));
        assert_eq!(
            app.logs
                .iter()
                .filter(|log| log.message.contains("Merge deferred for change-a"))
                .count(),
            2
        );
    }

    #[test]
    fn manual_resolve_merge_in_progress_tui_shows_resolve_wait() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "merge wait".to_string();
        app.clear_resolving();

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
        app.clear_resolving();

        app.handle_resolve_failed(
            "change-a".to_string(),
            "Base is dirty: Working tree has uncommitted changes".to_string(),
        );

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
    }

    /// Reducer-first production ordering: `M` reserves the local resolve slot, the
    /// reducer demotes the row to MergeWait, the runner syncs that display, and only
    /// then the TUI handles `MergeDeferred(auto_resumable=false)`. The stale
    /// reservation must not be mistaken for an active resolve.
    #[test]
    fn manual_deferral_before_resolve_started_clears_optimistic_reservation() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.clear_resolving();

        let first_cmd = app.resolve_merge();
        assert!(matches!(first_cmd, Some(TuiCommand::ResolveMerge(ref id)) if id == "change-a"));

        // What the shared service and the adapter do once the command is handled:
        // the reservation exists, but `ResolveStarted` has not arrived yet.
        app.set_resolving("change-a");
        app.changes[0].set_display_status_cache("resolve pending");
        assert!(app.is_resolving());

        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "change-a".to_string(),
            "merge wait",
        )]));
        let deferred_cmd = app.handle_merge_deferred(
            "change-a".to_string(),
            "Base is dirty: Working tree has uncommitted changes".to_string(),
            false,
        );

        assert!(
            deferred_cmd.is_none(),
            "manual deferral must not dispatch another resolve"
        );
        assert_eq!(
            app.changes[0].display_status_cache, "merge wait",
            "manual deferral must preserve the reducer-derived merge wait row"
        );
        assert!(
            !app.is_resolving(),
            "stale optimistic reservation must be cleared"
        );
        assert!(!app.resolve_reservations().is_reserved("change-a"));
        assert!(app.queued_resolves().is_empty());
    }

    /// Even if the deferred change was already queued locally (for example by an
    /// earlier auto-resumable deferral), manual deferral must drain that entry while
    /// leaving unrelated FIFO entries untouched.
    #[test]
    fn manual_deferral_removes_only_its_own_local_queue_entry() {
        let mut app = AppState::new(vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
            create_test_change("change-c", 0, 1),
        ]);
        app.set_resolving("__active__");
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "resolve pending".to_string();
        app.changes[2].display_status_cache = "resolve pending".to_string();
        app.add_to_resolve_queue("change-b");
        app.add_to_resolve_queue("change-c");

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Base is dirty: Working tree has uncommitted changes".to_string(),
            false,
        );

        assert!(cmd.is_none());
        assert_eq!(app.changes[1].display_status_cache, "merge wait");
        assert!(!app.resolve_reservations().is_reserved("change-b"));
        assert_eq!(
            app.queued_resolves(),
            vec!["change-c".to_string()],
            "unrelated queued resolves must keep FIFO membership"
        );
    }

    /// A manual deferral for one change must not steal serialization from a different
    /// change that actually entered the resolve lifecycle.
    #[test]
    fn manual_deferral_preserves_serialization_for_other_resolving_change() {
        let mut app = AppState::new(vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ]);
        app.set_resolving("__active__");
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "merge wait".to_string();

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Base is dirty: Working tree has uncommitted changes".to_string(),
            false,
        );

        assert!(cmd.is_none());
        assert_eq!(app.changes[1].display_status_cache, "merge wait");
        assert!(!app.resolve_reservations().is_reserved("change-b"));
        assert!(
            app.is_resolving(),
            "serialization must remain owned by the actually resolving change"
        );
        assert_eq!(app.changes[0].display_status_cache, "resolving");
    }

    /// Auto-resumable deferrals keep queueing behind a reservation-only slot owner,
    /// so scheduler-owned retry work still shows as `resolve pending`.
    #[test]
    fn auto_resumable_merge_deferred_still_queues_behind_optimistic_reservation() {
        let mut app = AppState::new(vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ]);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.changes[1].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.clear_resolving();
        assert!(app.resolve_merge().is_some());
        // The shared service took the reservation the emitted command asked for.
        app.set_resolving("change-a");

        let cmd = app.handle_merge_deferred(
            "change-b".to_string(),
            "Merge in progress (MERGE_HEAD exists)".to_string(),
            true,
        );

        assert!(cmd.is_none(), "queued retry must wait for the active slot");
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(app.resolve_reservations().is_reserved("change-b"));
        assert!(app.is_resolving());
    }

    /// Full local lifecycle: reservation, reducer demotion, TUI handling, then a
    /// second `M` on the same `AppState` must produce fresh scheduler-consumable
    /// retry intent without restarting the TUI.
    #[test]
    fn manual_deferral_keeps_second_m_actionable_without_restart() {
        use crate::orchestration::state::{OrchestratorState, WorkspaceObservation};
        use std::sync::Arc;

        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        {
            let mut guard = shared.blocking_write();
            guard.apply_observation("change-a", WorkspaceObservation::WorkspaceArchived);
        }
        app.set_shared_state(shared.clone());
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.clear_resolving();

        // 1st M press: the emitted command reaches the shared service, which
        // records the reducer retry intent and takes the single reservation.
        assert!(app.resolve_merge().is_some());
        accept_resolve(&mut app, &shared, "change-a");
        assert_eq!(
            shared.blocking_read().resolve_wait_change_ids(),
            vec!["change-a".to_string()]
        );

        // Scheduler retry evaluation finds a dirty base before ResolveStarted.
        let display_map = {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "Base is dirty: Working tree has uncommitted changes".to_string(),
                auto_resumable: false,
            });
            guard.all_display_statuses()
        };
        app.apply_display_statuses_from_reducer(&display_map);
        let deferred_cmd = app.handle_merge_deferred(
            "change-a".to_string(),
            "Base is dirty: Working tree has uncommitted changes".to_string(),
            false,
        );

        assert!(deferred_cmd.is_none());
        assert_eq!(app.changes[0].display_status_cache, "merge wait");
        assert!(!app.is_resolving());
        assert!(app.queued_resolves().is_empty());
        assert!(shared.blocking_read().resolve_wait_change_ids().is_empty());

        // 2nd M press after the user cleaned the base: fresh retry, same AppState.
        let second_cmd = app.resolve_merge();

        assert!(
            matches!(second_cmd, Some(TuiCommand::ResolveMerge(ref id)) if id == "change-a"),
            "clean second M must dispatch a fresh retry without a TUI restart"
        );
        accept_resolve(&mut app, &shared, "change-a");
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
        assert_eq!(
            shared.blocking_read().resolve_wait_change_ids(),
            vec!["change-a".to_string()],
            "second retry must be scheduler-consumable"
        );
        assert!(app.queued_resolves().is_empty());
    }

    #[test]
    fn resolve_failed_transitions_to_select_when_no_active() {
        let changes = vec![create_test_change("change-a", 3, 3)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "resolving".to_string();
        app.set_resolving("__active__");

        app.handle_resolve_failed("change-a".to_string(), "conflict".to_string());

        assert_eq!(app.mode, AppMode::Select);
    }
}
