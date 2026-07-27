use crate::events::RECOVERABLE_ANALYSIS_FALLBACK_MARKER;
use crate::tui::events::LogEntry;

use super::AppState;

/// Classify a global diagnostic as a recoverable dependency-analysis fallback.
///
/// Successful metadata-dependency-only fallback keeps the scheduler running, so the
/// diagnostic is degraded-execution observability rather than a fatal run failure.
/// The producer already emits it as a warning; this classifier keeps the TUI safe if
/// any producer path ever routes the same diagnostic through the fatal error channel.
pub(crate) fn is_recoverable_analysis_fallback(message: &str) -> bool {
    message.contains(RECOVERABLE_ANALYSIS_FALLBACK_MARKER)
}

impl AppState {
    pub(crate) fn handle_apply_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "applying") {
                change.update_iteration_monotonic(iteration);
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("apply")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    pub(crate) fn handle_archive_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: u32,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "archiving") {
                change.update_iteration_monotonic(Some(iteration));
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("archive")
                .with_iteration(iteration),
        );
    }

    pub(crate) fn handle_acceptance_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "accepting") {
                change.update_iteration_monotonic(iteration);
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("acceptance")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    pub(crate) fn handle_analysis_output(&mut self, output: String, iteration: u32) {
        self.add_log(
            LogEntry::info(output)
                .with_operation("analysis")
                .with_iteration(iteration),
        );
    }

    pub(crate) fn handle_resolve_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "resolving") {
                change.update_iteration_monotonic(iteration);
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(&change_id)
                .with_operation("resolve")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    pub(crate) fn handle_log(&mut self, entry: LogEntry) {
        self.add_log(entry);
    }

    pub(crate) fn handle_warning(&mut self, title: String, message: String) {
        if title != "Uncommitted Changes Detected" {
            self.show_warning_popup(title, message.clone());
        }
        self.add_log(LogEntry::warn(message));
    }

    pub(crate) fn handle_change_rejected(&mut self, change_id: String, reason: String) {
        self.reset_analysis_log_dedupe();
        if let Some(change) = self
            .changes
            .iter_mut()
            .find(|change| change.id == change_id)
        {
            change.set_display_status_cache("rejected");
            change.selected = false;
        }

        self.add_log(
            LogEntry::warn(format!("Change rejected: {} ({})", change_id, reason))
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_parallel_start_rejected(
        &mut self,
        change_ids: Vec<String>,
        reason: String,
    ) {
        self.reset_analysis_log_dedupe();
        let mut reset_ids = Vec::new();
        for change in &mut self.changes {
            if change_ids.contains(&change.id)
                && matches!(change.display_status_cache.as_str(), "queued")
            {
                change.set_display_status_cache("not queued");
                reset_ids.push(change.id.clone());
            }
        }

        if reset_ids.is_empty() {
            return;
        }

        if let Some(shared) = &self.shared_orchestrator_state {
            if let Ok(mut guard) = shared.try_write() {
                for id in &reset_ids {
                    guard.apply_command(
                        crate::orchestration::state::ReducerCommand::RemoveFromQueue(id.clone()),
                    );
                }
            }
        }

        self.add_log(LogEntry::warn(format!(
            "Not started ({}): {}",
            reason,
            reset_ids.join(", ")
        )));
    }

    pub(crate) fn handle_error(&mut self, message: String) {
        if is_recoverable_analysis_fallback(&message) {
            // Non-fatal path: orchestration keeps executing on metadata dependencies,
            // so the running lifecycle presentation (mode, current change, active rows,
            // queue marks, reducer-derived state) must survive untouched. Only the
            // warning log is appended.
            self.add_log(LogEntry::warn(message));
            return;
        }

        self.reset_analysis_log_dedupe();
        self.add_log(LogEntry::error(message.clone()));
        self.mode = crate::tui::types::AppMode::Error;
        self.error_change_id = None;
        self.current_change = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::remote::types::RemoteLogEntry;
    use crate::tui::events::{LogEntry, LogLevel, OrchestratorEvent};
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

    /// Build the exact operator-facing diagnostic the scheduler emits for a
    /// recoverable analysis fallback, so consumer classification is tested against
    /// real producer wording rather than a copy that can drift.
    pub(crate) fn recoverable_analysis_fallback_message(queued: &[&str], error: &str) -> String {
        let changes: Vec<Change> = queued
            .iter()
            .map(|id| create_test_change(id, 0, 1))
            .collect();
        let (_, message) =
            crate::parallel_run_service::ParallelRunService::recoverable_analysis_fallback_diagnostic(
                &changes,
                &[],
                error,
            );
        message
    }

    /// Running TUI with one applying change, one queued change, and reducer-backed
    /// shared state, matching the observed production scenario.
    fn running_app_with_shared_state() -> (
        AppState,
        std::sync::Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    ) {
        use crate::orchestration::state::{OrchestratorState, ReducerCommand};
        use std::sync::Arc;

        let mut app = AppState::new(vec![
            create_test_change("change-a", 1, 3),
            create_test_change("change-b", 0, 2),
        ]);
        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string(), "change-b".to_string()],
            0,
        )));
        {
            let mut guard = shared.blocking_write();
            guard.apply_command(ReducerCommand::AddToQueue("change-a".to_string()));
            guard.apply_command(ReducerCommand::AddToQueue("change-b".to_string()));
            guard.apply_execution_event(&crate::events::ExecutionEvent::ProcessingStarted(
                "change-a".to_string(),
            ));
        }
        app.set_shared_state(shared.clone());
        app.mode = AppMode::Running;
        app.orchestration_started_at = Some(std::time::Instant::now());
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingStarted("change-a".to_string()));
        app.changes[1].display_status_cache = "queued".to_string();
        app.changes[1].selected = true;
        (app, shared)
    }

    fn reducer_snapshot(
        shared: &std::sync::Arc<
            tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>,
        >,
    ) -> (
        Vec<String>,
        Vec<String>,
        std::collections::HashMap<String, &'static str>,
    ) {
        let guard = shared.blocking_read();
        (
            guard.queued_change_ids(),
            guard.active_change_ids(),
            guard.all_display_statuses(),
        )
    }

    #[test]
    fn recoverable_analysis_fallback_classifier_matches_producer_message_only() {
        let fallback =
            recoverable_analysis_fallback_message(&["change-a"], "Missing change IDs in response");

        assert!(
            is_recoverable_analysis_fallback(&fallback),
            "producer fallback wording must be recognized as recoverable: {fallback}"
        );
        assert!(
            !is_recoverable_analysis_fallback("Parallel execution failed: worktree base is dirty"),
            "unrelated global failures must stay fatal"
        );
        assert!(
            !is_recoverable_analysis_fallback("Dependency analysis failed: error=timeout"),
            "a terminal analysis failure must not be reclassified as recoverable"
        );
    }

    #[test]
    fn analysis_fallback_running_state_is_not_fatal() {
        let (mut app, shared) = running_app_with_shared_state();
        let before = reducer_snapshot(&shared);
        let statuses_before: Vec<String> = app
            .changes
            .iter()
            .map(|c| c.display_status_cache.clone())
            .collect();
        let started_at = app.orchestration_started_at;
        let fallback = recoverable_analysis_fallback_message(
            &["change-a", "change-b"],
            "Missing change IDs in response: [\"change-b\"]",
        );

        // Defensive path: even if the diagnostic ever arrives through the fatal
        // global error channel, the TUI must keep the running presentation.
        app.handle_orchestrator_event(OrchestratorEvent::Error {
            message: fallback.clone(),
        });

        assert_eq!(app.mode, AppMode::Running, "fallback must not stop the TUI");
        assert_eq!(app.current_change.as_deref(), Some("change-a"));
        assert_eq!(app.error_change_id, None);
        assert_eq!(app.orchestration_started_at, started_at);
        assert_eq!(
            app.changes
                .iter()
                .map(|c| c.display_status_cache.clone())
                .collect::<Vec<_>>(),
            statuses_before,
            "active rows and queue marks must survive the fallback"
        );
        assert!(app.changes[1].selected, "queue selection must survive");
        assert_eq!(
            reducer_snapshot(&shared),
            before,
            "presentation handling must not mutate reducer-derived scheduler state"
        );

        let warning = app
            .logs
            .iter()
            .rev()
            .find(|entry| entry.message.contains("metadata-dependency-only"))
            .expect("fallback warning log entry");
        assert_eq!(warning.level, LogLevel::Warn);
        assert!(
            warning.message.contains("Missing change IDs in response"),
            "warning must keep the original rejection reason: {}",
            warning.message
        );
        assert!(
            !app.logs.iter().any(|entry| entry.level == LogLevel::Error),
            "recoverable fallback must not produce an error-level log entry"
        );
    }

    #[test]
    fn analysis_fallback_warning_log_event_keeps_running_state() {
        let (mut app, _shared) = running_app_with_shared_state();
        let fallback =
            recoverable_analysis_fallback_message(&["change-a"], "Duplicate change ID in order");

        // Production path: the scheduler emits the diagnostic as a warning log event.
        app.handle_orchestrator_event(OrchestratorEvent::Log(LogEntry::warn(&fallback)));

        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.current_change.as_deref(), Some("change-a"));
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.level == LogLevel::Warn && entry.message.contains(&fallback)));
    }

    #[test]
    fn analysis_fallback_running_state_keeps_processing_later_events() {
        let (mut app, _shared) = running_app_with_shared_state();
        let fallback = recoverable_analysis_fallback_message(
            &["change-a", "change-b"],
            "Missing change IDs in response: [\"change-b\"]",
        );

        app.handle_orchestrator_event(OrchestratorEvent::Error { message: fallback });

        // No explicit retry is issued: later lifecycle events must flow normally.
        // Only handlers without filesystem access are exercised here so the test
        // stays unit-scoped; archive/refresh continuity is covered by the
        // integration-scoped sequence test in `tests/`.
        app.handle_orchestrator_event(OrchestratorEvent::AcceptanceCompleted {
            change_id: "change-a".to_string(),
        });
        assert_eq!(app.changes[0].display_status_cache, "archiving");

        app.handle_orchestrator_event(OrchestratorEvent::ProgressUpdated {
            change_id: "change-b".to_string(),
            completed: 1,
            total: 2,
        });
        assert_eq!(app.changes[1].completed_tasks, 1);

        app.handle_orchestrator_event(OrchestratorEvent::ProcessingStarted("change-b".to_string()));
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.current_change.as_deref(), Some("change-b"));
        assert_eq!(app.changes[1].display_status_cache, "applying");

        app.handle_orchestrator_event(OrchestratorEvent::Stopped);
        assert_eq!(
            app.mode,
            AppMode::Stopped,
            "stop must still take effect without an intervening retry"
        );
    }

    /// Integration-scoped evidence: this exercises the archive handler's real
    /// filesystem task-progress lookup, so it is not unit-test coverage.
    #[test]
    fn analysis_fallback_running_state_keeps_archive_handling_working() {
        let worktree = tempfile::TempDir::new().expect("tempdir");
        let change_dir = worktree.path().join("openspec/changes/change-a");
        std::fs::create_dir_all(&change_dir).expect("create change dir");
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] one\n- [x] two\n- [ ] three\n",
        )
        .expect("write tasks.md");

        let (mut app, _shared) = running_app_with_shared_state();
        app.worktree_paths
            .insert("change-a".to_string(), worktree.path().to_path_buf());
        let fallback = recoverable_analysis_fallback_message(
            &["change-a", "change-b"],
            "Missing change IDs in response: [\"change-b\"]",
        );

        app.handle_orchestrator_event(OrchestratorEvent::Error { message: fallback });
        app.handle_orchestrator_event(OrchestratorEvent::ChangeArchived("change-a".to_string()));

        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.changes[0].display_status_cache, "archived");
        assert_eq!(
            (app.changes[0].completed_tasks, app.changes[0].total_tasks),
            (2, 3),
            "archive handling must still read real task progress after the fallback"
        );
    }

    #[test]
    fn genuine_global_error_still_enters_fatal_error_mode() {
        let (mut app, _shared) = running_app_with_shared_state();

        app.handle_orchestrator_event(OrchestratorEvent::Error {
            message: "Parallel execution failed: base worktree is unusable".to_string(),
        });

        assert_eq!(app.mode, AppMode::Error);
        assert_eq!(app.current_change, None);
        assert_eq!(app.error_change_id, None);
        assert!(app.logs.iter().any(|entry| entry.level == LogLevel::Error
            && entry.message.contains("base worktree is unusable")));
    }

    #[test]
    fn warning_for_uncommitted_changes_is_logged_only() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::Warning {
            title: "Uncommitted Changes Detected".to_string(),
            message: "Warning: Uncommitted changes detected.".to_string(),
        });

        assert!(app.warning_popup.is_none());
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Warning: Uncommitted")));
    }

    #[test]
    fn remote_change_update_keeps_progress_monotonic() {
        let changes = vec![create_test_change("MyProj/feat", 4, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            status: None,
            iteration_number: None,
        });

        assert_eq!(app.changes[0].completed_tasks, 4);
    }

    #[test]
    fn remote_status_transition_log_carries_structured_change_id() {
        let mut app = AppState::new(vec![create_test_change("MyProj/feat", 1, 5)]);
        app.logs.clear();

        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            status: Some("applying".to_string()),
            iteration_number: None,
        });

        let entry = app
            .logs
            .iter()
            .find(|entry| entry.message.starts_with("Remote status:"))
            .expect("remote status transition log");
        assert_eq!(entry.change_id.as_deref(), Some("MyProj/feat"));
    }

    #[test]
    fn remote_change_update_keeps_iteration_monotonic() {
        let changes = vec![create_test_change("MyProj/feat", 1, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            status: None,
            iteration_number: Some(3),
        });
        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 3,
            total_tasks: 5,
            status: None,
            iteration_number: Some(2),
        });

        assert_eq!(app.changes[0].iteration_number, Some(3));
    }

    #[test]
    fn remote_log_event_is_added() {
        let mut app = AppState::new(vec![create_test_change("proj/change-a", 0, 3)]);
        let initial = app.logs.len();

        let entry = LogEntry {
            timestamp: "12:00:00".to_string(),
            created_at: chrono::Utc::now(),
            message: "remote stdout: cargo build succeeded".to_string(),
            color: ratatui::style::Color::Reset,
            level: LogLevel::Info,
            change_id: Some("change-a".to_string()),
            operation: None,
            iteration: None,
            workspace_path: None,
        };

        app.handle_orchestrator_event(OrchestratorEvent::Log(entry.clone()));

        assert!(app.logs.len() > initial);
        let last = app.logs.last().expect("at least one log entry");
        assert_eq!(last.message, entry.message);
        assert_eq!(last.change_id, entry.change_id);
    }

    #[test]
    fn remote_log_entry_project_id_round_trip() {
        let entry = RemoteLogEntry {
            message: "stdout: tests passed".to_string(),
            level: "info".to_string(),
            change_id: None,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            project_id: Some("proj-abc123".to_string()),
            operation: Some("apply".to_string()),
            iteration: Some(2),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let decoded: RemoteLogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.project_id, entry.project_id);
        assert_eq!(decoded.operation, entry.operation);
        assert_eq!(decoded.iteration, entry.iteration);
    }

    #[test]
    fn change_rejected_clears_only_target_selection() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);
        app.changes[0].selected = true;
        app.changes[1].selected = true;
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[1].display_status_cache = "queued".to_string();

        app.handle_change_rejected("change-a".to_string(), "blocked by review".to_string());

        assert_eq!(app.changes[0].display_status_cache, "rejected");
        assert!(!app.changes[0].selected);
        assert_eq!(app.changes[1].display_status_cache, "queued");
        assert!(app.changes[1].selected);
    }

    #[test]
    fn parallel_start_rejected_only_clears_target_rows() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[1].display_status_cache = "queued".to_string();

        app.handle_parallel_start_rejected(
            vec!["change-a".to_string()],
            "uncommitted or not in HEAD".to_string(),
        );

        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert_eq!(app.changes[1].display_status_cache, "queued");
    }
}
