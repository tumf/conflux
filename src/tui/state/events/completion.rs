//! Completion and error event handlers
//!
//! Handles ApplyFailed, ArchiveFailed, ResolveFailed, MergeDeferred, AcceptanceStarted, AcceptanceCompleted
//! BranchMergeFailed

use std::time::Instant;

use crate::tui::events::LogEntry;
use crate::tui::types::QueueStatus;

use super::super::{AppState, WarningPopup};

impl AppState {
    /// Handle ApplyFailed event
    pub(super) fn handle_apply_failed(&mut self, change_id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Error(error.clone());
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!(
            "Apply failed for {}: {}",
            change_id, error
        )));
    }

    /// Handle ArchiveFailed event
    pub(super) fn handle_archive_failed(&mut self, change_id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Error(error.clone());
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!(
            "Archive failed for {}: {}",
            change_id, error
        )));
    }

    /// Handle ResolveFailed event
    pub(super) fn handle_resolve_failed(&mut self, change_id: String, error: String) {
        self.is_resolving = false;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::MergeWait;
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
    }

    /// Handle MergeDeferred event
    pub(super) fn handle_merge_deferred(&mut self, change_id: String, reason: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::MergeWait;
        }
        self.add_log(LogEntry::warn(format!(
            "Merge deferred for {}: {}",
            change_id, reason
        )));
    }

    /// Handle AcceptanceStarted event
    pub(super) fn handle_acceptance_started(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Accepting;
        }
        self.add_log(LogEntry::info(format!("Acceptance started: {}", change_id)));
    }

    /// Handle AcceptanceCompleted event
    pub(super) fn handle_acceptance_completed(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Completed;
        }
        self.add_log(LogEntry::info(format!(
            "Acceptance completed: {}",
            change_id
        )));
    }

    /// Handle BranchMergeFailed event
    pub(super) fn handle_branch_merge_failed(&mut self, branch_name: String, error: String) {
        self.warning_popup = Some(WarningPopup {
            title: "Merge failed".to_string(),
            message: format!("Failed to merge '{}': {}", branch_name, error),
        });
        self.add_log(LogEntry::error(format!(
            "Merge failed for '{}': {}",
            branch_name, error
        )));
        // Clear is_merging flag on failure
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = false;
        }
    }
}
