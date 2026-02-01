//! TuiCommand handlers for TUI
//!
//! This module contains helper functions to handle TuiCommand processing.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};
use crate::tui::orchestrator::{run_orchestrator, run_orchestrator_parallel};
use crate::tui::queue::DynamicQueue;
use crate::tui::state::AppState;
use crate::tui::types::{AppMode, QueueStatus, StopMode};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::worktrees::load_worktrees_with_conflict_check;

/// Context for TuiCommand handling
pub struct TuiCommandContext<'a> {
    pub app: &'a mut AppState,
    pub repo_root: &'a Path,
    pub config: &'a OrchestratorConfig,
    pub tx: &'a mpsc::Sender<OrchestratorEvent>,
    pub dynamic_queue: &'a DynamicQueue,
    #[cfg(feature = "web-monitoring")]
    pub web_state: &'a Option<Arc<crate::web::WebState>>,
}

/// Handle TuiCommand::StartProcessing
pub async fn handle_start_processing_command(
    ids: Vec<String>,
    ctx: &mut TuiCommandContext<'_>,
    graceful_stop_flag: &Arc<std::sync::atomic::AtomicBool>,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    manual_resolve_counter: &Arc<AtomicUsize>,
    orchestrator_cancel: &mut Option<CancellationToken>,
) -> Option<tokio::task::JoinHandle<Result<()>>> {
    // Handle web control Start command (empty ids vec) or regular Start
    let cmd = if ids.is_empty() {
        // Web control start - determine which command based on app mode
        if ctx.app.mode == AppMode::Error {
            ctx.app.retry_error_changes()
        } else if ctx.app.mode == AppMode::Stopped {
            ctx.app.resume_processing()
        } else {
            ctx.app.start_processing()
        }
    } else {
        // Regular start with specific IDs (from F5 key)
        Some(TuiCommand::StartProcessing(ids.clone()))
    };

    if let Some(TuiCommand::StartProcessing(selected_ids)) = cmd {
        if !selected_ids.is_empty() {
            graceful_stop_flag.store(false, Ordering::SeqCst);
            let orch_tx = ctx.tx.clone();
            let orch_config = ctx.config.clone();
            let orch_cancel = CancellationToken::new();
            let orch_dynamic_queue = ctx.dynamic_queue.clone();
            let orch_graceful_stop = graceful_stop_flag.clone();
            let orch_shared_state = shared_state.clone();
            let orch_manual_resolve = manual_resolve_counter.clone();
            *orchestrator_cancel = Some(orch_cancel.clone());
            let use_parallel = ctx.app.parallel_mode;
            #[cfg(feature = "web-monitoring")]
            let orch_web_state = ctx.web_state.clone();

            return Some(tokio::spawn(async move {
                #[cfg(feature = "web-monitoring")]
                let result = if use_parallel {
                    run_orchestrator_parallel(
                        selected_ids,
                        orch_config,
                        orch_tx.clone(),
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_manual_resolve.clone(),
                        orch_web_state,
                    )
                    .await
                } else {
                    run_orchestrator(
                        selected_ids,
                        orch_config,
                        orch_tx.clone(),
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_web_state,
                    )
                    .await
                };
                #[cfg(not(feature = "web-monitoring"))]
                let result = if use_parallel {
                    run_orchestrator_parallel(
                        selected_ids,
                        orch_config,
                        orch_tx.clone(),
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_manual_resolve,
                    )
                    .await
                } else {
                    run_orchestrator(
                        selected_ids,
                        orch_config,
                        orch_tx.clone(),
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                    )
                    .await
                };
                // NOTE: Do not send Stopped here unconditionally.
                // The orchestrator already sends AllCompleted on normal completion
                // or Stopped when explicitly stopped via graceful_stop_flag.
                result
            }));
        }
    }
    None
}

/// Handle TuiCommand - main dispatcher
///
/// Returns Some(JoinHandle) if a new orchestrator task was spawned
#[allow(clippy::too_many_arguments)]
pub async fn handle_tui_command(
    cmd: TuiCommand,
    ctx: &mut TuiCommandContext<'_>,
    graceful_stop_flag: &Arc<std::sync::atomic::AtomicBool>,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    manual_resolve_counter: &Arc<AtomicUsize>,
    orchestrator_cancel: &mut Option<CancellationToken>,
) -> Result<Option<tokio::task::JoinHandle<Result<()>>>> {
    match cmd {
        TuiCommand::StartProcessing(ids) => {
            let handle = handle_start_processing_command(
                ids,
                ctx,
                graceful_stop_flag,
                shared_state,
                manual_resolve_counter,
                orchestrator_cancel,
            )
            .await;
            return Ok(handle);
        }
        TuiCommand::AddToQueue(id) => {
            // Push to dynamic queue for orchestrator to pick up
            if ctx.dynamic_queue.push(id.clone()).await {
                ctx.app
                    .add_log(LogEntry::info(format!("Added to dynamic queue: {}", id)));
            } else {
                ctx.app
                    .add_log(LogEntry::warn(format!("Already in dynamic queue: {}", id)));
            }
        }
        TuiCommand::RemoveFromQueue(id) => {
            // Remove from dynamic queue so orchestrator won't process it
            let removed_from_dynamic = ctx.dynamic_queue.remove(&id).await;
            let removed_from_pending = ctx.dynamic_queue.mark_removed(id.clone()).await;
            let mut details = Vec::new();
            if removed_from_dynamic {
                details.push("also removed from dynamic queue");
            }
            if removed_from_pending {
                details.push("removed from pending");
            }
            let suffix = if details.is_empty() {
                String::new()
            } else {
                format!(" ({})", details.join(", "))
            };
            ctx.app.add_log(LogEntry::info(format!(
                "Removed from queue: {}{}",
                id, suffix
            )));
        }
        TuiCommand::ApproveOnly(id) => {
            // Approve without adding to queue (select/running/stopped modes)
            use crate::approval;

            match approval::approve_change(&id) {
                Ok(_) => {
                    ctx.app.update_approval_status(&id, true);
                    if ctx.app.mode == AppMode::Select {
                        if let Some(change) = ctx.app.changes.iter_mut().find(|c| c.id == id) {
                            // In select mode, default to selecting approved changes.
                            // However, wait states must preserve queue_status/selected semantics.
                            if !matches!(
                                change.queue_status,
                                QueueStatus::MergeWait | QueueStatus::ResolveWait
                            ) {
                                change.selected = true;
                                change.queue_status = QueueStatus::NotQueued;
                            }
                        }
                    }
                    ctx.app
                        .add_log(LogEntry::info(format!("Approved (not queued): {}", id)));
                }
                Err(e) => {
                    ctx.app.add_log(LogEntry::error(format!(
                        "Failed to approve '{}': {}",
                        id, e
                    )));
                }
            }
        }
        TuiCommand::UnapproveOnly(id) => {
            // Unapprove without touching queue status or DynamicQueue.
            // Intended for wait states like MergeWait/ResolveWait.
            use crate::approval;

            match approval::unapprove_change(&id) {
                Ok(_) => {
                    ctx.app.update_approval_status(&id, false);
                    if let Some(change) = ctx.app.changes.iter_mut().find(|c| c.id == id) {
                        change.selected = false;
                    }
                    ctx.app.add_log(LogEntry::info(format!(
                        "Unapproved (no queue change): {}",
                        id
                    )));
                }
                Err(e) => {
                    ctx.app.add_log(LogEntry::error(format!(
                        "Failed to unapprove '{}': {}",
                        id, e
                    )));
                }
            }
        }
        TuiCommand::UnapproveAndDequeue(id) => {
            // Unapprove and remove from queue (used in running/completed mode)
            use crate::approval;

            // First check if queued
            let was_queued = ctx
                .app
                .changes
                .iter()
                .find(|c| c.id == id)
                .map(|c| matches!(c.queue_status, QueueStatus::Queued))
                .unwrap_or(false);

            match approval::unapprove_change(&id) {
                Ok(_) => {
                    ctx.app.update_approval_status(&id, false);
                    // Also remove from queue if queued
                    if let Some(change) = ctx.app.changes.iter_mut().find(|c| c.id == id) {
                        if matches!(change.queue_status, QueueStatus::Queued) {
                            change.queue_status = QueueStatus::NotQueued;
                        }
                        change.selected = false;
                    }
                    // Remove from dynamic queue so orchestrator won't process it
                    let removed_from_dynamic = ctx.dynamic_queue.remove(&id).await;
                    let removed_from_pending = ctx.dynamic_queue.mark_removed(id.clone()).await;
                    let mut details = Vec::new();
                    if removed_from_dynamic {
                        details.push("also removed from dynamic queue");
                    }
                    if removed_from_pending {
                        details.push("removed from pending");
                    }
                    let suffix = if details.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", details.join(", "))
                    };
                    let msg = if was_queued {
                        format!("Unapproved and removed from queue: {}{}", id, suffix)
                    } else {
                        format!("Unapproved: {}{}", id, suffix)
                    };
                    ctx.app.add_log(LogEntry::info(msg));
                }
                Err(e) => {
                    ctx.app.add_log(LogEntry::error(format!(
                        "Failed to unapprove '{}': {}",
                        id, e
                    )));
                }
            }
        }

        TuiCommand::DeleteWorktreeByPath(path, branch_name) => {
            match crate::vcs::git::commands::worktree_remove(
                ctx.repo_root,
                path.to_string_lossy().as_ref(),
            )
            .await
            {
                Ok(_) => {
                    ctx.app.add_log(LogEntry::success(format!(
                        "Deleted worktree: {}",
                        path.display()
                    )));

                    // Delete the associated branch if it exists
                    if let Some(branch) = branch_name {
                        match crate::vcs::git::commands::branch_delete(ctx.repo_root, &branch).await
                        {
                            Ok(_) => {
                                ctx.app.add_log(LogEntry::success(format!(
                                    "Deleted branch: {}",
                                    branch
                                )));
                            }
                            Err(e) => {
                                warn!("Failed to delete branch '{}': {} (continuing)", branch, e);
                                ctx.app.add_log(LogEntry::warn(format!(
                                    "Failed to delete branch '{}': {}",
                                    branch, e
                                )));
                            }
                        }
                    }

                    // Refresh worktree list with conflict check
                    match load_worktrees_with_conflict_check(ctx.repo_root).await {
                        Ok(worktrees) => {
                            let _ = ctx
                                .tx
                                .send(OrchestratorEvent::WorktreesRefreshed { worktrees })
                                .await;
                        }
                        Err(e) => {
                            ctx.app.add_log(LogEntry::error(format!(
                                "Failed to refresh worktrees: {}",
                                e
                            )));
                        }
                    }
                }
                Err(e) => {
                    ctx.app.warning_popup = Some(crate::tui::state::WarningPopup {
                        title: "Worktree delete failed".to_string(),
                        message: format!("Failed to delete worktree '{}': {}", path.display(), e),
                    });
                    ctx.app.add_log(LogEntry::error(format!(
                        "Worktree delete failed for '{}': {}",
                        path.display(),
                        e
                    )));
                }
            }
        }
        TuiCommand::Stop => {
            // Initiate graceful stop
            if ctx.app.mode == AppMode::Running {
                ctx.app.stop_mode = StopMode::GracefulPending;
                graceful_stop_flag.store(true, Ordering::SeqCst);
                ctx.app.mode = AppMode::Stopping;
                ctx.app
                    .add_log(LogEntry::warn("Stopping after current change completes..."));
                // Emit Stopping event for web clients
                ctx.app
                    .handle_orchestrator_event(OrchestratorEvent::Stopping);
                // Forward to web state immediately for web control API
                #[cfg(feature = "web-monitoring")]
                if let Some(ref web_state) = ctx.web_state {
                    web_state
                        .apply_execution_event(&OrchestratorEvent::Stopping)
                        .await;
                }
            } else {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Cannot stop: not running (current mode: {:?})",
                    ctx.app.mode
                )));
            }
        }
        TuiCommand::CancelStop => {
            // Cancel graceful stop and return to Running mode
            if ctx.app.mode == AppMode::Stopping {
                graceful_stop_flag.store(false, Ordering::SeqCst);
                ctx.app.stop_mode = StopMode::None;
                ctx.app.mode = AppMode::Running;
                ctx.app
                    .add_log(LogEntry::info("Stop canceled, continuing..."));
                // Forward to web state immediately for web control API
                #[cfg(feature = "web-monitoring")]
                if let Some(ref web_state) = ctx.web_state {
                    // Use ProcessingStarted with empty string to transition to running mode
                    web_state
                        .apply_execution_event(&OrchestratorEvent::ProcessingStarted(
                            "".to_string(),
                        ))
                        .await;
                }
            } else {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Cannot cancel stop: not stopping (current mode: {:?})",
                    ctx.app.mode
                )));
            }
        }
        TuiCommand::ForceStop => {
            // Force stop immediately
            if matches!(ctx.app.mode, AppMode::Running | AppMode::Stopping) {
                ctx.app.stop_mode = StopMode::ForceStopped;
                if let Some(cancel) = orchestrator_cancel {
                    cancel.cancel();
                }
                ctx.app
                    .handle_orchestrator_event(OrchestratorEvent::Stopped);
                ctx.app.current_change = None;
                ctx.app.add_log(LogEntry::warn("Force stopped"));

                // Forward stopped event to web state
                #[cfg(feature = "web-monitoring")]
                if let Some(ref web_state) = ctx.web_state {
                    use crate::events::ExecutionEvent;
                    web_state
                        .apply_execution_event(&ExecutionEvent::Stopped)
                        .await;
                }
            } else {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Cannot force stop: not running or stopping (current mode: {:?})",
                    ctx.app.mode
                )));
            }
        }
        TuiCommand::Retry => {
            // Retry error changes (same as F5 in error mode)
            if ctx.app.mode == AppMode::Error {
                if let Some(TuiCommand::StartProcessing(ids)) = ctx.app.retry_error_changes() {
                    // Handle StartProcessing directly to avoid recursion
                    let handle = handle_start_processing_command(
                        ids,
                        ctx,
                        graceful_stop_flag,
                        shared_state,
                        manual_resolve_counter,
                        orchestrator_cancel,
                    )
                    .await;
                    return Ok(handle);
                }
            } else {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Cannot retry: not in error mode (current mode: {:?})",
                    ctx.app.mode
                )));
            }
        }
        TuiCommand::MergeWorktreeBranch {
            worktree_path,
            branch_name,
        } => {
            debug!(
                "Processing TuiCommand::MergeWorktreeBranch: worktree_path={}, branch_name={}",
                worktree_path.display(),
                branch_name
            );

            let merge_tx = ctx.tx.clone();
            let merge_repo_root = ctx.repo_root.to_path_buf();
            let merge_branch = branch_name.clone();
            let merge_config = ctx.config.clone();
            let merge_worktree_path = worktree_path.clone();

            tokio::spawn(async move {
                debug!(
                    "Sending BranchMergeStarted event for branch: {}",
                    merge_branch
                );
                let _ = merge_tx
                    .send(OrchestratorEvent::BranchMergeStarted {
                        branch_name: merge_branch.clone(),
                    })
                    .await;

                // FIX: Merge in base (main worktree), not in worktree directory
                // This ensures working directory clean check happens on base side
                debug!(
                    "Executing merge in base repository: repo_root={}, branch={}",
                    merge_repo_root.display(),
                    merge_branch
                );
                match crate::vcs::git::commands::merge_branch(&merge_repo_root, &merge_branch).await
                {
                    Ok(_) => {
                        debug!("Merge succeeded for branch: {}", merge_branch);
                        let _ = merge_tx
                            .send(OrchestratorEvent::BranchMergeCompleted {
                                branch_name: merge_branch.clone(),
                            })
                            .await;

                        // Run on_merged hook after manual branch merge
                        // Try to extract change_id from branch name; if it fails, log a warning
                        if let Some(change_id) =
                            crate::vcs::GitWorkspaceManager::extract_change_id_from_worktree_name(
                                &merge_branch,
                            )
                        {
                            // Create hook runner from config
                            let hooks_config = merge_config.get_hooks();
                            let hooks = crate::hooks::HookRunner::new(hooks_config);

                            // Fetch actual task counts from change data
                            let (completed_tasks, total_tasks) =
                                match crate::openspec::list_changes_native() {
                                    Ok(changes) => changes
                                        .iter()
                                        .find(|c| c.id == change_id)
                                        .map(|c| (c.completed_tasks, c.total_tasks))
                                        .unwrap_or((0, 0)),
                                    Err(e) => {
                                        warn!(
                                            "Failed to fetch task counts for on_merged hook: {}",
                                            e
                                        );
                                        (0, 0)
                                    }
                                };

                            let hook_context = crate::hooks::HookContext::new(
                                0, // changes_processed not available in manual merge
                                0, // total_changes not available
                                0, // remaining_changes not available
                                false,
                            )
                            .with_change(&change_id, completed_tasks, total_tasks)
                            .with_apply_count(0)
                            .with_parallel_context(&merge_worktree_path.to_string_lossy(), None);

                            if let Err(e) = hooks
                                .run_hook(crate::hooks::HookType::OnMerged, &hook_context)
                                .await
                            {
                                warn!("on_merged hook failed for {}: {}", change_id, e);
                            }
                        } else {
                            warn!(
                                "Could not extract change_id from branch name '{}', skipping on_merged hook",
                                merge_branch
                            );
                        }

                        // Refresh worktree list to update UI with conflict check
                        debug!("Refreshing worktree list after successful merge");
                        match load_worktrees_with_conflict_check(&merge_repo_root).await {
                            Ok(worktrees) => {
                                debug!("Worktree list refreshed: {} worktrees", worktrees.len());
                                let _ = merge_tx
                                    .send(OrchestratorEvent::WorktreesRefreshed { worktrees })
                                    .await;
                            }
                            Err(e) => {
                                debug!("Failed to refresh worktrees: {}", e);
                                let _ = merge_tx
                                    .send(OrchestratorEvent::Log(LogEntry::error(format!(
                                        "Failed to refresh worktrees: {}",
                                        e
                                    ))))
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Merge failed for branch {}: {}", merge_branch, e);
                        let _ = merge_tx
                            .send(OrchestratorEvent::BranchMergeFailed {
                                branch_name: merge_branch,
                                error: format!("{}", e),
                            })
                            .await;
                    }
                }
            });
        }
        TuiCommand::ResolveMerge(id) => {
            let resolve_tx = ctx.tx.clone();
            let resolve_repo_root = ctx.repo_root.to_path_buf();
            let resolve_config = ctx.config.clone();
            let resolve_counter = manual_resolve_counter.clone();
            tokio::spawn(async move {
                // Increment counter when resolve starts (consumes a parallel execution slot)
                resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                // Note: ResolveStarted event is sent by resolve_deferred_merge -> resolve_merges_with_retry
                // with the actual expanded command. We don't send it here to avoid duplicate events.

                // Helper closure to decrement counter and send event
                let finish_resolve = |event: OrchestratorEvent| async {
                    resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    let _ = resolve_tx.send(event).await;
                };

                match crate::parallel::base_dirty_reason(&resolve_repo_root).await {
                    Ok(Some(reason)) => {
                        finish_resolve(OrchestratorEvent::ResolveFailed {
                            change_id: id.clone(),
                            error: format!("Base is dirty: {}", reason),
                        })
                        .await;
                        return;
                    }
                    Err(e) => {
                        finish_resolve(OrchestratorEvent::ResolveFailed {
                            change_id: id.clone(),
                            error: format!("Failed to check base status: {}", e),
                        })
                        .await;
                        return;
                    }
                    Ok(None) => {}
                }

                // Transition ResolveWait -> Resolving once the resolve task actually begins.
                // We intentionally send a ResolveStarted event here so the TUI shows "resolving"
                // immediately, even when the merge completes without conflicts (no AI resolve command).
                let _ = resolve_tx
                    .send(OrchestratorEvent::ResolveStarted {
                        change_id: id.clone(),
                        command: format!("resolve_deferred_merge {}", id),
                    })
                    .await;

                match crate::parallel::resolve_deferred_merge(
                    resolve_repo_root.clone(),
                    resolve_config.clone(),
                    &id,
                )
                .await
                {
                    Ok(_) => {
                        let worktree_change_ids =
                            match crate::vcs::git::list_worktree_change_ids(&resolve_repo_root)
                                .await
                            {
                                Ok(ids) => Some(ids),
                                Err(err) => {
                                    let _ = resolve_tx
                                        .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                                            "Failed to refresh worktree snapshot: {}",
                                            err
                                        ))))
                                        .await;
                                    None
                                }
                            };
                        finish_resolve(OrchestratorEvent::ResolveCompleted {
                            change_id: id.clone(),
                            worktree_change_ids,
                        })
                        .await;
                    }
                    Err(e) => {
                        finish_resolve(OrchestratorEvent::ResolveFailed {
                            change_id: id.clone(),
                            error: e.to_string(),
                        })
                        .await;
                    }
                }
            });
        }
    }

    Ok(None)
}
