use crate::tui::events::{LogEntry, TuiCommand};

use crate::tui::state::WarningPopup;

use super::AppState;

impl AppState {
    pub(crate) fn handle_apply_failed(&mut self, change_id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_error_message_cache(error.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!(
            "Apply failed for {}: {}",
            change_id, error
        )));
    }

    pub(crate) fn handle_archive_failed(&mut self, change_id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.set_error_message_cache(error.clone());
            change.selected = false;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!(
            "Archive failed for {}: {}",
            change_id, error
        )));
    }

    pub(crate) fn handle_resolve_failed(&mut self, change_id: String, error: String) {
        self.is_resolving = false;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.display_status_cache == "merged" {
                self.add_log(LogEntry::info(format!(
                    "Ignoring ResolveFailed for '{}': already Merged",
                    change_id
                )));
                return;
            }
            change.set_display_status_cache("merge wait");
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

        self.try_transition_to_select();
    }

    pub(crate) fn handle_merge_deferred(
        &mut self,
        change_id: String,
        reason: String,
        auto_resumable: bool,
    ) -> Option<TuiCommand> {
        if self.is_resolving {
            let is_current_resolving = self
                .changes
                .iter()
                .any(|c| c.id == change_id && c.display_status_cache == "resolving");

            if is_current_resolving {
                self.add_log(LogEntry::warn(format!(
                    "Merge deferred for '{}' (currently resolving, not queued): {}",
                    change_id, reason
                )));
            } else {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.set_display_status_cache("resolve pending");
                }
                if self.add_to_resolve_queue(&change_id) {
                    self.add_log(LogEntry::warn(format!(
                        "Merge deferred for '{}' (queued for resolve): {}",
                        change_id, reason
                    )));
                } else {
                    self.add_log(LogEntry::warn(format!(
                        "Merge deferred for '{}' (already queued): {}",
                        change_id, reason
                    )));
                }
            }
            None
        } else if auto_resumable {
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                change.set_display_status_cache("resolve pending");
            }
            self.add_to_resolve_queue(&change_id);
            self.add_log(LogEntry::warn(format!(
                "Merge deferred for '{}' (auto-resumable, starting resolve): {}",
                change_id, reason
            )));
            Some(TuiCommand::ResolveMerge(change_id))
        } else {
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                change.set_display_status_cache("merge wait");
            }
            self.add_log(LogEntry::warn(format!(
                "Merge deferred for {}: {}",
                change_id, reason
            )));
            None
        }
    }
}
