use super::apply_remote_status;
use super::AppState;
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};

mod completion;
mod errors;
mod output;
mod processing;
mod refresh;

impl AppState {
    /// Handle an event from the orchestrator
    ///
    /// This is the main entry point for event handling, dispatching to specialized handlers.
    /// Returns an optional TuiCommand that should be executed (e.g., for auto-starting next resolve).
    pub fn handle_orchestrator_event(&mut self, event: OrchestratorEvent) -> Option<TuiCommand> {
        match event {
            OrchestratorEvent::ProcessingStarted(id) => self.handle_processing_started(id),
            OrchestratorEvent::ProcessingCompleted(id) => self.handle_processing_completed(id),
            OrchestratorEvent::ProcessingError { id, error } => {
                self.handle_processing_error(id, error)
            }
            OrchestratorEvent::AllCompleted => self.handle_all_completed(),
            OrchestratorEvent::Stopped => self.handle_stopped(),
            OrchestratorEvent::ProgressUpdated {
                change_id,
                completed,
                total,
            } => self.handle_progress_updated(change_id, completed, total),
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
            } => return self.handle_resolve_completed(change_id, worktree_change_ids),
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
            OrchestratorEvent::ApplyFailed { change_id, error } => {
                self.handle_apply_failed(change_id, error)
            }
            OrchestratorEvent::ArchiveFailed { change_id, error } => {
                self.handle_archive_failed(change_id, error)
            }
            OrchestratorEvent::ResolveFailed { change_id, error } => {
                self.handle_resolve_failed(change_id, error)
            }
            OrchestratorEvent::MergeDeferred {
                change_id,
                reason,
                auto_resumable,
            } => return self.handle_merge_deferred(change_id, reason, auto_resumable),
            OrchestratorEvent::AcceptanceStarted { change_id, command } => {
                self.handle_acceptance_started(change_id, command)
            }
            OrchestratorEvent::AcceptanceCompleted { change_id } => {
                self.handle_acceptance_completed(change_id)
            }
            OrchestratorEvent::BranchMergeFailed { branch_name, error } => {
                self.handle_branch_merge_failed(branch_name, error)
            }
            OrchestratorEvent::ChangeStopped { change_id } => self.handle_change_stopped(change_id),
            OrchestratorEvent::ChangeStopFailed { change_id, error } => {
                self.handle_change_stop_failed(change_id, error)
            }
            OrchestratorEvent::ChangesRefreshed {
                changes,
                committed_change_ids,
                uncommitted_file_change_ids,
                worktree_change_ids,
                worktree_paths,
                worktree_not_ahead_ids,
                merge_wait_ids,
            } => self.handle_changes_refreshed(
                changes,
                committed_change_ids,
                uncommitted_file_change_ids,
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
            OrchestratorEvent::AnalysisStarted { remaining_changes } => {
                self.handle_analysis_started(remaining_changes)
            }
            OrchestratorEvent::Log(entry) => self.handle_log(entry),
            OrchestratorEvent::Warning { title, message } => self.handle_warning(title, message),
            OrchestratorEvent::ParallelStartRejected { change_ids, reason } => {
                self.handle_parallel_start_rejected(change_ids, reason)
            }
            OrchestratorEvent::Error { message } => self.handle_error(message),
            OrchestratorEvent::RemoteChangeUpdate {
                id,
                completed_tasks,
                total_tasks,
                status,
                iteration_number,
            } => {
                let mut status_log: Option<String> = None;
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    if completed_tasks >= change.completed_tasks {
                        change.completed_tasks = completed_tasks;
                    }
                    change.total_tasks = total_tasks;

                    if let Some(status) = status.as_deref() {
                        let before = change.display_status_cache.clone();
                        apply_remote_status(change, status);
                        if before != change.display_status_cache {
                            status_log = Some(format!(
                                "Remote status: {} -> {}",
                                id,
                                change.display_status_cache.as_str()
                            ));
                        }
                    }

                    change.update_iteration_monotonic(iteration_number);
                }
                if let Some(line) = status_log {
                    self.add_log(LogEntry::info(line));
                }
            }
            _ => {}
        }
        None
    }
}
