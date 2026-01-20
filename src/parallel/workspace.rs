//! Workspace creation and management for parallel execution.
//!
//! This module handles:
//! - Workspace creation and reuse (resumption logic)
//! - Workspace status tracking
//! - Workspace lifecycle management (create, resume, cleanup)

use crate::agent::AgentRunner;
use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::events::LogEntry;
use crate::history::{ApplyHistory, ArchiveHistory};
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::events::send_event;
use crate::parallel::executor::{
    execute_acceptance_in_workspace, execute_apply_in_workspace, execute_archive_in_workspace,
};
use crate::parallel::types::WorkspaceResult;
use crate::parallel::ParallelEvent;
use crate::vcs::{Workspace, WorkspaceManager, WorkspaceStatus};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Get or create a workspace for a change.
///
/// This function handles workspace creation/resumption logic:
/// - Checks for existing workspaces if no_resume is false
/// - Creates new workspaces when needed
/// - Sends appropriate events for workspace creation/resumption
#[allow(dead_code)] // Will be used when refactoring is complete
pub async fn get_or_create_workspace(
    workspace_manager: &mut dyn WorkspaceManager,
    change_id: &str,
    base_revision: &str,
    no_resume: bool,
    force_recreate_worktree: &HashSet<String>,
    event_tx: &Option<mpsc::Sender<ParallelEvent>>,
) -> Result<Workspace> {
    // Check for existing workspace (resume scenario)
    if !no_resume && !force_recreate_worktree.contains(change_id) {
        if let Ok(Some(workspace_info)) = workspace_manager.find_existing_workspace(change_id).await
        {
            info!(
                "Resuming existing workspace for '{}' (last modified: {:?})",
                change_id, workspace_info.last_modified
            );
            if let Ok(ws) = workspace_manager.reuse_workspace(&workspace_info).await {
                send_event(
                    event_tx,
                    ParallelEvent::WorkspaceResumed {
                        change_id: change_id.to_string(),
                        workspace: ws.name.clone(),
                    },
                )
                .await;
                return Ok(ws);
            }
        }
    }

    // Create new workspace
    let ws = workspace_manager
        .create_workspace(change_id, Some(base_revision))
        .await?;

    send_event(
        event_tx,
        ParallelEvent::WorkspaceCreated {
            change_id: change_id.to_string(),
            workspace: ws.name.clone(),
        },
    )
    .await;

    Ok(ws)
}

/// Dispatch a change to a workspace and spawn apply + archive task.
///
/// This function:
/// 1. Acquires a semaphore permit (enforces concurrency limit)
/// 2. Creates or reuses a workspace
/// 3. Spawns an async task for apply + acceptance + archive
/// 4. Returns immediately (non-blocking)
#[allow(dead_code)] // Will be used when refactoring is complete
#[allow(clippy::too_many_arguments)]
pub async fn dispatch_change_to_workspace(
    workspace_manager: &mut dyn WorkspaceManager,
    change_id: String,
    base_revision: String,
    semaphore: Arc<Semaphore>,
    join_set: &mut JoinSet<WorkspaceResult>,
    in_flight: &mut HashSet<String>,
    cleanup_guard: &mut WorkspaceCleanupGuard,
    no_resume: bool,
    force_recreate_worktree: &HashSet<String>,
    event_tx: &Option<mpsc::Sender<ParallelEvent>>,
    apply_command: &str,
    archive_command: &str,
    repo_root: &Path,
    config: &OrchestratorConfig,
    ai_runner: &AiCommandRunner,
    apply_history: &Arc<Mutex<ApplyHistory>>,
    archive_history: &Arc<Mutex<ArchiveHistory>>,
    cancel_token: &Option<CancellationToken>,
) -> Result<()> {
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

    // Create or reuse workspace
    let workspace = get_or_create_workspace(
        workspace_manager,
        &change_id,
        &base_revision,
        no_resume,
        force_recreate_worktree,
        event_tx,
    )
    .await?;

    // Track workspace for cleanup
    cleanup_guard.track(workspace.name.clone(), workspace.path.clone());

    // Add to in-flight set
    in_flight.insert(change_id.clone());

    // Prepare context for spawned task
    let apply_command = apply_command.to_string();
    let archive_command = archive_command.to_string();
    let repo_root = repo_root.to_path_buf();
    let config = config.clone();
    let event_tx = event_tx.clone();
    let vcs_backend = workspace_manager.backend_type();
    let ai_runner = ai_runner.clone();
    let apply_history = apply_history.clone();
    let archive_history = archive_history.clone();
    let cancel_token = cancel_token.clone();

    // Spawn apply + acceptance + archive task
    join_set.spawn(async move {
        let _permit = permit; // Hold permit until task completes

        // Create agent for acceptance testing
        let mut agent = AgentRunner::new(config.clone());

        // Track apply+acceptance cycles to prevent infinite loops
        const MAX_APPLY_ACCEPTANCE_CYCLES: u32 = 10;
        let mut cycle_count = 0u32;
        let mut cumulative_iteration = 0u32; // Track total apply iterations across all cycles

        // Apply+Acceptance loop: retry apply when acceptance fails
        let _apply_revision = loop {
            cycle_count += 1;
            if cycle_count > MAX_APPLY_ACCEPTANCE_CYCLES {
                error!(
                    "Max apply+acceptance cycles ({}) reached for {}",
                    MAX_APPLY_ACCEPTANCE_CYCLES, change_id
                );
                return WorkspaceResult {
                    change_id,
                    workspace_name: workspace.name,
                    final_revision: None,
                    error: Some(format!(
                        "Max apply+acceptance cycles ({}) reached",
                        MAX_APPLY_ACCEPTANCE_CYCLES
                    )),
                };
            }

            // Step 1: Execute apply with cumulative iteration count
            let apply_result = execute_apply_in_workspace(
                &change_id,
                &workspace.path,
                &apply_command,
                &config,
                event_tx.clone(),
                vcs_backend,
                None, // hooks
                None, // parallel_ctx
                cancel_token.as_ref(),
                &ai_runner,
                &repo_root,
                &apply_history,
                cumulative_iteration, // Pass current iteration count
            )
            .await;

            let (revision, final_iteration) = match apply_result {
                Ok((rev, iter)) => (rev, iter),
                Err(e) => {
                    // Apply failed - return error immediately
                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: Some(format!("Apply failed: {}", e)),
                    };
                }
            };

            // Update cumulative iteration count
            cumulative_iteration = final_iteration;

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
                cancel_token.as_ref(),
            )
            .await;

            // Get the acceptance iteration number for logging (count after recording)
            let acceptance_iteration = agent.next_acceptance_attempt_number(&change_id);

            match acceptance_result {
                Ok(crate::orchestration::AcceptanceResult::Pass) => {
                    info!("Acceptance passed for {}, proceeding to archive", change_id);
                    // Break out of loop, proceed to archive
                    break revision;
                }
                Ok(crate::orchestration::AcceptanceResult::Continue) => {
                    let continue_count = agent.count_consecutive_acceptance_continues(&change_id);
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
                Ok(crate::orchestration::AcceptanceResult::Fail { findings }) => {
                    warn!(
                        "Acceptance failed for {} with {} findings (cycle {}), returning to apply loop",
                        change_id,
                        findings.len(),
                        cycle_count
                    );
                    // Update tasks.md with acceptance findings
                    if let Err(e) = crate::orchestration::update_tasks_on_acceptance_failure(
                        &change_id,
                        &findings,
                        Some(&workspace.path),
                    )
                    .await
                    {
                        warn!("Failed to update tasks.md for {}: {}", change_id, e);
                    }
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::warn(format!(
                                    "Acceptance failed with {} findings, returning to apply loop (cycle {})",
                                    findings.len(),
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
                Ok(crate::orchestration::AcceptanceResult::CommandFailed { error, findings }) => {
                    error!(
                        "Acceptance command failed for {} (cycle {}): {}",
                        change_id, cycle_count, error
                    );
                    // Update tasks.md with command failure
                    if let Err(e) = crate::orchestration::update_tasks_on_acceptance_failure(
                        &change_id,
                        &findings,
                        Some(&workspace.path),
                    )
                    .await
                    {
                        warn!("Failed to update tasks.md for {}: {}", change_id, e);
                    }
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
                    };
                }
                Ok(crate::orchestration::AcceptanceResult::Cancelled) => {
                    info!("Acceptance cancelled for {}", change_id);
                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: Some("Acceptance cancelled".to_string()),
                    };
                }
                Err(e) => {
                    error!("Acceptance error for {}: {}", change_id, e);
                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: Some(format!("Acceptance error: {}", e)),
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
            let _ = tx
                .send(ParallelEvent::ArchiveStarted(change_id.clone()))
                .await;
        }

        let archive_result = execute_archive_in_workspace(
            &change_id,
            &workspace.path,
            &archive_command,
            &config,
            event_tx.clone(),
            vcs_backend,
            None, // hooks
            None, // parallel_ctx
            cancel_token.as_ref(),
            &ai_runner,
            &archive_history,
            &apply_history,
        )
        .await;

        match archive_result {
            Ok(archive_revision) => {
                // Clear acceptance history after successful archive
                agent.clear_acceptance_history(&change_id);

                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ChangeArchived(change_id.clone()))
                        .await;
                }
                WorkspaceResult {
                    change_id,
                    workspace_name: workspace.name,
                    final_revision: Some(archive_revision),
                    error: None,
                }
            }
            Err(e) => {
                warn!("Archive failed for {}: {}", change_id, e);
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ArchiveFailed {
                            change_id: change_id.clone(),
                            error: e.to_string(),
                        })
                        .await;
                }
                // Archive failed - do not merge unarchived changes
                WorkspaceResult {
                    change_id,
                    workspace_name: workspace.name,
                    final_revision: None,
                    error: Some(format!("Archive failed: {}", e)),
                }
            }
        }
        // _permit is dropped here, releasing semaphore
    });

    Ok(())
}
