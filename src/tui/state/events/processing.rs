//! Processing lifecycle event handlers
//!
//! Handles ProcessingStarted, ProcessingCompleted, ProcessingError, Stopped, AllCompleted

use std::time::Instant;

use crate::tui::events::LogEntry;
use crate::tui::types::{AppMode, QueueStatus, StopMode};

use super::super::AppState;

impl AppState {
    /// Handle ProcessingStarted event
    pub(super) fn handle_processing_started(&mut self, id: String) {
        self.current_change = Some(id.clone());
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Processing;
            change.started_at = Some(Instant::now());
            change.elapsed_time = None;
        }
        self.add_log(LogEntry::info(format!("Processing: {}", id)));
    }

    /// Handle ProcessingCompleted event
    pub(super) fn handle_processing_completed(&mut self, id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Completed;
            // Reload final progress from tasks.md to preserve it
            if let Ok(progress) = crate::task_parser::parse_change(&id) {
                change.completed_tasks = progress.completed;
                change.total_tasks = progress.total;
            }
        }
        self.add_log(LogEntry::success(format!("Completed: {}", id)));
    }

    /// Handle ProcessingError event
    pub(super) fn handle_processing_error(&mut self, id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Error(error.clone());
            change.selected = true;
            // Record elapsed time on error
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        // Record orchestration elapsed time on error
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }
        self.add_log(LogEntry::error(format!("Error in {}: {}", id, error)));
        // Transition to Error mode
        self.mode = AppMode::Error;
        self.error_change_id = Some(id.clone());
        self.current_change = None;
    }

    /// Handle AllCompleted event
    pub(super) fn handle_all_completed(&mut self) {
        if matches!(self.mode, AppMode::Stopped | AppMode::Error) {
            if let Some(started) = self.orchestration_started_at {
                self.orchestration_elapsed = Some(started.elapsed());
            }
            return;
        }

        self.mode = AppMode::Select;
        self.current_change = None;
        self.stop_mode = StopMode::None;
        // Record final orchestration time
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }
        self.add_log(LogEntry::success("All changes processed successfully"));
    }

    /// Handle Stopped event
    pub(super) fn handle_stopped(&mut self) {
        self.mode = AppMode::Stopped;
        self.current_change = None;
        self.stop_mode = StopMode::None;
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }
        // Reset queue status to NotQueued while preserving execution mark (selected)
        // This implements the policy: queued only during active execution
        for change in &mut self.changes {
            if matches!(
                change.queue_status,
                QueueStatus::Processing | QueueStatus::Archiving | QueueStatus::Queued
            ) {
                // Record elapsed time before resetting status (for in-flight changes)
                if let Some(started) = change.started_at {
                    change.elapsed_time = Some(started.elapsed());
                }
                // Reset to NotQueued but preserve execution mark (selected field)
                change.queue_status = QueueStatus::NotQueued;
                // Keep change.selected as-is to preserve execution mark
            }
        }
        self.add_log(LogEntry::warn("Processing stopped"));
    }
}
