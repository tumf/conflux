//! TuiCommand handlers for TUI
//!
//! This module contains helper functions to handle TuiCommand processing.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::orchestration::state::{ReduceOutcome, ReducerCommand};
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};
use crate::tui::orchestrator::{run_orchestrator, run_orchestrator_parallel};
use crate::tui::queue::DynamicQueue;
use crate::tui::state::AppState;
use crate::tui::types::{AppMode, StopMode};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

#[cfg(test)]
#[derive(Clone, Debug)]
enum DeleteWorktreeTestOutcome {
    Success,
    Failure(String),
}

#[cfg(test)]
static DELETE_WORKTREE_TEST_OUTCOMES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, DeleteWorktreeTestOutcome>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[cfg(test)]
fn set_delete_worktree_test_outcome(path: PathBuf, outcome: DeleteWorktreeTestOutcome) {
    DELETE_WORKTREE_TEST_OUTCOMES
        .lock()
        .expect("delete worktree test outcomes lock")
        .insert(path, outcome);
}

#[cfg(test)]
async fn remove_worktree_for_tui(
    _repo_root: &Path,
    path: &Path,
    _skip_teardown: bool,
) -> crate::error::Result<()> {
    let outcome = DELETE_WORKTREE_TEST_OUTCOMES
        .lock()
        .expect("delete worktree test outcomes lock")
        .remove(path)
        .unwrap_or(DeleteWorktreeTestOutcome::Success);

    match outcome {
        DeleteWorktreeTestOutcome::Success => Ok(()),
        DeleteWorktreeTestOutcome::Failure(message) => {
            Err(crate::error::OrchestratorError::GitCommand(format!(
                "stubbed delete failure for {}: {}",
                path.display(),
                message
            )))
        }
    }
}

#[cfg(not(test))]
async fn remove_worktree_for_tui(
    repo_root: &Path,
    path: &Path,
    skip_teardown: bool,
) -> crate::error::Result<()> {
    crate::vcs::git::commands::worktree_remove_with_options(
        repo_root,
        path.to_string_lossy().as_ref(),
        crate::vcs::git::commands::WorktreeRemoveOptions { skip_teardown },
    )
    .await
    .map_err(crate::error::OrchestratorError::from)
}

use super::worktrees::load_worktrees_with_conflict_check;

/// Context for TuiCommand handling
pub struct TuiCommandContext<'a> {
    pub app: &'a mut AppState,
    pub repo_root: &'a Path,
    pub config: &'a OrchestratorConfig,
    pub tx: &'a mpsc::Sender<OrchestratorEvent>,
    pub dynamic_queue: &'a DynamicQueue,
    pub remote_client: Option<crate::remote::RemoteClient>,
    pub orchestrator_running: bool,
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
            // Remote server mode: trigger server-side run instead of local orchestrator.
            if let Some(remote) = ctx.remote_client.as_ref() {
                // Group selected changes by project_id (encoded as "<project_id>::...").
                let mut by_project: std::collections::BTreeMap<String, Vec<String>> =
                    std::collections::BTreeMap::new();
                for id in &selected_ids {
                    let Some((project_id, rest)) = id.split_once("::") else {
                        // Unknown format; skip.
                        continue;
                    };
                    // rest looks like "<project_name>/<change_id>".
                    let change_id = rest.rsplit('/').next().unwrap_or(rest).to_string();
                    by_project
                        .entry(project_id.to_string())
                        .or_default()
                        .push(change_id);
                }

                for (project_id, change_ids) in by_project {
                    if let Err(e) = remote.control_run(&project_id, Some(change_ids)).await {
                        ctx.app.add_log(LogEntry::error(format!(
                            "Remote run failed for {}: {}",
                            project_id, e
                        )));
                    } else {
                        ctx.app.add_log(LogEntry::success(format!(
                            "Remote run started: {}",
                            project_id
                        )));
                    }
                }

                // No local orchestrator task.
                return None;
            }

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
            // Apply reducer command first, then push to dynamic queue.
            let outcome = {
                let mut guard = shared_state.write().await;
                if guard.is_terminal_error_change(&id) {
                    guard.apply_command(ReducerCommand::RetryError(id.clone()))
                } else {
                    guard.apply_command(ReducerCommand::AddToQueue(id.clone()))
                }
            };
            if matches!(outcome, crate::orchestration::state::ReduceOutcome::NoOp) {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Queue add ignored by reducer: {}",
                    id
                )));
                return Ok(None);
            }
            ctx.app.apply_display_statuses_from_reducer(
                &shared_state.read().await.all_display_statuses(),
            );
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
            // Apply reducer command first, then remove from dynamic queue.
            shared_state
                .write()
                .await
                .apply_command(ReducerCommand::RemoveFromQueue(id.clone()));
            ctx.app.apply_display_statuses_from_reducer(
                &shared_state.read().await.all_display_statuses(),
            );
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
        TuiCommand::DeleteWorktreeByPath(path, branch_name, skip_teardown) => {
            let delete_result = remove_worktree_for_tui(ctx.repo_root, &path, skip_teardown).await;
            ctx.app.clear_worktree_deleting(&path);

            match delete_result {
                Ok(_) => {
                    info!("Worktree deleted successfully: {}", path.display());
                    ctx.app.add_log(LogEntry::success(format!(
                        "Deleted worktree: {}",
                        path.display()
                    )));

                    // Delete the associated branch if it exists
                    if let Some(branch) = branch_name {
                        match crate::vcs::git::commands::branch_delete(ctx.repo_root, &branch).await
                        {
                            Ok(_) => {
                                info!("Branch deleted after worktree removal: {}", branch);
                                ctx.app.add_log(LogEntry::success(format!(
                                    "Deleted branch: {}",
                                    branch
                                )));
                            }
                            Err(e) => {
                                warn!(
                                    "Branch deletion failed for '{}' after worktree removal: {} (non-fatal)",
                                    branch, e
                                );
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
                    ctx.app.show_warning_popup(
                        "Worktree delete failed",
                        format!("Failed to delete worktree '{}': {}", path.display(), e),
                    );
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

                        // Run on_merged hook before BranchMergeCompleted event (before merged status transition)
                        // Try to extract change_id from branch name; if it fails, log a warning
                        if let Some(change_id) =
                            crate::vcs::GitWorkspaceManager::extract_change_id_from_worktree_name(
                                &merge_branch,
                            )
                        {
                            // Create hook runner from config
                            let hooks_config = merge_config.get_hooks();
                            let merge_repo_root = std::env::current_dir()
                                .unwrap_or_else(|_| std::path::PathBuf::from("."));
                            let hooks = crate::hooks::HookRunner::with_event_tx(
                                hooks_config,
                                merge_repo_root,
                                merge_tx.clone(),
                            );

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
                                let message = format!(
                                    "on_merged hook failed for '{}'; branch merged transition blocked: {}",
                                    change_id, e
                                );
                                warn!("{}", message);
                                let _ = merge_tx
                                    .send(OrchestratorEvent::HookFailed {
                                        change_id: change_id.clone(),
                                        hook_type: crate::hooks::HookType::OnMerged.to_string(),
                                        error: e.to_string(),
                                    })
                                    .await;
                                let _ = merge_tx
                                    .send(OrchestratorEvent::BranchMergeFailed {
                                        branch_name: merge_branch.clone(),
                                        error: message,
                                    })
                                    .await;
                                return;
                            }
                        } else {
                            warn!(
                                "Could not extract change_id from branch name '{}', skipping on_merged hook",
                                merge_branch
                            );
                        }

                        // Send BranchMergeCompleted after on_merged hook (triggers merged status transition)
                        let _ = merge_tx
                            .send(OrchestratorEvent::BranchMergeCompleted {
                                branch_name: merge_branch.clone(),
                            })
                            .await;

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
        TuiCommand::DequeueChange(id) => {
            // Apply reducer command first so shared state reflects stop-and-dequeue intent.
            shared_state
                .write()
                .await
                .apply_command(ReducerCommand::DequeueChange(id.clone()));
            // Force-kill the in-flight execution for this change.
            // force_kill() marks stopped AND immediately cancels the per-change token,
            // bypassing the 100ms polling delay for truly immediate process termination.
            let had_token = ctx.dynamic_queue.force_kill(&id).await;
            if had_token {
                ctx.app.add_log(LogEntry::info(format!(
                    "Force-kill issued for active change: {}",
                    id
                )));
            } else {
                ctx.app.add_log(LogEntry::info(format!(
                    "Stop-and-dequeue request received for: {}",
                    id
                )));
            }
        }
        TuiCommand::ResolveMerge(id) => {
            // Apply reducer command first so shared state reflects scheduler-visible resolve intent.
            // A NoOp means reducer-owned retry intent was not accepted, so do not notify or
            // start the scheduler as if display-only pending were real work.
            let reduce_outcome = shared_state
                .write()
                .await
                .apply_command(ReducerCommand::ResolveMerge(id.clone()));
            if matches!(reduce_outcome, ReduceOutcome::NoOp) {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Manual merge-wait retry intent for '{}' was not accepted by scheduler state",
                    id
                )));
                return Ok(None);
            }

            if ctx.orchestrator_running {
                // Scheduler-owned execution model when scheduler is already alive:
                // wake scheduler so normal loop can pick up retry intent and queued work together.
                ctx.dynamic_queue.notify_scheduler();
                ctx.app.add_log(LogEntry::info(format!(
                    "Scheduled merge-wait retry intent for '{}'; notified existing scheduler",
                    id
                )));
            } else {
                // Scheduler not alive: spawn a scheduler-owned run to consume reducer-owned ResolveWait.
                graceful_stop_flag.store(false, Ordering::SeqCst);
                let orch_tx = ctx.tx.clone();
                let orch_config = ctx.config.clone();
                let orch_cancel = CancellationToken::new();
                let orch_dynamic_queue = ctx.dynamic_queue.clone();
                let orch_graceful_stop = graceful_stop_flag.clone();
                let orch_shared_state = shared_state.clone();
                let orch_manual_resolve = manual_resolve_counter.clone();
                *orchestrator_cancel = Some(orch_cancel.clone());

                #[cfg(feature = "web-monitoring")]
                let orch_web_state = ctx.web_state.clone();

                let handle = tokio::spawn(async move {
                    #[cfg(feature = "web-monitoring")]
                    let result = run_orchestrator_parallel(
                        Vec::new(),
                        orch_config,
                        orch_tx,
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_manual_resolve,
                        orch_web_state,
                    )
                    .await;

                    #[cfg(not(feature = "web-monitoring"))]
                    let result = run_orchestrator_parallel(
                        Vec::new(),
                        orch_config,
                        orch_tx,
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_manual_resolve,
                    )
                    .await;

                    result
                });

                ctx.app.add_log(LogEntry::info(format!(
                    "Scheduled merge-wait retry intent for '{}'; started scheduler for manual resolve",
                    id
                )));
                return Ok(Some(handle));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::orchestration::state::OrchestratorState;
    use crate::tui::types::WorktreeInfo;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use tokio::sync::RwLock;

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    fn create_test_config() -> OrchestratorConfig {
        OrchestratorConfig::default()
    }

    fn create_test_worktree(path: &str) -> WorktreeInfo {
        WorktreeInfo {
            path: PathBuf::from(path),
            head: "abc123".to_string(),
            branch: "feature-a".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        }
    }

    fn create_command_context<'a>(
        app: &'a mut AppState,
        tx: &'a mpsc::Sender<OrchestratorEvent>,
        dynamic_queue: &'a DynamicQueue,
        config: &'a OrchestratorConfig,
    ) -> TuiCommandContext<'a> {
        TuiCommandContext {
            app,
            repo_root: Path::new("."),
            config,
            tx,
            dynamic_queue,
            remote_client: None,
            orchestrator_running: false,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        }
    }

    #[tokio::test]
    async fn test_delete_worktree_command_clears_marker_on_success() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(vec![], 10)));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let path = PathBuf::from("/tmp/worktree-success");
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        let mut app = AppState::new(vec![]);

        app.worktrees = vec![create_test_worktree("/tmp/worktree-success")];
        app.mark_worktree_deleting(path.clone());

        let mut ctx = create_command_context(&mut app, &tx, &dynamic_queue, &config);
        let handle = handle_tui_command(
            TuiCommand::DeleteWorktreeByPath(path.clone(), None, false),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("delete command should succeed");

        assert!(handle.is_none());
        assert!(!ctx.app.is_worktree_deleting(&path));
        assert!(ctx.app.logs.iter().any(|entry| entry
            .message
            .contains("Deleted worktree: /tmp/worktree-success")));
    }

    #[tokio::test]
    async fn test_delete_worktree_command_clears_marker_on_failure() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(vec![], 10)));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let path = PathBuf::from("/tmp/worktree-failure");
        set_delete_worktree_test_outcome(
            path.clone(),
            DeleteWorktreeTestOutcome::Failure("boom".to_string()),
        );
        let mut app = AppState::new(vec![]);
        app.worktrees = vec![create_test_worktree("/tmp/worktree-failure")];
        app.mark_worktree_deleting(path.clone());

        let mut ctx = create_command_context(&mut app, &tx, &dynamic_queue, &config);
        let handle = handle_tui_command(
            TuiCommand::DeleteWorktreeByPath(path.clone(), None, false),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("delete command should handle failures as UI errors");

        assert!(handle.is_none());
        assert!(!ctx.app.is_worktree_deleting(&path));
        assert!(ctx.app.warning_popup.is_some());
        assert!(ctx
            .app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Worktree delete failed")));
    }

    #[tokio::test]
    async fn test_resolve_merge_starts_parallel_scheduler_when_idle() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::with_mode(
            vec!["change-a".to_string()],
            10,
            crate::orchestration::state::ExecutionMode::Parallel,
        )));
        {
            let mut guard = shared_state.write().await;
            guard.apply_observation(
                "change-a",
                crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
            );
        }
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            orchestrator_running: false,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("resolve merge command should succeed");

        assert!(
            handle.is_some(),
            "idle scheduler must spawn a new orchestrator task"
        );
        assert!(
            orchestrator_cancel.is_some(),
            "spawned scheduler must install cancellation token"
        );
        assert!(
            ctx.app.logs.iter().any(|entry| entry
                .message
                .contains("started scheduler for manual resolve")),
            "log must report scheduler startup"
        );
        {
            let state = shared_state.read().await;
            assert_eq!(
                state.display_status("change-a"),
                "resolve pending",
                "ResolveMerge reducer intent must move change to resolve pending"
            );
            assert_eq!(
                state.resolve_wait_change_ids(),
                vec!["change-a".to_string()],
                "idle handoff must leave reducer-owned ResolveWait visible to scheduler startup"
            );
        }

        if let Some(join) = handle {
            join.abort();
        }
    }

    #[tokio::test]
    async fn test_resolve_merge_notifies_live_scheduler_without_duplicate_spawn() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("resolve merge command should succeed");

        assert!(
            handle.is_none(),
            "live scheduler path must not spawn a duplicate orchestrator task"
        );
        assert!(
            orchestrator_cancel.is_none(),
            "live scheduler notification path must not replace cancel token"
        );
        assert!(
            ctx.app
                .logs
                .iter()
                .any(|entry| entry.message.contains("notified existing scheduler")),
            "log must report scheduler notification"
        );
    }

    #[tokio::test]
    async fn test_resolve_merge_logs_scheduler_start_or_notify_truthfully() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));

        let mut app_idle = AppState::new(vec![create_test_change("change-a")]);
        let mut cancel_idle: Option<CancellationToken> = None;
        let mut idle_ctx = TuiCommandContext {
            app: &mut app_idle,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            orchestrator_running: false,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let idle_handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut idle_ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut cancel_idle,
        )
        .await
        .expect("idle resolve merge should succeed");

        assert!(idle_ctx.app.logs.iter().any(|entry| entry
            .message
            .contains("started scheduler for manual resolve")));

        if let Some(join) = idle_handle {
            join.abort();
        }

        let mut app_live = AppState::new(vec![create_test_change("change-a")]);
        let mut cancel_live: Option<CancellationToken> = None;
        let mut live_ctx = TuiCommandContext {
            app: &mut app_live,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let live_handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut live_ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut cancel_live,
        )
        .await
        .expect("live resolve merge should succeed");

        assert!(live_handle.is_none());
        assert!(live_ctx
            .app
            .logs
            .iter()
            .any(|entry| entry.message.contains("notified existing scheduler")));
    }

    #[tokio::test]
    async fn test_resolve_merge_noop_does_not_notify_or_log_scheduled() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::with_mode(
            vec!["change-a".to_string()],
            10,
            crate::orchestration::state::ExecutionMode::Parallel,
        )));
        {
            let mut guard = shared_state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::MergeCompleted {
                change_id: "change-a".to_string(),
                revision: "rev-a".to_string(),
            });
        }
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("resolve merge command should not fail");

        assert!(handle.is_none());
        assert!(orchestrator_cancel.is_none());
        assert!(ctx.app.logs.iter().any(|entry| entry
            .message
            .contains("was not accepted by scheduler state")));
        assert!(!ctx
            .app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Scheduled merge-wait retry intent")));
        assert!(shared_state
            .read()
            .await
            .resolve_wait_change_ids()
            .is_empty());
    }

    #[tokio::test]
    async fn test_add_to_queue_updates_reducer_intent_even_if_dynamic_queue_already_contains_id() {
        use crate::orchestration::state::ReducerCommand;

        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;

        // Pre-populate dynamic queue so AddToQueue push path returns false (already present).
        dynamic_queue.push("change-a".to_string()).await;

        {
            // Clear reducer intent first to ensure command handler is the source of re-queue intent.
            let mut guard = shared_state.write().await;
            guard.apply_command(ReducerCommand::RemoveFromQueue("change-a".to_string()));
            assert_eq!(guard.display_status("change-a"), "not queued");
        }

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::AddToQueue("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("add-to-queue command should succeed");

        assert!(
            handle.is_none(),
            "queue command should not spawn orchestrator"
        );
        assert_eq!(
            shared_state.read().await.display_status("change-a"),
            "queued",
            "reducer queue intent must be queued even when dynamic queue push is duplicate"
        );
        assert!(ctx
            .app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Already in dynamic queue: change-a")));
    }

    #[tokio::test]
    async fn remove_from_queue_updates_reducer_snapshot_and_dynamic_queue() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        dynamic_queue.push("change-a".to_string()).await;
        shared_state
            .write()
            .await
            .apply_command(ReducerCommand::AddToQueue("change-a".to_string()));
        app.apply_display_statuses_from_reducer(&shared_state.read().await.all_display_statuses());
        assert!(app.changes[0].selected);
        assert_eq!(app.changes[0].display_status_cache, "queued");

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::RemoveFromQueue("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("remove-from-queue command should succeed");

        assert!(handle.is_none());
        assert_eq!(
            shared_state.read().await.display_status("change-a"),
            "not queued"
        );
        assert_eq!(ctx.app.changes[0].display_status_cache, "not queued");
        assert!(!dynamic_queue.contains("change-a").await);
        assert_eq!(
            dynamic_queue.drain_removed().await,
            vec!["change-a".to_string()]
        );
    }

    #[tokio::test]
    async fn test_resolve_merge_scheduler_liveness_none_finished_live() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));

        // none/finished = idle path => scheduler spawn
        for running in [false, false] {
            let mut app = AppState::new(vec![create_test_change("change-a")]);
            let mut orchestrator_cancel: Option<CancellationToken> = None;
            let mut ctx = TuiCommandContext {
                app: &mut app,
                repo_root: Path::new("."),
                config: &config,
                tx: &tx,
                dynamic_queue: &dynamic_queue,
                remote_client: None,
                orchestrator_running: running,
                #[cfg(feature = "web-monitoring")]
                web_state: &None,
            };

            let handle = handle_tui_command(
                TuiCommand::ResolveMerge("change-a".to_string()),
                &mut ctx,
                &graceful_stop_flag,
                &shared_state,
                &manual_resolve_counter,
                &mut orchestrator_cancel,
            )
            .await
            .expect("resolve merge should succeed");

            assert!(
                handle.is_some(),
                "non-live scheduler state must spawn scheduler"
            );
            if let Some(join) = handle {
                join.abort();
            }
        }

        // live = notification-only path => no spawn
        let mut app_live = AppState::new(vec![create_test_change("change-a")]);
        let mut orchestrator_cancel_live: Option<CancellationToken> = None;
        let mut ctx_live = TuiCommandContext {
            app: &mut app_live,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle_live = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut ctx_live,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel_live,
        )
        .await
        .expect("resolve merge should succeed");

        assert!(
            handle_live.is_none(),
            "live scheduler state must not spawn scheduler"
        );
    }
}
