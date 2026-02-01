//! Event handling modules for AppState
//!
//! This module organizes event handlers by responsibility:
//! - `processing`: Lifecycle events (start, complete, error, stop)
//! - `progress`: Progress update events
//! - `stages`: Stage events (apply, archive, resolve, merge)
//! - `completion`: Completion and error events
//! - `refresh`: Refresh events (changes, worktrees)
//! - `output`: Output streaming events
//! - `messages`: Log, warning, and error message events
//!
//! This file also contains helper methods (update_changes, auto_clear_merge_wait) and tests.

mod completion;
mod helpers;
mod messages;
mod output;
mod processing;
mod progress;
mod refresh;
mod stages;

use super::AppState;
use crate::tui::events::OrchestratorEvent;

impl AppState {
    /// Handle an event from the orchestrator
    ///
    /// This is the main entry point for event handling, dispatching to specialized handlers.
    pub fn handle_orchestrator_event(&mut self, event: OrchestratorEvent) {
        match event {
            // Processing lifecycle events
            OrchestratorEvent::ProcessingStarted(id) => self.handle_processing_started(id),
            OrchestratorEvent::ProcessingCompleted(id) => self.handle_processing_completed(id),
            OrchestratorEvent::ProcessingError { id, error } => {
                self.handle_processing_error(id, error)
            }
            OrchestratorEvent::AllCompleted => self.handle_all_completed(),
            OrchestratorEvent::Stopped => self.handle_stopped(),

            // Progress events
            OrchestratorEvent::ProgressUpdated {
                change_id,
                completed,
                total,
            } => self.handle_progress_updated(change_id, completed, total),

            // Stage events
            OrchestratorEvent::ApplyStarted { change_id, command } => {
                self.handle_apply_started(change_id, command)
            }
            OrchestratorEvent::ArchiveStarted { change_id, command } => {
                self.handle_archive_started(change_id, command)
            }
            OrchestratorEvent::ChangeArchived(id) => self.handle_change_archived(id),
            OrchestratorEvent::ResolveStarted { change_id, command } => {
                self.handle_resolve_started(change_id, command)
            }
            OrchestratorEvent::ResolveCompleted {
                change_id,
                worktree_change_ids,
            } => self.handle_resolve_completed(change_id, worktree_change_ids),
            OrchestratorEvent::MergeCompleted {
                change_id,
                revision: _,
            } => self.handle_merge_completed(change_id),
            OrchestratorEvent::BranchMergeStarted { branch_name } => {
                self.handle_branch_merge_started(branch_name)
            }
            OrchestratorEvent::BranchMergeCompleted { branch_name } => {
                self.handle_branch_merge_completed(branch_name)
            }

            // Completion and error events
            OrchestratorEvent::ApplyFailed { change_id, error } => {
                self.handle_apply_failed(change_id, error)
            }
            OrchestratorEvent::ArchiveFailed { change_id, error } => {
                self.handle_archive_failed(change_id, error)
            }
            OrchestratorEvent::ResolveFailed { change_id, error } => {
                self.handle_resolve_failed(change_id, error)
            }
            OrchestratorEvent::MergeDeferred { change_id, reason } => {
                self.handle_merge_deferred(change_id, reason)
            }
            OrchestratorEvent::AcceptanceStarted { change_id } => {
                self.handle_acceptance_started(change_id)
            }
            OrchestratorEvent::AcceptanceCompleted { change_id } => {
                self.handle_acceptance_completed(change_id)
            }
            OrchestratorEvent::BranchMergeFailed { branch_name, error } => {
                self.handle_branch_merge_failed(branch_name, error)
            }

            // Refresh events
            OrchestratorEvent::ChangesRefreshed {
                changes,
                committed_change_ids,
                worktree_change_ids,
                worktree_paths,
                worktree_not_ahead_ids,
                merge_wait_ids,
            } => self.handle_changes_refreshed(
                changes,
                committed_change_ids,
                worktree_change_ids,
                worktree_paths,
                worktree_not_ahead_ids,
                merge_wait_ids,
            ),
            OrchestratorEvent::WorktreesRefreshed { worktrees } => {
                self.handle_worktrees_refreshed(worktrees)
            }
            OrchestratorEvent::ChangeSkipped { change_id, reason } => {
                self.handle_change_skipped(change_id, reason)
            }
            OrchestratorEvent::DependencyBlocked {
                change_id,
                dependency_ids: _,
            } => self.handle_dependency_blocked(change_id),
            OrchestratorEvent::DependencyResolved { change_id } => {
                self.handle_dependency_resolved(change_id)
            }

            // Output events
            OrchestratorEvent::ApplyOutput {
                change_id,
                output,
                iteration,
            } => self.handle_apply_output(change_id, output, iteration),
            OrchestratorEvent::ArchiveOutput {
                change_id,
                output,
                iteration,
            } => self.handle_archive_output(change_id, output, iteration),
            OrchestratorEvent::AcceptanceOutput {
                change_id,
                output,
                iteration,
            } => self.handle_acceptance_output(change_id, output, iteration),
            OrchestratorEvent::AnalysisOutput { output, iteration } => {
                self.handle_analysis_output(output, iteration)
            }
            OrchestratorEvent::ResolveOutput {
                change_id,
                output,
                iteration,
            } => self.handle_resolve_output(change_id, output, iteration),

            // Message events
            OrchestratorEvent::Log(entry) => self.handle_log(entry),
            OrchestratorEvent::Warning { title, message } => self.handle_warning(title, message),
            OrchestratorEvent::Error { message } => self.handle_error(message),

            // Ignore other parallel-specific events that don't affect TUI state
            _ => {
                // Other events (workspace, merge, group events) are for status tracking
                // and don't need to be displayed in the log
            }
        }
    }
}
