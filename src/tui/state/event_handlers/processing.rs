use std::time::Instant;

use crate::parallel::dedup::DiagnosticDeduplicationKey;
use crate::task_parser;
use crate::tui::events::LogEntry;
use crate::tui::types::{AppMode, StopMode};

use super::AppState;

impl AppState {
    pub(crate) fn handle_processing_started(&mut self, id: String) {
        self.reset_analysis_log_dedupe();
        self.current_change = Some(id.clone());
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.set_display_status_cache("applying");
            change.started_at = Some(Instant::now());
            change.elapsed_time = None;
        }
        self.add_log(LogEntry::info(format!("Processing: {}", id)).with_change_id(&id));
    }

    pub(crate) fn handle_apply_started(&mut self, change_id: String, command: String) {
        self.reset_analysis_log_dedupe();
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
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            if change.display_status_cache == "merged" {
                tracing::debug!(
                    change_id = %id,
                    "Ignoring stale ArchiveStarted event for row already displayed as merged"
                );
            } else {
                if change.started_at.is_none() {
                    change.started_at = Some(Instant::now());
                }
                change.set_display_status_cache("archiving");
                change.iteration_number = None;
            }
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

    pub(crate) fn handle_resolve_started(&mut self, change_id: String, command: String) {
        self.reset_analysis_log_dedupe();
        // The runtime, not the frontend, decides which change owns the resolver:
        // scheduler-owned resolves never went through a reservation request.
        self.set_resolving(&change_id);
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

    pub(crate) fn handle_analysis_started(&mut self, remaining_changes: usize, attempt_id: String) {
        let key = DiagnosticDeduplicationKey::TuiAnalysisStarted {
            remaining_changes,
            attempt_id,
        };
        if !self.diagnostic_dedup.should_emit(key) {
            tracing::debug!(
                remaining_changes = remaining_changes,
                "Suppressing repeated analysis-started TUI log"
            );
            return;
        }

        self.add_log(LogEntry::info(format!(
            "Re-analyzing queued changes for dispatch (remaining: {})",
            remaining_changes
        )));
    }

    pub(crate) fn handle_acceptance_started(&mut self, change_id: String, command: String) {
        self.reset_analysis_log_dedupe();
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
            LogEntry::info(format!(
                "  {}",
                crate::events::command_log_summary(&command)
            ))
            .with_operation("acceptance")
            .with_change_id(&change_id),
        );
    }

    /// Transition to `AppMode::Select` if no active changes remain.
    ///
    /// "Active" means any change is still in a processing queue status:
    /// Queued, Blocked, Applying, Accepting, Archiving, Resolving, or ResolveWait.
    pub(crate) fn try_transition_to_select(&mut self) {
        if !matches!(self.mode, AppMode::Running) {
            return;
        }

        let has_active = self.changes.iter().any(|c| {
            matches!(
                c.display_status_cache.as_str(),
                "queued"
                    | "blocked"
                    | "applying"
                    | "accepting"
                    | "archiving"
                    | "resolving"
                    | "resolve pending"
            )
        });

        if !has_active {
            tracing::info!("No active changes remaining after resolve; transitioning to Select");
            self.mode = AppMode::Select;
            self.current_change = None;
            self.stop_mode = StopMode::None;
            if let Some(started) = self.orchestration_started_at {
                self.orchestration_elapsed = Some(started.elapsed());
            }
            self.add_log(LogEntry::success("All changes processed successfully"));
        }
    }

    /// Apply a terminal `Stopped` transition.
    ///
    /// The first transition into `AppMode::Stopped` owns the terminal
    /// `Processing stopped` message. A repeated or late `Stopped` delivery (for
    /// example the scheduler's own cancellation event arriving after the
    /// frontend already applied the stop) still reconciles queue and mode state,
    /// but must not append a duplicate terminal message.
    pub(crate) fn handle_stopped(&mut self) {
        let already_stopped = matches!(self.mode, AppMode::Stopped);
        self.reset_analysis_log_dedupe();
        self.mode = AppMode::Stopped;
        self.current_change = None;
        self.stop_mode = StopMode::None;
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }

        for change in &mut self.changes {
            if matches!(
                change.display_status_cache.as_str(),
                "applying" | "accepting" | "archiving" | "resolving" | "queued" | "blocked"
            ) {
                if let Some(started) = change.started_at {
                    change.elapsed_time = Some(started.elapsed());
                }
                change.set_display_status_cache("not queued");
            }
        }
        if !already_stopped {
            self.add_log(LogEntry::warn("Processing stopped"));
        }
    }

    pub(crate) fn handle_progress_updated(
        &mut self,
        change_id: String,
        completed: u32,
        total: u32,
    ) {
        self.reset_analysis_log_dedupe();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if total > 0 {
                change.completed_tasks = completed;
                change.total_tasks = total;
            }
        }
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

    fn count_logs(app: &AppState, needle: &str) -> usize {
        app.logs
            .iter()
            .filter(|entry| entry.message.contains(needle))
            .count()
    }

    fn count_analysis_logs(app: &AppState, remaining_changes: usize) -> usize {
        let message = format!(
            "Re-analyzing queued changes for dispatch (remaining: {})",
            remaining_changes
        );
        app.logs
            .iter()
            .filter(|entry| entry.message == message)
            .count()
    }

    #[test]
    fn repeated_analysis_started_with_same_remaining_count_logs_once() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_analysis_started(1, "attempt-a".to_string());
        app.handle_analysis_started(1, "attempt-a".to_string());

        assert_eq!(count_analysis_logs(&app, 1), 1);
    }

    #[test]
    fn analysis_started_logs_again_when_remaining_count_changes() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_analysis_started(1, "attempt-a".to_string());
        app.handle_analysis_started(2, "attempt-b".to_string());

        assert_eq!(count_analysis_logs(&app, 1), 1);
        assert_eq!(count_analysis_logs(&app, 2), 1);
    }

    #[test]
    fn analysis_started_logs_again_after_progress_reset() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_analysis_started(1, "attempt-a".to_string());
        app.handle_progress_updated("change-a".to_string(), 1, 1);
        app.handle_analysis_started(1, "attempt-a".to_string());

        assert_eq!(count_analysis_logs(&app, 1), 2);
    }

    #[test]
    fn distinct_same_count_analysis_attempts_both_log_after_merge_wait_queueing() {
        let mut app = AppState::new(vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ]);

        app.handle_analysis_started(1, "iteration=1;trigger=initial;queued=change-a".to_string());
        let _ = app.handle_merge_deferred("change-a".to_string(), "merge wait".to_string(), true);
        app.handle_analysis_started(1, "iteration=1;trigger=queue;queued=change-b".to_string());

        assert_eq!(count_analysis_logs(&app, 1), 2);
    }

    #[test]
    fn same_count_analysis_with_distinct_attempt_id_logs_again() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_analysis_started(1, "attempt-a".to_string());
        app.handle_analysis_started(1, "attempt-b".to_string());

        assert_eq!(count_analysis_logs(&app, 1), 2);
    }

    fn change_ids_for_message(app: &AppState, needle: &str) -> Vec<Option<String>> {
        app.logs
            .iter()
            .filter(|entry| entry.message.contains(needle))
            .map(|entry| entry.change_id.clone())
            .collect()
    }

    #[test]
    fn proposal_lifecycle_start_logs_carry_structured_change_id() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        app.handle_processing_started("change-a".to_string());
        app.handle_apply_started("change-a".to_string(), "run".to_string());
        app.handle_archive_started("change-a".to_string(), "run".to_string());
        app.handle_resolve_started("change-a".to_string(), "run".to_string());
        app.handle_acceptance_started("change-a".to_string(), "run".to_string());

        for needle in [
            "Processing: change-a",
            "Apply started: change-a",
            "Archiving: change-a",
            "Resolving merge for 'change-a'",
            "Acceptance started: change-a",
        ] {
            assert_eq!(
                change_ids_for_message(&app, needle),
                vec![Some("change-a".to_string())],
                "expected structured change_id on {needle}"
            );
        }
    }

    #[test]
    fn global_orchestration_logs_remain_unscoped() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.mode = AppMode::Running;

        app.handle_analysis_started(1, "attempt-a".to_string());
        app.try_transition_to_select();
        app.handle_stopped();

        assert_eq!(
            change_ids_for_message(&app, "Re-analyzing queued changes for dispatch"),
            vec![None]
        );
        assert_eq!(
            change_ids_for_message(&app, "All changes processed successfully"),
            vec![None]
        );
        assert_eq!(
            change_ids_for_message(&app, "Processing stopped"),
            vec![None]
        );
    }

    #[test]
    fn processing_started_sets_current_change_and_applying_state() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_processing_started("change-a".to_string());

        assert_eq!(app.current_change, Some("change-a".to_string()));
        let change = app.changes.iter().find(|c| c.id == "change-a").unwrap();
        assert_eq!(change.display_status_cache, "applying");
        assert!(change.started_at.is_some());
    }

    #[test]
    fn acceptance_started_logs_command_metadata_without_prompt() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        let command = format!("claude --print '{}'", "secret prompt".repeat(1000));

        app.handle_acceptance_started("change-a".to_string(), command.clone());

        let entry = app.logs.last().expect("command metadata log");
        assert!(entry.message.contains("Command metadata:"));
        assert!(entry.message.contains(&format!("bytes={}", command.len())));
        assert!(entry.message.contains("hash="));
        assert!(!entry.message.contains("secret prompt"));
    }

    #[test]
    fn stopped_resets_display_status_cache() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[0].selected = true;

        app.handle_stopped();

        assert_eq!(app.mode, AppMode::Stopped);
        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert!(app.changes[0].selected);
    }

    #[test]
    fn handle_stopped_resets_blocked_to_not_queued() {
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "applying".to_string();
        app.changes[0].selected = true;
        app.changes[1].display_status_cache = "blocked".to_string();
        app.changes[1].selected = true;

        app.handle_stopped();

        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert_eq!(app.changes[1].display_status_cache, "not queued");
        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn stopped_resets_resolving_changes() {
        let changes = vec![
            create_test_change("change-a", 3, 3),
            create_test_change("change-b", 2, 4),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[0].selected = true;
        app.changes[1].display_status_cache = "merged".to_string();

        app.handle_stopped();

        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert!(app.changes[0].selected);
        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn try_transition_to_select_no_op_when_not_running() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;

        app.try_transition_to_select();

        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn try_transition_to_select_stays_running_with_active() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "applying".to_string();

        app.try_transition_to_select();

        assert_eq!(app.mode, AppMode::Running);
    }

    #[test]
    fn idle_parallel_stop_first_stopped_transition_owns_the_terminal_message() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.mode = AppMode::Running;
        app.changes[0].set_display_status_cache("applying");
        app.changes[0].selected = true;

        app.handle_stopped();

        assert_eq!(app.mode, AppMode::Stopped);
        assert_eq!(count_logs(&app, "Processing stopped"), 1);
        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert!(app.changes[0].selected, "execution marks must be preserved");
    }

    #[test]
    fn idle_parallel_stop_repeated_stopped_delivery_does_not_duplicate_the_message() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.mode = AppMode::Running;
        app.changes[0].set_display_status_cache("applying");
        app.changes[0].selected = true;

        app.handle_stopped();
        // A late scheduler-side `Stopped` arriving after the frontend already
        // applied the stop only reconciles state.
        app.handle_stopped();
        app.handle_stopped();

        assert_eq!(count_logs(&app, "Processing stopped"), 1);
        assert_eq!(app.mode, AppMode::Stopped);
        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert!(app.changes[0].selected, "execution marks must be preserved");
    }

    #[test]
    fn idle_parallel_stop_new_run_can_report_its_own_terminal_stop() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.mode = AppMode::Running;

        app.handle_stopped();
        app.mode = AppMode::Running;
        app.handle_stopped();

        assert_eq!(
            count_logs(&app, "Processing stopped"),
            2,
            "a later run owns its own terminal stop message"
        );
    }
}
