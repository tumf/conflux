use crate::events::{ApplyCommitPhase, CommitOutputStream};
use crate::tui::events::LogEntry;

use super::AppState;

impl AppState {
    pub(crate) fn handle_apply_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        let mut operation = "apply".to_string();
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "applying") {
                change.update_iteration_monotonic(iteration);
            }
            // While finalization owns the lane, everything it prints is labeled
            // `[commit]` so the operator can tell hook work from agent work.
            operation = change.apply_operation().to_string();
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation(operation)
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    /// Switch the Apply lane between `[apply]` and `[commit]` presentation.
    ///
    /// Only the rendered operation label changes. The row keeps its `applying`
    /// status and color, and no scheduling or routing state is touched.
    pub(crate) fn handle_apply_commit_phase(
        &mut self,
        change_id: String,
        phase: ApplyCommitPhase,
        _attempt: u32,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.apply_operation_cache =
                if phase.is_active() { "commit" } else { "apply" }.to_string();
        }
    }

    /// Log one streamed line of final-commit output.
    ///
    /// Labeled by attempt rather than deduplicated, so an index-lock retry's
    /// repeated hook output stays attributable to the attempt that produced it.
    pub(crate) fn handle_apply_commit_output(
        &mut self,
        change_id: String,
        attempt: u32,
        stream: CommitOutputStream,
        line: String,
    ) {
        // Hooks routinely report progress on stderr, so the stream is
        // attribution, not severity: every streamed line is logged at info and
        // rejection is classified from the exit status alone. The stream name
        // is carried in the message because that is what makes each line
        // self-identifying without changing the published log schema.
        self.add_log(
            LogEntry::info(format!("{}: {}", stream.as_str(), line))
                .with_change_id(change_id)
                .with_operation("commit")
                .with_iteration(attempt),
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

    /// Handle a global error event as fatal.
    ///
    /// Fatality is decided by event type alone. Diagnostic message content is never
    /// inspected: a genuine fatal error can quote or wrap recoverable-fallback wording,
    /// and downgrading on that text would keep `AppExecutionMode::Running` after orchestration
    /// had already stopped. Recoverable dependency-analysis fallback stays non-fatal by
    /// arriving on the producer's warning-log event path instead of this channel.
    pub(crate) fn handle_error(&mut self, message: String) {
        self.reset_analysis_log_dedupe();
        self.add_log(LogEntry::error(message.clone()));
        self.execution_mode = crate::tui::types::AppExecutionMode::Error;
        self.error_change_id = None;
        self.current_change = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::tui::events::{LogEntry, LogLevel, OrchestratorEvent};
    use crate::tui::types::AppExecutionMode;

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
    /// recoverable analysis fallback, so consumer handling is tested against real
    /// producer wording rather than a copy that can drift.
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
        app.execution_mode = AppExecutionMode::Running;
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
    fn fatal_global_error_quoting_fallback_marker_still_enters_error_mode() {
        let (mut app, _shared) = running_app_with_shared_state();
        let quoted =
            recoverable_analysis_fallback_message(&["change-a"], "Missing change IDs in response");
        let fatal = format!(
            "Parallel execution failed: base worktree is unusable (last diagnostic: \"{quoted}\")"
        );

        app.handle_orchestrator_event(OrchestratorEvent::Error {
            message: fatal.clone(),
        });

        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Error,
            "message content must not downgrade a global error event"
        );
        assert_eq!(app.current_change, None, "fatal error must clear context");
        assert_eq!(app.error_change_id, None);
        let entry = app
            .logs
            .iter()
            .rev()
            .find(|entry| entry.message.contains("base worktree is unusable"))
            .expect("fatal diagnostic log entry");
        assert_eq!(
            entry.level,
            LogLevel::Error,
            "fatal diagnostic must stay error-level even when it quotes fallback wording"
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

        // Production path: the scheduler emits the successful fallback as a warning
        // log event, so the running presentation must survive untouched.
        app.handle_orchestrator_event(OrchestratorEvent::Log(LogEntry::warn(&fallback)));

        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Running,
            "fallback must not stop the TUI"
        );
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

        assert_eq!(app.execution_mode, AppExecutionMode::Running);
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

        app.handle_orchestrator_event(OrchestratorEvent::Log(LogEntry::warn(&fallback)));

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
        assert_eq!(app.execution_mode, AppExecutionMode::Running);
        assert_eq!(app.current_change.as_deref(), Some("change-b"));
        assert_eq!(app.changes[1].display_status_cache, "applying");

        app.handle_orchestrator_event(OrchestratorEvent::Stopped);
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Stopped,
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

        app.handle_orchestrator_event(OrchestratorEvent::Log(LogEntry::warn(&fallback)));
        app.handle_orchestrator_event(OrchestratorEvent::ChangeArchived("change-a".to_string()));

        assert_eq!(app.execution_mode, AppExecutionMode::Running);
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

        assert_eq!(app.execution_mode, AppExecutionMode::Error);
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
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[1].display_status_cache = "queued".to_string();

        app.handle_parallel_start_rejected(
            vec!["change-a".to_string()],
            "uncommitted or not in HEAD".to_string(),
        );

        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert_eq!(app.changes[1].display_status_cache, "queued");
    }

    // === Apply-lane operation presentation ===

    /// Build an app with one row already rendering the Apply lane.
    fn applying_app(change_id: &str) -> AppState {
        let mut app = AppState::new(vec![create_test_change(change_id, 0, 1)]);
        app.changes[0].set_display_status_cache("applying");
        app
    }

    /// The rendered log header must follow the commit subphase: `[apply]` while
    /// the agent runs, `[commit]` while finalization does, `[apply]` again for a
    /// repair iteration. The row's status word never changes.
    #[test]
    fn the_apply_lane_label_follows_the_commit_subphase() {
        let mut app = applying_app("change-a");

        app.handle_orchestrator_event(OrchestratorEvent::ApplyOutput {
            change_id: "change-a".to_string(),
            output: "agent working".to_string(),
            iteration: Some(2),
        });
        assert_eq!(app.logs.last().unwrap().operation.as_deref(), Some("apply"));

        app.handle_orchestrator_event(OrchestratorEvent::ApplyCommitPhase {
            change_id: "change-a".to_string(),
            phase: ApplyCommitPhase::Started,
            attempt: 2,
        });
        assert_eq!(app.changes[0].apply_operation(), "commit");
        assert_eq!(
            app.changes[0].display_status_cache, "applying",
            "the canonical status stays `applying` throughout finalization"
        );

        app.handle_orchestrator_event(OrchestratorEvent::ApplyOutput {
            change_id: "change-a".to_string(),
            output: "still in finalization".to_string(),
            iteration: Some(2),
        });
        assert_eq!(
            app.logs.last().unwrap().operation.as_deref(),
            Some("commit")
        );

        // A repair iteration restores apply presentation.
        app.handle_orchestrator_event(OrchestratorEvent::ApplyCommitPhase {
            change_id: "change-a".to_string(),
            phase: ApplyCommitPhase::Failed,
            attempt: 2,
        });
        assert_eq!(app.changes[0].apply_operation(), "apply");
        app.handle_orchestrator_event(OrchestratorEvent::ApplyOutput {
            change_id: "change-a".to_string(),
            output: "repair working".to_string(),
            iteration: Some(3),
        });
        assert_eq!(app.logs.last().unwrap().operation.as_deref(), Some("apply"));
    }

    /// Completion must also clear the subphase, so no row keeps rendering
    /// `[commit]` after finalization ends.
    #[test]
    fn a_completed_commit_phase_clears_the_commit_label() {
        let mut app = applying_app("change-a");

        app.handle_orchestrator_event(OrchestratorEvent::ApplyCommitPhase {
            change_id: "change-a".to_string(),
            phase: ApplyCommitPhase::Started,
            attempt: 1,
        });
        app.handle_orchestrator_event(OrchestratorEvent::ApplyCommitPhase {
            change_id: "change-a".to_string(),
            phase: ApplyCommitPhase::Completed,
            attempt: 1,
        });

        assert_eq!(app.changes[0].apply_operation(), "apply");
    }

    /// Streamed commit lines must be self-identifying: change, `commit`
    /// operation, source stream, and finalization attempt.
    #[test]
    fn streamed_commit_lines_carry_change_stream_and_attempt() {
        let mut app = applying_app("change-a");

        app.handle_orchestrator_event(OrchestratorEvent::ApplyCommitOutput {
            change_id: "change-a".to_string(),
            attempt: 2,
            stream: CommitOutputStream::Stderr,
            line: "pre-commit: running clippy".to_string(),
        });

        let entry = app
            .logs
            .last()
            .expect("a streamed line becomes a log entry");
        assert_eq!(entry.change_id.as_deref(), Some("change-a"));
        assert_eq!(entry.operation.as_deref(), Some("commit"));
        assert_eq!(entry.iteration, Some(2));
        assert!(
            entry.message.contains("stderr: pre-commit: running clippy"),
            "the source stream must be identifiable: {}",
            entry.message
        );
        assert_eq!(
            entry.level,
            LogLevel::Info,
            "a stderr hook line is progress, not a failure verdict"
        );
    }

    /// Contention retries re-run the same hooks and produce the same text. Those
    /// lines must stay separate entries labelled by their own attempt rather
    /// than being collapsed into one.
    #[test]
    fn repeated_lines_from_separate_attempts_are_not_deduplicated() {
        let mut app = applying_app("change-a");

        for attempt in 1..=2 {
            app.handle_orchestrator_event(OrchestratorEvent::ApplyCommitOutput {
                change_id: "change-a".to_string(),
                attempt,
                stream: CommitOutputStream::Stdout,
                line: "running repository verification".to_string(),
            });
        }

        let commit_entries: Vec<_> = app
            .logs
            .iter()
            .filter(|entry| entry.operation.as_deref() == Some("commit"))
            .collect();
        assert_eq!(
            commit_entries.len(),
            2,
            "identical lines from two attempts must both survive"
        );
        assert_eq!(commit_entries[0].iteration, Some(1));
        assert_eq!(commit_entries[1].iteration, Some(2));
    }

    /// The reducer refresh is a self-healing backstop: a frontend that missed a
    /// phase event must still converge instead of rendering `[commit]` forever.
    #[test]
    fn the_reducer_refresh_converges_a_frontend_that_missed_an_event() {
        let mut app = applying_app("change-a");
        app.handle_orchestrator_event(OrchestratorEvent::ApplyCommitPhase {
            change_id: "change-a".to_string(),
            phase: ApplyCommitPhase::Started,
            attempt: 1,
        });
        assert_eq!(app.changes[0].apply_operation(), "commit");

        let mut labels = std::collections::HashMap::new();
        labels.insert("change-a".to_string(), "apply");
        app.apply_operation_labels_from_reducer(&labels);

        assert_eq!(app.changes[0].apply_operation(), "apply");
    }
}
