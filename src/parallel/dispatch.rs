//! Change dispatch logic for parallel execution.
//!
//! This module handles spawning individual change execution tasks into worktrees:
//! - Pre-flight checks (stopped changes, duplicate dispatch prevention)
//! - Workspace acquisition (semaphore-gated)
//! - Apply + Acceptance + Archive pipeline execution
//! - Per-change cancellation monitoring

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::agent::AgentRunner;
use crate::error::{OrchestratorError, Result};
use crate::events::LogEntry;
use crate::execution::state::{detect_workspace_state, WorkspaceState};
use crate::orchestration::{
    execute_rejection_flow, handle_resume_apply_from_rejecting, run_rejection_review,
    RejectionReviewVerdict,
};
use crate::task_parser;
use crate::vcs::WorkspaceStatus;

use super::cleanup::WorkspaceCleanupGuard;
use super::events::send_event;
use super::executor::{
    execute_acceptance_in_workspace, execute_apply_in_workspace, execute_archive_in_workspace,
};
use super::types::WorkspaceResult;
use super::workspace;
use super::ParallelEvent;
use super::ParallelExecutor;

#[cfg(test)]
mod tests {
    use super::{decide_resume_action, should_run_apply, ResumeAction};
    use crate::execution::state::WorkspaceState;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn decide_resume_action_routes_applied_with_incomplete_tasks_to_apply() {
        let tmp = TempDir::new().unwrap();
        let tasks_dir = tmp.path().join("openspec/changes/change-incomplete");
        fs::create_dir_all(&tasks_dir).unwrap();
        fs::write(
            tasks_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n- [ ] pending\n",
        )
        .unwrap();

        let action =
            decide_resume_action("change-incomplete", tmp.path(), &WorkspaceState::Applied);
        assert_eq!(action, ResumeAction::Apply);
    }

    #[test]
    fn decide_resume_action_routes_applied_with_complete_tasks_to_acceptance() {
        let tmp = TempDir::new().unwrap();
        let tasks_dir = tmp.path().join("openspec/changes/change-complete");
        fs::create_dir_all(&tasks_dir).unwrap();
        fs::write(
            tasks_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n- [x] done2\n",
        )
        .unwrap();

        let action = decide_resume_action("change-complete", tmp.path(), &WorkspaceState::Applied);
        assert_eq!(action, ResumeAction::Acceptance);
    }

    #[test]
    fn decide_resume_action_keeps_archived_as_terminal() {
        let tmp = TempDir::new().unwrap();
        let action = decide_resume_action("change-archived", tmp.path(), &WorkspaceState::Archived);
        assert_eq!(action, ResumeAction::Terminal);
    }

    #[test]
    fn should_run_apply_consumes_skip_flag_after_first_cycle() {
        let mut skip_apply_once = true;

        assert!(!should_run_apply(&mut skip_apply_once));
        assert!(!skip_apply_once);
        assert!(should_run_apply(&mut skip_apply_once));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResumeAction {
    Terminal,
    Apply,
    Acceptance,
    Rejecting,
}

pub(super) fn decide_resume_action(
    change_id: &str,
    workspace_path: &std::path::Path,
    state: &WorkspaceState,
) -> ResumeAction {
    match state {
        WorkspaceState::Merged | WorkspaceState::Archived => ResumeAction::Terminal,
        WorkspaceState::Archiving => ResumeAction::Acceptance,
        WorkspaceState::Rejecting => ResumeAction::Rejecting,
        WorkspaceState::Applied => {
            match task_parser::parse_progress_with_fallback(change_id, Some(workspace_path)) {
                Ok(progress) if progress.total > 0 && progress.completed >= progress.total => {
                    ResumeAction::Acceptance
                }
                Ok(_) | Err(_) => ResumeAction::Apply,
            }
        }
        WorkspaceState::Created | WorkspaceState::Applying { .. } => ResumeAction::Apply,
    }
}

fn should_run_apply(skip_apply_once: &mut bool) -> bool {
    if *skip_apply_once {
        *skip_apply_once = false;
        false
    } else {
        true
    }
}

impl ParallelExecutor {
    /// Dispatch a single change to a workspace for apply + acceptance + archive.
    ///
    /// This method:
    /// - Checks if the change has been stopped or is already in-flight
    /// - Acquires a semaphore permit (to enforce concurrency limits)
    /// - Creates or resumes a workspace
    /// - Spawns an async task for apply + acceptance + archive pipeline
    ///
    /// The spawned task will:
    /// - Execute apply command
    /// - Execute acceptance test (with retry loop)
    /// - Execute archive command (only if acceptance passes)
    /// - Return WorkspaceResult
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn dispatch_change_to_workspace(
        &mut self,
        change_id: String,
        base_revision: String,
        semaphore: Arc<Semaphore>,
        join_set: &mut JoinSet<WorkspaceResult>,
        in_flight: &mut HashSet<String>,
        cleanup_guard: &mut WorkspaceCleanupGuard,
    ) -> Result<()> {
        // Check if this change has been stopped (single-change stop)
        if let Some(ref queue) = self.dynamic_queue {
            if queue.is_stopped(&change_id).await {
                queue.clear_stopped(&change_id).await;
                info!("Change '{}' stopped before dispatch", change_id);
                send_event(
                    &self.event_tx,
                    ParallelEvent::ChangeDequeued {
                        change_id: change_id.clone(),
                    },
                )
                .await;
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::info(format!("Change stopped: {}", change_id))),
                )
                .await;
                return Ok(());
            }
        }

        // Check if already in-flight (avoid duplicate dispatch)
        if in_flight.contains(&change_id) {
            warn!(
                "Change '{}' already in-flight, skipping dispatch",
                change_id
            );
            return Ok(());
        }

        // Acquire semaphore permit
        let permit = semaphore.clone().acquire_owned().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to acquire semaphore: {}", e))
        })?;

        // Create or reuse workspace; was_resumed=true means an existing workspace was reused.
        let (workspace_val, was_resumed) = workspace::get_or_create_workspace(
            self.workspace_manager.as_mut(),
            &change_id,
            &base_revision,
            self.no_resume,
            &self.force_recreate_worktree,
            &self.event_tx,
        )
        .await?;

        // Track workspace for cleanup
        cleanup_guard.track(workspace_val.name.clone(), workspace_val.path.clone());

        // Add to in-flight set
        in_flight.insert(change_id.clone());

        // Prepare context for spawned task
        let apply_command = self.apply_command.clone();
        let archive_command = self.archive_command.clone();
        let repo_root = self.repo_root.clone();
        let config = self.config.clone();
        let event_tx = self.event_tx.clone();
        let vcs_backend = self.workspace_manager.backend_type();
        let ai_runner = self.ai_runner.clone();
        let apply_history = self.apply_history.clone();
        let archive_history = self.archive_history.clone();
        let acceptance_history = self.acceptance_history.clone();
        let acceptance_tail_injected = self.acceptance_tail_injected.clone();
        let cancel_token = self.cancel_token.clone();
        let shared_stagger_state = self.shared_stagger_state.clone();
        let base_branch = self
            .workspace_manager
            .ensure_original_branch_initialized()
            .await
            .map_err(OrchestratorError::from_vcs_error)?;
        let dynamic_queue = self.dynamic_queue.clone();
        let workspace = workspace_val;

        // Spawn apply + acceptance + archive task
        join_set.spawn(async move {
            let _permit = permit; // Hold permit until task completes

            // Detect workspace state for resumed workspaces and route accordingly.
            // A new workspace always starts fresh (Created state).
            // A resumed workspace may be in any state; we must not blindly run the full
            // pipeline for terminal states (Archived, Merged) or already-applied states.
            let effective_state = if was_resumed {
                match detect_workspace_state(&change_id, &workspace.path, &base_branch).await {
                    Ok(state) => {
                        let state_label = format!("{:?}", state);
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::info(format!(
                                        "Resuming existing workspace for {} (detected state: {})",
                                        change_id, state_label
                                    ))
                                    .with_change_id(&change_id),
                                ))
                                .await;
                        }
                        state
                    }
                    Err(e) => {
                        warn!(
                            "State detection failed for '{}': {}, treating as Created",
                            change_id, e
                        );
                        WorkspaceState::Created
                    }
                }
            } else {
                WorkspaceState::Created
            };

            let resume_action = if was_resumed {
                decide_resume_action(&change_id, &workspace.path, &effective_state)
            } else {
                ResumeAction::Apply
            };

            if was_resumed {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::Log(
                            LogEntry::info(format!(
                                "Resume routing for {}: state={:?} -> {:?}",
                                change_id, effective_state, resume_action
                            ))
                            .with_change_id(&change_id),
                        ))
                        .await;
                }
            }

            // Early return for terminal states: Archived and Merged workspaces must not
            // re-enter the apply/acceptance/archive pipeline.  Doing so silently creates
            // duplicate apply commits or masks already-complete work as a fresh start.
            if matches!(resume_action, ResumeAction::Terminal) {
                match &effective_state {
                    WorkspaceState::Merged => {
                    info!(
                        "Change '{}' workspace already merged to base, skipping all processing",
                        change_id
                    );
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::info(format!(
                                    "Change {} skipped: workspace already merged to base branch",
                                    change_id
                                ))
                                .with_change_id(&change_id),
                            ))
                            .await;
                    }
                    // cancel_monitor has not been spawned yet at this point,
                    // so we return without aborting it.
                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: None,
                        rejected: None,
                    };
                }
                WorkspaceState::Archived => {
                    // The workspace is already past the archive step.  We must hand it
                    // off to merge handling rather than silently returning a no-op result
                    // with final_revision=None (which would cause the change to disappear
                    // from the queue lifecycle and never reach MergeWait).
                    info!(
                        "Change '{}' workspace already archived on resume, handing off to merge",
                        change_id
                    );
                    // Get the current HEAD revision of the worktree — this is the
                    // archive commit that the merge step needs.
                    let resume_revision =
                        crate::vcs::git::commands::get_current_commit(&workspace.path).await;
                    match resume_revision {
                        Ok(rev) => {
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::info(format!(
                                            "Change {} resumed: workspace already archived, entering merge handling",
                                            change_id
                                        ))
                                        .with_change_id(&change_id),
                                    ))
                                    .await;
                                // Emit the same ChangeArchived event as the normal archive
                                // success path so that downstream state machines (TUI,
                                // output bridge) treat this resume identically.
                                let _ = tx
                                    .send(ParallelEvent::ChangeArchived(change_id.clone()))
                                    .await;
                            }
                            // cancel_monitor has not been spawned yet at this point,
                            // so we return without aborting it.
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: Some(rev),
                                error: None,
                                rejected: None,
                            };
                        }
                        Err(e) => {
                            // Could not read the revision — treat as a transient error so
                            // the orchestrator can surface it rather than silently dropping
                            // the change from the queue.
                            warn!(
                                "Change '{}' archived on resume but revision read failed: {}",
                                change_id, e
                            );
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Archived resume: failed to read workspace revision: {}",
                                    e
                                )),
                                rejected: None,
                            };
                        }
                    }
                }
                    _ => {}
                }
            }

            // Create agent for acceptance testing
            let mut agent =
                AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());

            // Track apply+acceptance cycles to prevent infinite loops
            const MAX_APPLY_ACCEPTANCE_CYCLES: u32 = 10;
            let mut cycle_count = 0u32;
            let mut cumulative_iteration = 0u32; // Track total apply iterations across all cycles

            // Create a per-change cancel token that monitors both global cancel and single-change stop
            let per_change_cancel = CancellationToken::new();
            let monitor_cancel = per_change_cancel.clone();
            let monitor_global = cancel_token.clone();
            let monitor_queue = dynamic_queue.clone();
            let monitor_change_id = change_id.clone();

            // Spawn a background task to monitor both cancellation sources
            let cancel_monitor = tokio::spawn(async move {
                loop {
                    // Check global cancellation
                    if let Some(ref token) = monitor_global {
                        if token.is_cancelled() {
                            monitor_cancel.cancel();
                            break;
                        }
                    }

                    // Check single-change stop
                    if let Some(ref queue) = monitor_queue {
                        if queue.is_stopped(&monitor_change_id).await {
                            monitor_cancel.cancel();
                            break;
                        }
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            });

            // Apply+Acceptance loop: retry apply when acceptance fails.
            // Resume routing determines whether we start from apply or acceptance.
            // The acceptance-only shortcut is consumed after one cycle so that any
            // acceptance FAIL/command error path re-enters apply on the next cycle.
            // Rejecting resumes are handled as an immediate rejection review branch.
            if matches!(resume_action, ResumeAction::Rejecting) {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::WorkspaceStatusUpdated {
                            workspace_name: workspace.name.clone(),
                            status: WorkspaceStatus::Rejecting,
                        })
                        .await;
                }

                match run_rejection_review(&change_id, &workspace.path, &config, &ai_runner).await {
                    Ok(RejectionReviewVerdict::Confirm) => {
                        let rejected_path = workspace
                            .path
                            .join("openspec")
                            .join("changes")
                            .join(&change_id)
                            .join("REJECTED.md");
                        let reason = format!(
                            "Rejecting review confirmed rejection (proposal: {})",
                            rejected_path.display()
                        );
                        let resolved_base = base_branch.clone();
                        match execute_rejection_flow(
                            &change_id,
                            &reason,
                            &workspace.path,
                            &resolved_base,
                            &repo_root,
                        )
                        .await
                        {
                            Ok(()) => {
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ChangeRejected {
                                            change_id: change_id.clone(),
                                            reason: reason.clone(),
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::ChangeDequeued {
                                            change_id: change_id.clone(),
                                        })
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None,
                                    rejected: Some(reason),
                                };
                            }
                            Err(e) => {
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: Some(format!(
                                        "Rejected flow failed after rejecting CONFIRM verdict: {}",
                                        e
                                    )),
                                    rejected: None,
                                };
                            }
                        }
                    }
                    Ok(RejectionReviewVerdict::Resume) => {
                        if let Err(e) = handle_resume_apply_from_rejecting(&change_id, &workspace.path).await {
                            cancel_monitor.abort();
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Failed to resume apply from rejecting verdict: {}",
                                    e
                                )),
                                rejected: None,
                            };
                        }
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::warn("Rejecting review returned RESUME; returning to apply loop")
                                        .with_change_id(&change_id)
                                        .with_operation("rejecting"),
                                ))
                                .await;
                        }
                    }
                    Err(e) => {
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!(
                                "Rejecting review failed while resuming rejecting stage: {}",
                                e
                            )),
                            rejected: None,
                        };
                    }
                }
            }
            let mut skip_apply_once = matches!(resume_action, ResumeAction::Acceptance);

            let _apply_revision = loop {
                // Skip apply only for the first cycle when resuming from an already-applied state.
                if !should_run_apply(&mut skip_apply_once) {
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::info(format!(
                                    "Skipping apply for {} (workspace already in {:?} state)",
                                    change_id, effective_state
                                ))
                                .with_change_id(&change_id),
                            ))
                            .await;
                    }
                    break String::new();
                }

                // Check if this change has been stopped (single-change stop)
                if let Some(ref queue) = dynamic_queue {
                    if queue.is_stopped(&change_id).await {
                        queue.clear_stopped(&change_id).await;
                        info!("Change '{}' stopped during execution", change_id);
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ChangeDequeued {
                                    change_id: change_id.clone(),
                                })
                                .await;
                            let _ = tx
                                .send(ParallelEvent::Log(LogEntry::info(format!(
                                    "Change stopped: {}",
                                    change_id
                                ))))
                                .await;
                        }
                        cancel_monitor.abort();
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: None, // No error - intentionally stopped
                                        rejected: None,
                                    };



                    }
                }

                cycle_count += 1;
                if cycle_count > MAX_APPLY_ACCEPTANCE_CYCLES {
                    error!(
                        "Max apply+acceptance cycles ({}) reached for {}",
                        MAX_APPLY_ACCEPTANCE_CYCLES, change_id
                    );
                    cancel_monitor.abort();
                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: Some(format!(
                            "Max apply+acceptance cycles ({}) reached",
                            MAX_APPLY_ACCEPTANCE_CYCLES
                        )),
                        rejected: None,
                    };
                }

                // Step 1: Execute apply with cumulative iteration count
                // Use per-change cancel token that monitors both global and single-change stop
                let apply_result = execute_apply_in_workspace(
                    &change_id,
                    &workspace.path,
                    &apply_command,
                    &config,
                    event_tx.clone(),
                    vcs_backend,
                    None, // hooks
                    None, // parallel_ctx
                    Some(&per_change_cancel),
                    &ai_runner,
                    &repo_root,
                    &apply_history,
                    &acceptance_history,
                    &acceptance_tail_injected,
                    cumulative_iteration, // Pass current iteration count
                )
                .await;

                let (revision, final_iteration, blocked_handoff) = match apply_result {
                    Ok((rev, iter, blocked_handoff)) => (rev, iter, blocked_handoff),
                    Err(e) => {
                        // Check if this was a single-change stop
                        let error_str = e.to_string();
                        if error_str.contains("Cancelled") {
                            if let Some(ref queue) = dynamic_queue {
                                if queue.is_stopped(&change_id).await {
                                    queue.clear_stopped(&change_id).await;
                                    info!("Change '{}' stopped during apply", change_id);
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx
                                            .send(ParallelEvent::ChangeDequeued {
                                                change_id: change_id.clone(),
                                            })
                                            .await;
                                        let _ = tx
                                            .send(ParallelEvent::Log(LogEntry::info(format!(
                                                "Change stopped: {}",
                                                change_id
                                            ))))
                                            .await;
                                    }
                                    cancel_monitor.abort();
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: None, // No error - intentionally stopped
                                        rejected: None,
                                    };
                                }
                            }
                        }
                        // Apply failed - return error immediately
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Apply failed: {}", e)),
                            rejected: None,
                        };
                    }
                };

                // Update cumulative iteration count
                cumulative_iteration = final_iteration;

                if let Some(handoff) = &blocked_handoff {
                    info!(
                        change_id = %change_id,
                        rejected_path = %handoff.rejected_path.display(),
                        "Apply emitted blocked handoff proposal; routing to rejecting stage"
                    );
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::WorkspaceStatusUpdated {
                                workspace_name: workspace.name.clone(),
                                status: WorkspaceStatus::Rejecting,
                            })
                            .await;
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::warn(format!(
                                    "Apply blocked handoff detected via {}; entering rejecting stage",
                                    handoff.rejected_path.display()
                                ))
                                .with_change_id(&change_id)
                                .with_operation("rejecting"),
                            ))
                            .await;
                    }

                    match run_rejection_review(&change_id, &workspace.path, &config, &ai_runner).await {
                        Ok(RejectionReviewVerdict::Confirm) => {
                            let reason = format!(
                                "Apply-blocked rejection confirmed by rejecting review (proposal: {})",
                                handoff.rejected_path.display()
                            );
                            let resolved_base = base_branch.clone();
                            match execute_rejection_flow(
                                &change_id,
                                &reason,
                                &workspace.path,
                                &resolved_base,
                                &repo_root,
                            )
                            .await
                            {
                                Ok(()) => {
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx
                                            .send(ParallelEvent::ChangeRejected {
                                                change_id: change_id.clone(),
                                                reason: reason.clone(),
                                            })
                                            .await;
                                        let _ = tx
                                            .send(ParallelEvent::ChangeDequeued {
                                                change_id: change_id.clone(),
                                            })
                                            .await;
                                    }
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: None,
                                        rejected: Some(reason),
                                    };
                                }
                                Err(e) => {
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: Some(format!(
                                            "Rejected flow failed after rejecting CONFIRM verdict: {}",
                                            e
                                        )),
                                        rejected: None,
                                    };
                                }
                            }
                        }
                        Ok(RejectionReviewVerdict::Resume) => {
                            if let Err(e) = handle_resume_apply_from_rejecting(&change_id, &workspace.path).await {
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: Some(format!(
                                        "Failed to resume apply from rejecting verdict: {}",
                                        e
                                    )),
                                    rejected: None,
                                };
                            }
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::warn("Rejecting review returned RESUME; returning to apply loop")
                                            .with_change_id(&change_id)
                                            .with_operation("rejecting"),
                                    ))
                                    .await;
                            }
                            continue;
                        }
                        Err(e) => {
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Rejecting review failed after apply blocked handoff: {}",
                                    e
                                )),
                                rejected: None,
                            };
                        }
                    }
                }

                // Send ApplyCompleted event
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ApplyCompleted {
                            change_id: change_id.clone(),
                            revision: revision.clone(),
                        })
                        .await;
                }

                // Step 2: Execute acceptance test after apply succeeds
                // IMPORTANT: Acceptance results are NOT persisted to disk or git commits.
                // This means acceptance will always run after apply completes, even on resume.
                // This ensures quality gates are enforced regardless of interruptions.

                // Update status to Accepting
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::WorkspaceStatusUpdated {
                            workspace_name: workspace.name.clone(),
                            status: WorkspaceStatus::Accepting,
                        })
                        .await;
                }

                info!(
                    "Running acceptance test for {} after apply completion (cycle {})",
                    change_id, cycle_count
                );
                let acceptance_result = execute_acceptance_in_workspace(
                    &change_id,
                    &workspace.path,
                    &mut agent,
                    event_tx.clone(),
                    Some(&per_change_cancel),
                    &ai_runner,
                    &config,
                    &acceptance_tail_injected,
                    &acceptance_history,
                    Some(base_branch.as_str()),
                )
                .await;

                match acceptance_result {
                    Ok((crate::orchestration::AcceptanceResult::Pass, _acceptance_iteration)) => {
                        info!("Acceptance passed for {}, proceeding to archive", change_id);
                        // Break out of loop, proceed to archive
                        break revision;
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::Continue,
                        acceptance_iteration,
                    )) => {
                        let continue_count =
                            agent.count_consecutive_acceptance_continues(&change_id);
                        let max_continues = config.get_acceptance_max_continues();

                        if continue_count >= max_continues {
                            warn!(
                                "Acceptance CONTINUE limit ({}) exceeded for {} (cycle {}), treating as FAIL",
                                max_continues, change_id, cycle_count
                            );
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::warn(format!(
                                            "Acceptance CONTINUE limit exceeded (cycle {}), change will not be archived",
                                            cycle_count
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Acceptance CONTINUE limit ({}) exceeded",
                                    max_continues
                                )),
                                rejected: None,
                            };
                        } else {
                            info!(
                                "Acceptance requires continuation for {} (attempt {}/{}, cycle {}), retrying acceptance",
                                change_id,
                                continue_count,
                                max_continues,
                                cycle_count
                            );
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::info(format!(
                                            "Acceptance requires continuation (attempt {}/{}, cycle {}), retrying",
                                            continue_count,
                                            max_continues,
                                            cycle_count
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            // Continue the acceptance loop - retry acceptance without re-applying
                            continue;
                        }
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::Fail { findings },
                        acceptance_iteration,
                    )) => {
                        let blocking_gate_context = findings
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "no acceptance findings captured".to_string());
                        warn!(
                            "Acceptance failed for {} ({} findings) (cycle {}), blocking gate context: {}; returning to apply loop",
                            change_id,
                            findings.len(),
                            cycle_count,
                            blocking_gate_context
                        );
                        // Note: tasks.md is now updated by the acceptance agent itself
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::warn(format!(
                                        "Acceptance failed ({} findings), blocking gate context: {}; returning to apply loop (cycle {})",
                                        findings.len(),
                                        blocking_gate_context,
                                        cycle_count
                                    ))
                                    .with_change_id(&change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ))
                                .await;
                        }
                        // Continue loop - retry apply with updated tasks
                        continue;
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::CommandFailed {
                            error,
                            findings: _,
                        },
                        acceptance_iteration,
                    )) => {
                        error!(
                            "Acceptance command failed for {} (cycle {}): {}",
                            change_id, cycle_count, error
                        );
                        // Note: tasks.md is now updated by the acceptance agent itself
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::error(format!(
                                        "Acceptance command failed (cycle {}): {}",
                                        cycle_count, error
                                    ))
                                    .with_change_id(&change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ))
                                .await;
                        }
                        // Command failed - this is a critical error, don't retry
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Acceptance command failed: {}", error)),
                            rejected: None,
                        };
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::Blocked,
                        acceptance_iteration,
                    )) => {
                        let reason = blocked_handoff
                            .as_ref()
                            .map(|handoff| {
                                format!(
                                    "Acceptance-confirmed apply blocker (proposal: {})",
                                    handoff.rejected_path.display()
                                )
                            })
                            .unwrap_or_else(|| {
                                "Acceptance-confirmed implementation blocker".to_string()
                            });
                        warn!(
                            "Acceptance blocked for {} - running rejection flow",
                            change_id
                        );

                        let resolved_base = base_branch.clone();

                        match execute_rejection_flow(
                            &change_id,
                            &reason,
                            &workspace.path,
                            &resolved_base,
                            &repo_root,
                        )
                        .await
                        {
                            Ok(()) => {
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::warn(format!(
                                                "Acceptance blocked - rejection flow completed ({})",
                                                resolved_base
                                            ))
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::ChangeDequeued {
                                            change_id: change_id.clone(),
                                        })
                                        .await;
                                }

                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None,
                                    rejected: Some(reason),
                                };
                            }
                            Err(e) => {
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: Some(format!(
                                        "Rejected flow failed after blocked acceptance: {}",
                                        e
                                    )),
                                    rejected: None,
                                };
                            }
                        }
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::Cancelled,
                        _acceptance_iteration,
                    )) => {
                        // Check if this was a single-change stop
                        if let Some(ref queue) = dynamic_queue {
                            if queue.is_stopped(&change_id).await {
                                queue.clear_stopped(&change_id).await;
                                info!("Change '{}' stopped during acceptance", change_id);
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ChangeDequeued {
                                            change_id: change_id.clone(),
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::Log(LogEntry::info(format!(
                                            "Change stopped: {}",
                                            change_id
                                        ))))
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None, // No error - intentionally stopped
                                    rejected: None,
                                };
                            }
                        }
                        // Global cancellation
                        info!("Acceptance cancelled for {}", change_id);
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some("Acceptance cancelled".to_string()),
                            rejected: None,
                        };
                    }
                    Err(e) => {
                        // Check if this was a single-change stop (error contains "Cancelled")
                        let error_str = e.to_string();
                        if error_str.contains("Cancelled") {
                            if let Some(ref queue) = dynamic_queue {
                                if queue.is_stopped(&change_id).await {
                                    queue.clear_stopped(&change_id).await;
                                    info!("Change '{}' stopped during acceptance", change_id);
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx
                                            .send(ParallelEvent::ChangeDequeued {
                                                change_id: change_id.clone(),
                                            })
                                            .await;
                                        let _ = tx
                                            .send(ParallelEvent::Log(LogEntry::info(format!(
                                                "Change stopped: {}",
                                                change_id
                                            ))))
                                            .await;
                                    }
                                    cancel_monitor.abort();
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: None, // No error - intentionally stopped
                                        rejected: None,
                                    };
                                }
                            }
                        }
                        error!("Acceptance error for {}: {}", change_id, e);
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Acceptance error: {}", e)),
                            rejected: None,
                        };
                    }
                }
            };

            // Step 3: Execute archive after acceptance passes
            // Update status to Archiving
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::WorkspaceStatusUpdated {
                        workspace_name: workspace.name.clone(),
                        status: WorkspaceStatus::Archiving,
                    })
                    .await;
            }

            // ArchiveStarted event is sent inside execute_archive_in_workspace with command string
            let archive_result = execute_archive_in_workspace(
                &change_id,
                &workspace.path,
                &archive_command,
                &config,
                event_tx.clone(),
                vcs_backend,
                None, // hooks
                None, // parallel_ctx
                Some(&per_change_cancel),
                &ai_runner,
                &archive_history,
                &apply_history,
                &shared_stagger_state,
            )
            .await;

            match archive_result {
                Ok(archive_revision) => {
                    // Archive succeeded
                    agent.clear_acceptance_history(&change_id);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::ChangeArchived(change_id.clone()))
                            .await;
                    }
                    cancel_monitor.abort();
                    WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: Some(archive_revision),
                        error: None,
                        rejected: None,
                    }
                }
                Err(e) => {
                    // Check if this was a single-change stop
                    if e.to_string().contains("Cancelled") {
                        if let Some(ref queue) = dynamic_queue {
                            if queue.is_stopped(&change_id).await {
                                queue.clear_stopped(&change_id).await;
                                info!("Change '{}' stopped during archive", change_id);
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ChangeDequeued {
                                            change_id: change_id.clone(),
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::Log(LogEntry::info(format!(
                                            "Change stopped: {}",
                                            change_id
                                        ))))
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None, // No error - intentionally stopped
                                    rejected: None,
                                };
                            }
                        }
                    }
                    warn!("Archive failed for {}: {}", change_id, e);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::ArchiveFailed {
                                change_id: change_id.clone(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                    cancel_monitor.abort();
                    // Archive failed - do not merge unarchived changes
                    WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: Some(format!("Archive failed: {}", e)),
                        rejected: None,
                    }
                }
            }
            // _permit is dropped here, releasing semaphore
        });

        Ok(())
    }
}
