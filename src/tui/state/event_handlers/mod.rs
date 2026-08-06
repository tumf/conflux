use super::AppState;
use crate::tui::events::OrchestratorEvent;

mod completion;
mod errors;
#[cfg(test)]
mod modal_tests;
mod output;
mod processing;
mod refresh;

impl AppState {
    /// Handle an event from the orchestrator
    ///
    /// This is the main entry point for event handling, dispatching to specialized handlers.
    ///
    /// Event handling never emits a `TuiCommand`. Every lifecycle transition an
    /// orchestrator event implies — including promoting the next resolve — is
    /// dispatched by the scheduler from reducer-owned intent, so the frontend
    /// only paints. Re-submitting one of those intents from here could only be
    /// refused as no longer eligible, and would reach the operator as a warning
    /// for work that is in fact proceeding.
    pub fn handle_orchestrator_event(&mut self, event: OrchestratorEvent) {
        self.apply_persistent_idle_episode_rules(&event);
        self.dispatch_orchestrator_event(event);

        // A background event may move execution or the observed target set while a
        // modal is visible. Handlers own execution state; the modal axis changes
        // only here, through the explicit validity policy, and only ever by being
        // cleared together with its payload.
        if let Some(invalidation) = self.revalidate_modal() {
            tracing::debug!("Cleared stale TUI modal: {}", invalidation.reason());
        }
    }

    /// End or preserve the persistent-idle episode for one incoming event.
    ///
    /// This is one shared rule spread over many event variants, so it lives here
    /// rather than being copied into each handler. It runs before dispatch so a
    /// variant handler already observes the resumed execution mode.
    ///
    /// A Start notification, a queue notification, `AnalysisStarted`, and a
    /// catalog refresh are all deliberately absent: none of them proves anything
    /// is executing, so none of them may take the frontend out of Ready.
    fn apply_persistent_idle_episode_rules(&mut self, event: &OrchestratorEvent) {
        use crate::tui::types::AppExecutionMode;

        if crate::events::is_admitted_work_start(event) {
            self.persistent_scheduler_idle = false;
            // A graceful stop that arrived first is still owed: work starting
            // under it does not withdraw the request, so Stopping is preserved.
            // A terminal mode is preserved for the same reason — only Ready is
            // waiting for this evidence.
            if self.execution_mode == AppExecutionMode::Select {
                self.execution_mode = AppExecutionMode::Running;
            }
            return;
        }

        // A terminal outcome ends the episode without resuming anything; the
        // variant handlers below still own the mode itself.
        if matches!(
            event,
            OrchestratorEvent::Stopped | OrchestratorEvent::Error { .. }
        ) {
            self.persistent_scheduler_idle = false;
        }
    }

    fn dispatch_orchestrator_event(&mut self, event: OrchestratorEvent) {
        match event {
            OrchestratorEvent::ProcessingStarted(id) => self.handle_processing_started(id),
            OrchestratorEvent::ProcessingError { id, error } => {
                self.handle_processing_error(id, error)
            }
            OrchestratorEvent::AllCompleted => self.handle_all_completed(),
            OrchestratorEvent::PersistentSchedulerIdle => self.handle_persistent_scheduler_idle(),
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
            OrchestratorEvent::ApplyFailed { change_id, error } => {
                self.handle_apply_failed(change_id, error)
            }
            OrchestratorEvent::ArchiveFailed {
                change_id, error, ..
            } => self.handle_archive_failed(change_id, error),
            OrchestratorEvent::ResolveFailed { change_id, error } => {
                self.handle_resolve_failed(change_id, error)
            }
            OrchestratorEvent::MergeDeferred {
                change_id,
                reason,
                auto_resumable,
            } => self.handle_merge_deferred(change_id, reason, auto_resumable),
            OrchestratorEvent::AcceptanceStarted { change_id, command } => {
                self.handle_acceptance_started(change_id, command)
            }
            OrchestratorEvent::AcceptanceCompleted { change_id } => {
                self.handle_acceptance_completed(change_id)
            }
            OrchestratorEvent::ChangeRejected { change_id, reason } => {
                self.handle_change_rejected(change_id, reason)
            }
            OrchestratorEvent::BranchMergeFailed { branch_name, error } => {
                self.handle_branch_merge_failed(branch_name, error)
            }
            OrchestratorEvent::HookFailed {
                change_id,
                hook_type,
                error,
            } => self.handle_hook_failed(change_id, hook_type, error),
            OrchestratorEvent::ChangeDequeued { change_id } => {
                self.handle_change_stopped(change_id)
            }
            OrchestratorEvent::ChangeStopped { change_id } => self.handle_change_stopped(change_id),
            OrchestratorEvent::ChangeStopFailed { change_id, error } => {
                self.handle_change_stop_failed(change_id, error)
            }
            OrchestratorEvent::ChangesRefreshed {
                changes,
                rejected_changes,
                committed_change_ids,
                uncommitted_file_change_ids,
                worktree_change_ids,
                worktree_paths,
                worktree_not_ahead_ids,
                merge_wait_ids,
            } => self.handle_changes_refreshed(
                changes,
                rejected_changes,
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
            OrchestratorEvent::ApplyCommitPhase {
                change_id,
                phase,
                attempt,
            } => self.handle_apply_commit_phase(change_id, phase, attempt),
            OrchestratorEvent::ApplyCommitOutput {
                change_id,
                attempt,
                stream,
                line,
            } => self.handle_apply_commit_output(change_id, attempt, stream, line),
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
            OrchestratorEvent::AnalysisStarted {
                remaining_changes,
                attempt_id,
            } => self.handle_analysis_started(remaining_changes, attempt_id),
            OrchestratorEvent::Log(entry) => self.handle_log(entry),
            OrchestratorEvent::Warning { title, message } => self.handle_warning(title, message),
            OrchestratorEvent::ParallelStartRejected { change_ids, reason } => {
                self.handle_parallel_start_rejected(change_ids, reason)
            }
            OrchestratorEvent::Error { message } => self.handle_error(message),
            _ => {}
        }
    }
}
