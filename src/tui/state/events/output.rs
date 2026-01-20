//! Output event handlers
//!
//! Handles ApplyOutput, ArchiveOutput, AnalysisOutput, ResolveOutput events

use crate::tui::events::LogEntry;

use super::super::AppState;

impl AppState {
    /// Handle ApplyOutput event
    pub(super) fn handle_apply_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        // Update iteration number in change state
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.iteration_number = iteration;
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("apply")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    /// Handle ArchiveOutput event
    pub(super) fn handle_archive_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: u32,
    ) {
        // Update iteration number in change state
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.iteration_number = Some(iteration);
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("archive")
                .with_iteration(iteration),
        );
    }

    /// Handle AcceptanceOutput event
    pub(super) fn handle_acceptance_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        // Update iteration number in change state
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.iteration_number = iteration;
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("acceptance")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    /// Handle AnalysisOutput event
    pub(super) fn handle_analysis_output(&mut self, output: String, iteration: u32) {
        self.add_log(
            LogEntry::info(output)
                .with_operation("analysis")
                .with_iteration(iteration),
        );
    }

    /// Handle ResolveOutput event
    pub(super) fn handle_resolve_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        // Update iteration number in change state
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.iteration_number = iteration;
        }

        let mut entry = LogEntry::info(output)
            .with_change_id(&change_id)
            .with_operation("resolve");
        if let Some(iter) = iteration {
            entry = entry.with_iteration(iter);
        }
        self.add_log(entry);
    }
}
