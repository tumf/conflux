//! Orchestrator execution logic for the TUI
//!
//! Contains the run_orchestrator function and archive operations.

use crate::agent::AgentRunner;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::events::{LogEntry, OrchestratorEvent};
use super::queue::DynamicQueue;

/// Context for archive operations
pub struct ArchiveContext {
    pub changes_processed: usize,
    pub total_changes: usize,
    pub remaining_changes: usize,
    pub apply_count: u32,
}

/// Result of archive operation
pub enum ArchiveResult {
    Success,
    Failed,
    Cancelled,
}

/// Archive a single completed change
/// Returns Ok(ArchiveResult) indicating success, failure, or cancellation
pub async fn archive_single_change(
    change_id: &str,
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &crate::hooks::HookRunner,
    tx: &mpsc::Sender<OrchestratorEvent>,
    cancel_token: &CancellationToken,
    context: &ArchiveContext,
) -> Result<ArchiveResult> {
    use crate::agent::OutputLine;
    use crate::hooks::{HookContext, HookType};

    // Run on_change_complete hook
    let complete_context = HookContext::new(
        context.changes_processed,
        context.total_changes,
        context.remaining_changes,
        false,
    )
    .with_change(change_id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);
    if let Err(e) = hooks
        .run_hook(HookType::OnChangeComplete, &complete_context)
        .await
    {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_change_complete hook failed: {}",
                e
            ))))
            .await;
    }

    // Run pre_archive hook
    let pre_archive_context = HookContext::new(
        context.changes_processed,
        context.total_changes,
        context.remaining_changes,
        false,
    )
    .with_change(change_id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);
    if let Err(e) = hooks
        .run_hook(HookType::PreArchive, &pre_archive_context)
        .await
    {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "pre_archive hook failed: {}",
                e
            ))))
            .await;
    }

    // Archive the change
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(format!(
            "Archiving: {}",
            change_id
        ))))
        .await;

    // Run archive command with streaming output
    let (mut child, mut output_rx) = agent.run_archive_streaming(change_id).await?;

    // Stream output to TUI log, with cancellation support
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                let _ = child.kill().await;
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::warn(
                        "Process killed due to cancellation".to_string(),
                    )))
                    .await;
                return Ok(ArchiveResult::Cancelled);
            }
            line = output_rx.recv() => {
                match line {
                    Some(OutputLine::Stdout(s)) => {
                        let _ = tx.send(OrchestratorEvent::Log(LogEntry::info(s))).await;
                    }
                    Some(OutputLine::Stderr(s)) => {
                        let _ = tx.send(OrchestratorEvent::Log(LogEntry::warn(s))).await;
                    }
                    None => break,
                }
            }
        }
    }

    // Wait for child process to complete
    let status = child.wait().await.map_err(|e| {
        crate::error::OrchestratorError::AgentCommand(format!("Failed to wait for process: {}", e))
    })?;

    if status.success() {
        // Clear apply history for the archived change
        agent.clear_apply_history(change_id);

        // Run post_archive hook
        let post_archive_context = HookContext::new(
            context.changes_processed + 1,
            context.total_changes,
            context.remaining_changes.saturating_sub(1),
            false,
        )
        .with_change(change_id, change.completed_tasks, change.total_tasks)
        .with_apply_count(context.apply_count);
        if let Err(e) = hooks
            .run_hook(HookType::PostArchive, &post_archive_context)
            .await
        {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "post_archive hook failed: {}",
                    e
                ))))
                .await;
        }

        let _ = tx
            .send(OrchestratorEvent::ChangeArchived(change_id.to_string()))
            .await;
        Ok(ArchiveResult::Success)
    } else {
        let error_msg = format!("Archive failed with exit code: {:?}", status.code());

        // Run on_error hook
        let error_context = HookContext::new(
            context.changes_processed,
            context.total_changes,
            context.remaining_changes,
            false,
        )
        .with_change(change_id, change.completed_tasks, change.total_tasks)
        .with_apply_count(context.apply_count)
        .with_error(&error_msg);
        let _ = hooks.run_hook(HookType::OnError, &error_context).await;

        let _ = tx
            .send(OrchestratorEvent::ProcessingError {
                id: change_id.to_string(),
                error: error_msg.clone(),
            })
            .await;
        Ok(ArchiveResult::Failed)
    }
}

/// Archive all complete changes from the pending set
/// Returns the number of successfully archived changes
#[allow(clippy::too_many_arguments)]
pub async fn archive_all_complete_changes(
    pending_ids: &HashSet<String>,
    _openspec_cmd: &str, // Kept for API compatibility, native impl doesn't need it
    agent: &mut AgentRunner,
    hooks: &crate::hooks::HookRunner,
    tx: &mpsc::Sender<OrchestratorEvent>,
    cancel_token: &CancellationToken,
    archived_set: &mut HashSet<String>,
    total_changes: usize,
    changes_processed: &mut usize,
    apply_counts: &HashMap<String, u32>,
) -> Result<usize> {
    use crate::openspec;

    // Fetch current state of all changes using native implementation
    let changes = openspec::list_changes_native()?;

    // Find complete changes that are still in pending set
    let complete_changes: Vec<Change> = changes
        .into_iter()
        .filter(|c| pending_ids.contains(&c.id) && !archived_set.contains(&c.id) && c.is_complete())
        .collect();

    let mut archived_count = 0;

    for change in complete_changes {
        if cancel_token.is_cancelled() {
            break;
        }

        let remaining_changes = pending_ids.len().saturating_sub(archived_count);
        let apply_count = *apply_counts.get(&change.id).unwrap_or(&0);
        let context = ArchiveContext {
            changes_processed: *changes_processed,
            total_changes,
            remaining_changes,
            apply_count,
        };

        // Notify processing started for this change
        let _ = tx
            .send(OrchestratorEvent::ProcessingStarted(change.id.clone()))
            .await;

        // Send ProcessingCompleted before archiving
        let _ = tx
            .send(OrchestratorEvent::ProcessingCompleted(change.id.clone()))
            .await;

        match archive_single_change(
            &change.id,
            &change,
            agent,
            hooks,
            tx,
            cancel_token,
            &context,
        )
        .await?
        {
            ArchiveResult::Success => {
                archived_set.insert(change.id.clone());
                archived_count += 1;
                *changes_processed += 1;
            }
            ArchiveResult::Failed => {
                // Error already logged and sent, continue to next
            }
            ArchiveResult::Cancelled => {
                break;
            }
        }
    }

    Ok(archived_count)
}

/// Run the orchestrator for selected changes
/// Uses streaming output to send log entries in real-time
/// Supports cancellation via CancellationToken for graceful shutdown
///
/// The orchestrator uses a two-phase loop:
/// - Phase 1: Archive all complete changes before doing any apply
/// - Phase 2: Apply one incomplete change
///
/// This ensures complete changes are never skipped.
pub async fn run_orchestrator(
    change_ids: Vec<String>,
    openspec_cmd: String,
    config: OrchestratorConfig,
    tx: mpsc::Sender<OrchestratorEvent>,
    cancel_token: CancellationToken,
    dynamic_queue: DynamicQueue,
    graceful_stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    use crate::agent::OutputLine;
    use crate::hooks::{HookContext, HookRunner, HookType};
    use crate::openspec;

    let hooks = HookRunner::new(config.get_hooks());
    let max_iterations = config.get_max_iterations();
    let mut agent = AgentRunner::new(config);

    let mut total_changes = change_ids.len();
    let mut iteration: u32 = 0;
    let mut changes_processed: usize = 0;
    let mut current_change_id: Option<String> = None;
    let mut apply_counts: HashMap<String, u32> = HashMap::new();
    let mut archived_changes: HashSet<String> = HashSet::new();
    let mut pending_changes: HashSet<String> = change_ids.iter().cloned().collect();
    let mut processed_change_ids: Vec<String> = change_ids.clone();

    // Run on_start hook
    let start_context = HookContext::new(0, total_changes, total_changes, false);
    if let Err(e) = hooks.run_hook(HookType::OnStart, &start_context).await {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_start hook failed: {}",
                e
            ))))
            .await;
    }

    // Main two-phase loop
    loop {
        // Check for cancellation before each iteration
        if cancel_token.is_cancelled() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Check for graceful stop flag (stop after current change completes)
        if graceful_stop_flag.load(Ordering::SeqCst) {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::info(
                    "Graceful stop: stopping after current change".to_string(),
                )))
                .await;
            let _ = tx.send(OrchestratorEvent::Stopped).await;
            break;
        }

        // Check max iterations limit (0 = no limit)
        if max_iterations > 0 && iteration >= max_iterations {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Max iterations ({}) reached, stopping orchestration",
                    max_iterations
                ))))
                .await;
            // Send completion event
            let _ = tx.send(OrchestratorEvent::AllCompleted).await;
            break;
        }

        // Log warning when approaching limit (80%)
        if max_iterations > 0 {
            let warning_threshold = (max_iterations as f32 * 0.8) as u32;
            if iteration == warning_threshold {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                        "Approaching max iterations: {}/{}",
                        iteration, max_iterations
                    ))))
                    .await;
            }
        }

        // Check dynamic queue for new changes before checking if we're done
        while let Some(dynamic_id) = dynamic_queue.pop().await {
            // Skip if already archived or in pending
            if !archived_changes.contains(&dynamic_id) && !pending_changes.contains(&dynamic_id) {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Processing dynamically added: {}",
                        dynamic_id
                    ))))
                    .await;
                pending_changes.insert(dynamic_id.clone());
                processed_change_ids.push(dynamic_id);
                total_changes += 1;
            }
        }

        // Check if all pending changes are done
        if pending_changes.is_empty() {
            break;
        }

        // Phase 1: Archive all complete changes
        let archived_count = archive_all_complete_changes(
            &pending_changes,
            &openspec_cmd,
            &mut agent,
            &hooks,
            &tx,
            &cancel_token,
            &mut archived_changes,
            total_changes,
            &mut changes_processed,
            &apply_counts,
        )
        .await?;

        // Remove archived changes from pending
        for id in &archived_changes {
            pending_changes.remove(id);
        }

        if archived_count > 0 {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                    "Archived {} complete change(s)",
                    archived_count
                ))))
                .await;
        }

        // Check if all done after archiving
        // Dynamic queue is checked at the start of the next iteration
        if pending_changes.is_empty() {
            continue; // Re-check dynamic queue
        }

        // Check for cancellation after archive phase
        if cancel_token.is_cancelled() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Phase 2: Select and apply next incomplete change
        // Fetch current state to find best candidate using native implementation
        let changes = openspec::list_changes_native()?;

        // Find the next incomplete change from our pending set
        // Prioritize by highest progress percentage
        let next_change = changes
            .iter()
            .filter(|c| pending_changes.contains(&c.id) && !c.is_complete())
            .max_by(|a, b| {
                let a_progress = if a.total_tasks > 0 {
                    a.completed_tasks as f32 / a.total_tasks as f32
                } else {
                    0.0
                };
                let b_progress = if b.total_tasks > 0 {
                    b.completed_tasks as f32 / b.total_tasks as f32
                } else {
                    0.0
                };
                a_progress
                    .partial_cmp(&b_progress)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let Some(change) = next_change else {
            // No incomplete changes found - might all be complete now
            // Loop will re-check in Phase 1
            continue;
        };

        let change_id = change.id.clone();
        let change = change.clone();
        iteration += 1;

        // Notify processing started
        let _ = tx
            .send(OrchestratorEvent::ProcessingStarted(change_id.clone()))
            .await;

        let remaining_changes = pending_changes.len();

        // Check if this is a new change (for on_change_start hook)
        let is_new_change = current_change_id.as_ref() != Some(&change_id);
        if is_new_change {
            // Run on_change_start hook
            let change_start_context =
                HookContext::new(changes_processed, total_changes, remaining_changes, false)
                    .with_change(&change_id, change.completed_tasks, change.total_tasks)
                    .with_apply_count(0);
            if let Err(e) = hooks
                .run_hook(HookType::OnChangeStart, &change_start_context)
                .await
            {
                let _ = tx
                    .send(OrchestratorEvent::ProcessingError {
                        id: change_id.clone(),
                        error: format!("on_change_start hook failed: {}", e),
                    })
                    .await;
                break;
            }
            current_change_id = Some(change_id.clone());
        }

        // Get current apply count for this change and increment it
        let apply_count = *apply_counts.get(&change_id).unwrap_or(&0) + 1;
        apply_counts.insert(change_id.clone(), apply_count);

        // Run pre_apply hook
        let pre_apply_context =
            HookContext::new(changes_processed, total_changes, remaining_changes, false)
                .with_change(&change_id, change.completed_tasks, change.total_tasks)
                .with_apply_count(apply_count);
        if let Err(e) = hooks.run_hook(HookType::PreApply, &pre_apply_context).await {
            let _ = tx
                .send(OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: format!("pre_apply hook failed: {}", e),
                })
                .await;
            break;
        }

        // Apply the change
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::info(format!(
                "Applying: {}",
                change_id
            ))))
            .await;

        // Run apply command with streaming output
        let (mut child, mut output_rx, start_time) = agent.run_apply_streaming(&change_id).await?;

        // Stream output to TUI log, with cancellation support
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    let _ = child.kill().await;
                    let _ = tx
                        .send(OrchestratorEvent::Log(LogEntry::warn(
                            "Process killed due to cancellation".to_string(),
                        )))
                        .await;
                    // Exit the main loop
                    pending_changes.clear();
                    break;
                }
                line = output_rx.recv() => {
                    match line {
                        Some(OutputLine::Stdout(s)) => {
                            let _ = tx.send(OrchestratorEvent::Log(LogEntry::info(s))).await;
                        }
                        Some(OutputLine::Stderr(s)) => {
                            let _ = tx.send(OrchestratorEvent::Log(LogEntry::warn(s))).await;
                        }
                        None => break,
                    }
                }
            }
        }

        // Check if we were cancelled during streaming
        if cancel_token.is_cancelled() {
            break;
        }

        // Wait for child process to complete
        let status = child.wait().await.map_err(|e| {
            crate::error::OrchestratorError::AgentCommand(format!(
                "Failed to wait for process: {}",
                e
            ))
        })?;

        // Record the apply attempt for history context in subsequent retries
        agent.record_apply_attempt(&change_id, &status, start_time);

        if status.success() {
            // Run post_apply hook
            let post_apply_context =
                HookContext::new(changes_processed, total_changes, remaining_changes, false)
                    .with_change(&change_id, change.completed_tasks, change.total_tasks)
                    .with_apply_count(apply_count);
            if let Err(e) = hooks
                .run_hook(HookType::PostApply, &post_apply_context)
                .await
            {
                let _ = tx
                    .send(OrchestratorEvent::ProcessingError {
                        id: change_id.clone(),
                        error: format!("post_apply hook failed: {}", e),
                    })
                    .await;
                break;
            }

            // Apply succeeded - check if tasks are now 100% complete
            // Re-fetch change to get updated task counts after apply
            let updated_changes = crate::openspec::list_changes_native().unwrap_or_default();
            let is_complete = updated_changes
                .iter()
                .find(|c| c.id == change_id)
                .is_some_and(|c| c.is_complete());

            if is_complete {
                // Only send ProcessingCompleted when tasks are 100% done
                let _ = tx
                    .send(OrchestratorEvent::ProcessingCompleted(change_id.clone()))
                    .await;
            }

            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                    "Apply completed for {}, checking for completion...",
                    change_id
                ))))
                .await;
        } else {
            let error_msg = format!("Apply failed with exit code: {:?}", status.code());

            // Run on_error hook
            let error_context =
                HookContext::new(changes_processed, total_changes, remaining_changes, false)
                    .with_change(&change_id, change.completed_tasks, change.total_tasks)
                    .with_apply_count(apply_count)
                    .with_error(&error_msg);
            let _ = hooks.run_hook(HookType::OnError, &error_context).await;

            let _ = tx
                .send(OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error_msg,
                })
                .await;

            // Remove failed change from pending to prevent infinite retry
            pending_changes.remove(&change_id);
        }
    }

    // Final verification: check if any changes remain unarchived
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(
            "Verifying all changes have been archived...".to_string(),
        )))
        .await;

    // Check against our tracked archived set for reliable verification
    let unarchived_by_tracking: Vec<&str> = processed_change_ids
        .iter()
        .filter(|id| !archived_changes.contains(*id))
        .map(|id| id.as_str())
        .collect();

    // Also verify against native list as backup
    let final_changes = openspec::list_changes_native().ok();
    if let Some(changes) = final_changes {
        let unarchived_by_list: Vec<&str> = processed_change_ids
            .iter()
            .filter(|id| changes.iter().any(|c| &c.id == *id))
            .map(|id| id.as_str())
            .collect();

        // Report unarchived changes (use tracking as primary, list as confirmation)
        if !unarchived_by_tracking.is_empty() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Warning: {} change(s) were not archived (tracking): {}",
                    unarchived_by_tracking.len(),
                    unarchived_by_tracking.join(", ")
                ))))
                .await;
        }
        if !unarchived_by_list.is_empty() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Warning: {} change(s) remain in openspec list: {}",
                    unarchived_by_list.len(),
                    unarchived_by_list.join(", ")
                ))))
                .await;
        }
        if unarchived_by_tracking.is_empty() && unarchived_by_list.is_empty() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::success(
                    "All processed changes have been archived".to_string(),
                )))
                .await;
        }
    } else if !unarchived_by_tracking.is_empty() {
        // Could not fetch final list, but tracking shows unarchived changes
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "Warning: {} change(s) were not archived (tracking): {}",
                unarchived_by_tracking.len(),
                unarchived_by_tracking.join(", ")
            ))))
            .await;
    }

    let _ = tx.send(OrchestratorEvent::AllCompleted).await;
    Ok(())
}

/// Run the orchestrator in parallel mode using jj workspaces
/// This function executes all changes in parallel using ParallelExecutor
pub async fn run_orchestrator_parallel(
    change_ids: Vec<String>,
    _openspec_cmd: String,
    config: OrchestratorConfig,
    tx: mpsc::Sender<OrchestratorEvent>,
    cancel_token: CancellationToken,
) -> Result<()> {
    use crate::openspec::list_changes_native;
    use crate::parallel_executor::{ParallelEvent, ParallelExecutor};
    use std::collections::HashSet;

    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(format!(
            "Starting parallel processing of {} change(s)",
            change_ids.len()
        ))))
        .await;

    // Changes remain in Queued state until ApplyStarted event marks them as Processing

    // Get repo root
    let repo_root = std::env::current_dir()?;

    // Create event channel for ParallelExecutor
    let (parallel_tx, mut parallel_rx) = mpsc::channel::<ParallelEvent>(100);

    // Create ParallelExecutor
    let mut executor = ParallelExecutor::new(repo_root, config.clone(), Some(parallel_tx));

    // Check jj availability
    if !executor.check_jj_available().await? {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::error(
                "jj is not available for parallel execution".to_string(),
            )))
            .await;
        return Err(crate::error::OrchestratorError::AgentCommand(
            "jj not available".to_string(),
        ));
    }

    // Load changes with dependencies and filter to selected ones
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(
            "Loading changes with dependencies...".to_string(),
        )))
        .await;

    let all_changes = list_changes_native()?;
    let selected_set: HashSet<_> = change_ids.iter().collect();
    let selected_changes: Vec<_> = all_changes
        .into_iter()
        .filter(|c| selected_set.contains(&c.id))
        .collect();

    // Use deterministic dependency-based grouping (no LLM)
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(format!(
            "Grouping {} changes by dependencies...",
            selected_changes.len()
        ))))
        .await;

    let groups = group_by_dependencies(&selected_changes);
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(format!(
            "Created {} groups based on dependencies",
            groups.len()
        ))))
        .await;

    // Spawn event forwarding task
    let forward_tx = tx.clone();
    let forward_cancel = cancel_token.clone();
    let forward_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = forward_cancel.cancelled() => {
                    break;
                }
                event = parallel_rx.recv() => {
                    match event {
                        Some(ParallelEvent::WorkspaceCreated { change_id, workspace }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(
                                    LogEntry::info(format!("Created workspace: {}", workspace))
                                        .with_change_id(&change_id),
                                ))
                                .await;
                        }
                        Some(ParallelEvent::ApplyStarted { change_id }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(
                                    LogEntry::info("Apply started".to_string())
                                        .with_change_id(&change_id),
                                ))
                                .await;
                            // Mark as Processing when apply actually starts
                            let _ = forward_tx
                                .send(OrchestratorEvent::ProcessingStarted(change_id))
                                .await;
                        }
                        Some(ParallelEvent::ApplyOutput { change_id, output }) => {
                            // Split output into multiple log entries for each line
                            for line in output.lines() {
                                if !line.trim().is_empty() {
                                    let _ = forward_tx
                                        .send(OrchestratorEvent::Log(
                                            LogEntry::info(line.to_string())
                                                .with_change_id(&change_id),
                                        ))
                                        .await;
                                }
                            }
                        }
                        Some(ParallelEvent::ApplyCompleted { change_id, .. }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(
                                    LogEntry::success("Apply completed".to_string())
                                        .with_change_id(&change_id),
                                ))
                                .await;
                            let _ = forward_tx
                                .send(OrchestratorEvent::ProcessingCompleted(change_id))
                                .await;
                        }
                        Some(ParallelEvent::ApplyFailed { change_id, error }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(
                                    LogEntry::error(format!("Apply failed: {}", error))
                                        .with_change_id(&change_id),
                                ))
                                .await;
                            let _ = forward_tx
                                .send(OrchestratorEvent::ProcessingError {
                                    id: change_id,
                                    error,
                                })
                                .await;
                        }
                        Some(ParallelEvent::MergeStarted { revisions }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                                    "Merging {} revisions",
                                    revisions.len()
                                ))))
                                .await;
                        }
                        Some(ParallelEvent::MergeCompleted { .. }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(LogEntry::success(
                                    "Merge completed".to_string(),
                                )))
                                .await;
                        }
                        Some(ParallelEvent::MergeConflict { files }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                                    "Merge conflicts in {} files",
                                    files.len()
                                ))))
                                .await;
                        }
                        Some(ParallelEvent::CleanupStarted { workspace }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                                    "Cleaning up workspace: {}",
                                    workspace
                                ))))
                                .await;
                        }
                        Some(ParallelEvent::CleanupCompleted { .. }) => {
                            // Silent cleanup completion
                        }
                        Some(ParallelEvent::GroupStarted { group_id, changes }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                                    "Starting group {} with {} change(s): {}",
                                    group_id,
                                    changes.len(),
                                    changes.join(", ")
                                ))))
                                .await;
                        }
                        Some(ParallelEvent::GroupCompleted { group_id }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(LogEntry::success(format!(
                                    "Group {} completed",
                                    group_id
                                ))))
                                .await;
                        }
                        Some(ParallelEvent::ChangeArchived { change_id }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(
                                    LogEntry::success("Archived".to_string())
                                        .with_change_id(&change_id),
                                ))
                                .await;
                            let _ = forward_tx
                                .send(OrchestratorEvent::ChangeArchived(change_id))
                                .await;
                        }
                        Some(ParallelEvent::ArchiveFailed { change_id, error }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(
                                    LogEntry::error(format!("Archive failed: {}", error))
                                        .with_change_id(&change_id),
                                ))
                                .await;
                        }
                        Some(ParallelEvent::AllCompleted) => {
                            break;
                        }
                        Some(ParallelEvent::Error { message }) => {
                            let _ = forward_tx
                                .send(OrchestratorEvent::Log(LogEntry::error(message)))
                                .await;
                        }
                        Some(_) => {
                            // Other events - ignore
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Execute groups sequentially (groups are already ordered by dependencies)
    let result = tokio::select! {
        _ = cancel_token.cancelled() => {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Parallel execution cancelled".to_string(),
                )))
                .await;
            Err(crate::error::OrchestratorError::AgentCommand("Cancelled".to_string()))
        }
        result = executor.execute_groups(groups) => {
            result
        }
    };

    // Wait for forward task to complete
    let _ = forward_handle.await;

    match result {
        Ok(_) => {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::success(
                    "All parallel changes completed, archiving...".to_string(),
                )))
                .await;

            // Actually archive completed changes
            let agent = crate::agent::AgentRunner::new(config);
            for id in &change_ids {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Archiving: {}",
                        id
                    ))))
                    .await;

                // Run archive command
                match agent.run_archive(id).await {
                    Ok(_) => {
                        let _ = tx.send(OrchestratorEvent::ChangeArchived(id.clone())).await;
                        let _ = tx
                            .send(OrchestratorEvent::Log(LogEntry::success(format!(
                                "Archived: {}",
                                id
                            ))))
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(OrchestratorEvent::Log(LogEntry::error(format!(
                                "Failed to archive {}: {}",
                                id, e
                            ))))
                            .await;
                    }
                }
            }
        }
        Err(e) => {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::error(format!(
                    "Parallel execution failed: {}",
                    e
                ))))
                .await;
        }
    }

    let _ = tx.send(OrchestratorEvent::AllCompleted).await;
    Ok(())
}

/// Group changes by their declared dependencies (deterministic, no LLM)
///
/// Returns groups in topological order where:
/// - Group 1: Changes with no dependencies
/// - Group 2: Changes that depend only on Group 1 changes
/// - And so on...
fn group_by_dependencies(changes: &[Change]) -> Vec<crate::analyzer::ParallelGroup> {
    use std::collections::{HashMap, HashSet};

    if changes.is_empty() {
        return Vec::new();
    }

    // Build lookup maps with owned strings to avoid lifetime issues
    let change_ids: HashSet<String> = changes.iter().map(|c| c.id.clone()).collect();
    let mut remaining: HashSet<String> = change_ids.clone();
    let mut completed_changes: HashSet<String> = HashSet::new();

    // Map from change_id to its dependencies (filtered to only include changes in our set)
    let deps_map: HashMap<String, Vec<String>> = changes
        .iter()
        .map(|c| {
            let deps: Vec<String> = c
                .dependencies
                .iter()
                .filter(|d| change_ids.contains(*d))
                .cloned()
                .collect();
            (c.id.clone(), deps)
        })
        .collect();

    let mut groups: Vec<crate::analyzer::ParallelGroup> = Vec::new();
    let mut group_id = 1u32;

    // Iteratively find changes whose dependencies are all complete
    while !remaining.is_empty() {
        let mut current_group: Vec<String> = Vec::new();

        for change_id in &remaining {
            let deps = deps_map.get(change_id).map(|d| d.as_slice()).unwrap_or(&[]);
            // A change can be in this group if all its dependencies are completed
            if deps.iter().all(|d| completed_changes.contains(d)) {
                current_group.push(change_id.clone());
            }
        }

        if current_group.is_empty() {
            // Circular dependency or missing dependency - add remaining changes to last group
            tracing::warn!(
                "Unable to resolve dependencies for: {:?}",
                remaining.iter().collect::<Vec<_>>()
            );
            current_group = remaining.iter().cloned().collect();
        }

        // Calculate depends_on (previous group if any)
        let depends_on = if group_id > 1 {
            vec![group_id - 1]
        } else {
            Vec::new()
        };

        // Remove completed changes from remaining
        for change_id in &current_group {
            remaining.remove(change_id);
            completed_changes.insert(change_id.clone());
        }

        // Sort for deterministic output
        current_group.sort();

        groups.push(crate::analyzer::ParallelGroup {
            id: group_id,
            changes: current_group,
            depends_on,
        });

        group_id += 1;
    }

    groups
}
