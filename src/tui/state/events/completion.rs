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
    pub(super) fn handle_acceptance_started(&mut self, change_id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Accepting;
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

    /// Handle AcceptanceCompleted event
    pub(super) fn handle_acceptance_completed(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Archiving;
        }
        self.add_log(LogEntry::info(format!(
            "Acceptance completed: {}",
            change_id
        )));
    }

    /// Handle ChangeSkipped event
    pub(super) fn handle_change_skipped(&mut self, change_id: String, reason: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Error(reason.clone());
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::warn(format!("Skipped {}: {}", change_id, reason)));
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

    /// Handle DependencyBlocked event
    pub(super) fn handle_dependency_blocked(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Blocked;
        }
        self.add_log(LogEntry::info(format!(
            "Change '{}' blocked by dependencies",
            change_id
        )));
    }

    /// Handle DependencyResolved event
    pub(super) fn handle_dependency_resolved(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            // Only update if currently blocked, otherwise preserve the current state
            if change.queue_status == QueueStatus::Blocked {
                change.queue_status = QueueStatus::Queued;
            }
        }
        self.add_log(LogEntry::info(format!(
            "Change '{}' dependencies resolved",
            change_id
        )));
    }
}
