use std::collections::HashSet;

use crate::task_parser;
use crate::tui::events::LogEntry;
use crate::tui::types::{AppMode, StopMode};

use super::AppState;

impl AppState {
    pub(crate) fn handle_processing_completed(&mut self, id: String) {
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.set_display_status_cache("archiving");
            if let Ok(progress) = task_parser::parse_change(&id) {
                change.completed_tasks = progress.completed;
                change.total_tasks = progress.total;
            }
        }
        self.add_log(LogEntry::success(format!("Completed: {}", id)).with_change_id(&id));
    }

    pub(crate) fn handle_all_completed(&mut self) {
        self.reset_analysis_log_dedupe();
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
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            if !matches!(change.display_status_cache.as_str(), "merged" | "resolving") {
                change.set_display_status_cache("archived");
            }
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
        self.add_log(LogEntry::info(format!("Archived: {}", id)).with_change_id(&id));
    }

    pub(crate) fn handle_resolve_completed(
        &mut self,
        change_id: String,
        worktree_change_ids: Option<HashSet<String>>,
    ) {
        self.reset_analysis_log_dedupe();
        let mut already_merged = false;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            already_merged = change.display_status_cache == "merged";
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
        if !already_merged {
            self.add_log(
                LogEntry::success(format!("Merge resolved for '{}'", change_id))
                    .with_change_id(&change_id),
            );
        }

        self.complete_resolve_lifecycle();
    }

    pub(crate) fn handle_merge_completed(&mut self, change_id: String) {
        self.reset_analysis_log_dedupe();
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
        self.add_log(
            LogEntry::success(format!("Merge completed for '{}'", change_id))
                .with_change_id(&change_id),
        );

        if self.is_resolving() || self.has_queued_resolves() {
            self.complete_resolve_lifecycle();
        }
    }

    /// Release the finished resolver and promote the next waiting change.
    ///
    /// This is ledger and presentation bookkeeping only. The promoted change was
    /// already given reducer-owned `ResolveWait` intent when its resolve was
    /// admitted, and the parallel executor syncs its resolve-wait set from that
    /// reducer state — so the scheduler already owns the promoted resolve and
    /// cannot exit while the intent stands. Re-submitting a `ResolveMerge`
    /// command here would only ask the shared service to admit a change whose
    /// display status is no longer `merge wait`, which it must refuse; the TUI
    /// would then show a red refusal for a resolve that is in fact proceeding.
    fn complete_resolve_lifecycle(&mut self) {
        if let Some(next_change_id) = self.pop_from_resolve_queue() {
            self.add_log(
                LogEntry::info(format!(
                    "Promoted '{}' from the resolve queue to the active resolver",
                    next_change_id
                ))
                .with_change_id(&next_change_id),
            );
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == next_change_id) {
                change.set_display_status_cache("resolve pending");
            }
        } else {
            self.try_transition_to_select();
        }
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
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("archiving");
        }
        self.add_log(
            LogEntry::info(format!("Acceptance completed: {}", change_id))
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_change_skipped(&mut self, change_id: String, reason: String) {
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_error_message_cache(reason.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(
            LogEntry::warn(format!("Skipped {}: {}", change_id, reason)).with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_branch_merge_failed(&mut self, branch_name: String, error: String) {
        self.show_warning_popup(
            "Merge failed",
            format!("Failed to merge '{}': {}", branch_name, error),
        );
        self.add_log(LogEntry::error(format!(
            "Merge failed for '{}': {}",
            branch_name, error
        )));
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = false;
        }
    }

    pub(crate) fn handle_change_stopped(&mut self, change_id: String) {
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_display_status_cache("not queued");
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::info(format!("Stopped: {}", change_id)).with_change_id(&change_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::tui::events::OrchestratorEvent;

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

    fn change_ids_for_message(app: &AppState, needle: &str) -> Vec<Option<String>> {
        app.logs
            .iter()
            .filter(|entry| entry.message.contains(needle))
            .map(|entry| entry.change_id.clone())
            .collect()
    }

    #[test]
    fn proposal_completion_skip_and_stop_logs_carry_structured_change_id() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_processing_completed("change-a".to_string());
        app.handle_change_archived("change-a".to_string());
        app.handle_acceptance_completed("change-a".to_string());
        app.handle_merge_completed("change-a".to_string());
        app.handle_change_skipped("change-a".to_string(), "dependency".to_string());
        app.handle_change_stopped("change-a".to_string());

        for needle in [
            "Completed: change-a",
            "Archived: change-a",
            "Acceptance completed: change-a",
            "Merge completed for 'change-a'",
            "Skipped change-a: dependency",
            "Stopped: change-a",
        ] {
            assert_eq!(
                change_ids_for_message(&app, needle),
                vec![Some("change-a".to_string())],
                "expected structured change_id on {needle}"
            );
        }
    }

    #[test]
    fn resolve_completion_and_queue_retry_logs_carry_structured_change_id() {
        let mut app = AppState::new(vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ]);
        app.set_resolving("change-a");
        app.add_to_resolve_queue("change-b");

        app.handle_resolve_completed("change-a".to_string(), None);

        assert_eq!(
            change_ids_for_message(&app, "Merge resolved for 'change-a'"),
            vec![Some("change-a".to_string())]
        );
        assert_eq!(
            change_ids_for_message(&app, "Promoted 'change-b' from the resolve queue"),
            vec![Some("change-b".to_string())]
        );
    }

    #[test]
    fn global_completion_log_remains_unscoped() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.mode = AppMode::Running;

        app.handle_all_completed();

        assert_eq!(
            change_ids_for_message(&app, "All changes processed successfully"),
            vec![None]
        );
    }

    #[test]
    fn branch_scoped_merge_logs_stay_unscoped_without_a_proposal_id() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_branch_merge_started("feature/change-a".to_string());
        app.handle_branch_merge_completed("feature/change-a".to_string());

        assert_eq!(
            change_ids_for_message(&app, "merging branch 'feature/change-a'"),
            vec![None]
        );
        assert_eq!(
            change_ids_for_message(&app, "merged branch 'feature/change-a' successfully"),
            vec![None]
        );
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
    fn change_archived_does_not_regress_merged_display_status() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].display_status_cache = "merged".to_string();

        app.handle_change_archived("change-a".to_string());

        assert_eq!(app.changes[0].display_status_cache, "merged");
    }

    #[test]
    fn change_archived_does_not_regress_active_resolving_display_status() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].display_status_cache = "resolving".to_string();

        app.handle_change_archived("change-a".to_string());

        assert_eq!(app.changes[0].display_status_cache, "resolving");
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

    #[test]
    fn change_dequeued_clears_selection_and_marks_not_queued() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].set_display_status_cache("applying");
        app.changes[0].selected = true;

        app.handle_orchestrator_event(OrchestratorEvent::ChangeDequeued {
            change_id: "change-a".to_string(),
        });

        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert!(!app.changes[0].selected);
        assert!(app
            .logs
            .iter()
            .any(|log| log.message == "Stopped: change-a"));
    }

    #[test]
    fn merge_completed_closes_active_resolve_lifecycle() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.set_resolving("__active__");
        app.changes[0].set_display_status_cache("resolving");

        app.handle_merge_completed("change-a".to_string());

        assert!(!app.is_resolving());
        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert!(app
            .logs
            .iter()
            .any(|log| log.message == "Merge completed for 'change-a'"));
    }

    #[test]
    fn merge_completed_drains_resolve_queue() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.set_resolving("__active__");
        app.changes[0].set_display_status_cache("resolving");
        app.changes[1].set_display_status_cache("resolve pending");
        app.add_to_resolve_queue("change-b");

        app.handle_merge_completed("change-a".to_string());

        assert!(!app.is_resolving());
        assert!(app.queued_resolves().is_empty());
        assert!(!app.resolve_reservations().is_reserved("change-b"));
        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
    }

    /// Promotion is bookkeeping, not a new submission.
    ///
    /// Both resolves are admitted through the same shared service the TUI adapter
    /// uses, so the queued change already carries reducer-owned `ResolveWait` —
    /// which is what the parallel executor actually dispatches from. Re-submitting
    /// the promoted change would ask the service to admit a row whose display
    /// status is no longer `merge wait`, and the refusal would reach the operator
    /// as a red status-bar warning for a resolve that is proceeding normally.
    #[tokio::test]
    async fn merge_completed_promotes_the_queued_resolve_without_a_refusal() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::operator_command::{
            ExecutionMarkStore, NoopQueueHooks, OperatorCommandService,
        };
        use crate::orchestration::run_control::testing::RecordingScheduler;
        use crate::orchestration::run_control::{
            ResolveReservation, ResolveReservations, RunControlOutcome, RunControlService,
            StartEligibility,
        };
        use crate::orchestration::state::{ExecutionMode, OrchestratorState};
        use crate::tui::queue::DynamicQueue;
        use std::sync::Arc;
        use tokio::sync::RwLock;

        let state = Arc::new(RwLock::new(OrchestratorState::with_mode(
            vec!["change-a".to_string(), "change-b".to_string()],
            10,
            ExecutionMode::Parallel,
        )));
        {
            let mut guard = state.write().await;
            for id in ["change-a", "change-b"] {
                guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
                    change_id: id.to_string(),
                    reason: "manual resolution required".to_string(),
                    auto_resumable: false,
                });
            }
        }

        let resolves = Arc::new(ResolveReservations::new());
        let queue = DynamicQueue::new();
        let run_control = RunControlService::new(
            state.clone(),
            Arc::new(OperatorCommandService::new(
                state.clone(),
                Arc::new(queue.clone()),
                Arc::new(NoopQueueHooks),
                Arc::new(ExecutionMarkStore::new()),
            )),
            Arc::new(RecordingScheduler::new()),
            resolves.clone(),
            Arc::new(StartEligibility::new()),
        );

        assert!(matches!(
            run_control.resolve_merge("change-a").await,
            Ok(RunControlOutcome::ResolveReserved {
                reservation: ResolveReservation::Active,
                ..
            })
        ));
        assert!(matches!(
            run_control.resolve_merge("change-b").await,
            Ok(RunControlOutcome::ResolveReserved {
                reservation: ResolveReservation::Queued { .. },
                ..
            })
        ));

        let mut app = AppState::new(vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ]);
        app.mode = AppMode::Running;
        app.set_resolve_reservations(resolves.clone());
        app.set_shared_state(state.clone());
        app.apply_display_statuses_from_reducer(&state.read().await.all_display_statuses());
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");

        // The runtime applies the event to the reducer before the TUI sees it.
        let merged = ExecutionEvent::MergeCompleted {
            change_id: "change-a".to_string(),
            revision: "1".to_string(),
        };
        state.write().await.apply_execution_event(&merged);
        let command = app.handle_orchestrator_event(merged);

        assert!(
            command.is_none(),
            "promotion must emit no command the shared service would have to refuse"
        );
        assert_eq!(
            app.warning_message, None,
            "a promoted resolve is not an operator-facing refusal"
        );
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert_eq!(
            state.read().await.resolve_wait_change_ids(),
            vec!["change-b".to_string()],
            "the promoted change keeps the reducer intent the scheduler dispatches from"
        );
        assert!(app.queued_resolves().is_empty());
    }

    #[test]
    fn merge_completed_preserves_non_resolve_behavior() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].set_display_status_cache("merge wait");
        app.changes[0].started_at = Some(std::time::Instant::now());

        app.handle_merge_completed("change-a".to_string());

        assert!(!app.is_resolving());
        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert!(app.changes[0].elapsed_time.is_some());
        assert!(app
            .logs
            .iter()
            .any(|log| log.message == "Merge completed for 'change-a'"));
    }
}
