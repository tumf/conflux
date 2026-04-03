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

    pub(crate) fn handle_processing_completed(&mut self, id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.set_display_status_cache("archiving");
            if let Ok(progress) = task_parser::parse_change(&id) {
                change.completed_tasks = progress.completed;
                change.total_tasks = progress.total;
            }
        }
        self.add_log(LogEntry::success(format!("Completed: {}", id)));
    }

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

    pub(crate) fn handle_all_completed(&mut self) {
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
