use std::time::Instant;

use crate::task_parser;
use crate::tui::events::LogEntry;
use crate::tui::types::{AppMode, StopMode};

use super::AppState;

impl AppState {
    pub(crate) fn handle_processing_started(&mut self, id: String) {
        self.current_change = Some(id.clone());
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.set_display_status_cache("applying");
            change.started_at = Some(Instant::now());
            change.elapsed_time = None;
        }
        self.add_log(LogEntry::info(format!("Processing: {}", id)));
    }

    pub(crate) fn handle_apply_started(&mut self, change_id: String, command: String) {
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
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.set_display_status_cache("archiving");
            change.iteration_number = None;
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
        self.is_resolving = true;
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

    pub(crate) fn handle_analysis_started(&mut self, remaining_changes: usize) {
        self.add_log(LogEntry::info(format!(
            "Re-analyzing queued changes for dispatch (remaining: {})",
            remaining_changes
        )));
    }

    pub(crate) fn handle_acceptance_started(&mut self, change_id: String, command: String) {
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
            LogEntry::info(format!("  Command: {}", command))
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

    pub(crate) fn handle_stopped(&mut self) {
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
        self.add_log(LogEntry::warn("Processing stopped"));
    }

    pub(crate) fn handle_progress_updated(
        &mut self,
        change_id: String,
        completed: u32,
        total: u32,
    ) {
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
}
