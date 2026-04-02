//! Orchestration logic for parallel execution with order-based re-analysis.
//!
//! This module handles the main scheduler loop that:
//! - Does NOT block on dispatch (spawn tasks into JoinSet)
//! - Continues re-analysis even when apply commands are running
//! - Tracks in-flight changes to calculate available slots
//! - Responds to queue notifications, debounce timers, and task completions

use crate::error::Result;
use crate::events::LogEntry;
use crate::merge_stall_monitor::MergeStallMonitor;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use super::cleanup::WorkspaceCleanupGuard;
use super::dynamic_queue::ReanalysisReason;
use super::events::send_event;
use super::types::WorkspaceResult;
use super::ParallelEvent;
use super::ParallelExecutor;

impl ParallelExecutor {
    /// Execute changes with order-based dependency analysis and concurrent re-analysis.
    ///
    /// This method uses a `tokio::select!` based scheduler loop that:
    /// - Does NOT block on dispatch (spawn tasks into JoinSet)
    /// - Continues re-analysis even when apply commands are running
    /// - Tracks in-flight changes to calculate available slots
    /// - Responds to queue notifications, debounce timers, and task completions
    ///
    /// # Arguments
    /// * `changes` - Initial list of changes to execute
    /// * `analyzer` - Async function that returns AnalysisResult (order + dependencies)
    ///   - First parameter: queued changes to analyze
    ///   - Second parameter: in-flight change IDs (currently executing)
    ///   - Third parameter: iteration number
    pub async fn execute_with_order_based_reanalysis<F>(
        &mut self,
        changes: Vec<crate::openspec::Change>,
        analyzer: F,
    ) -> Result<()>
    where
        for<'a> F: Fn(
                &'a [crate::openspec::Change],
                &'a [String],
                u32,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::analyzer::AnalysisResult> + Send + 'a>,
            > + Send
            + Sync,
    {
        if changes.is_empty() {
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
            return Ok(());
        }

        info!(
            "Starting order-based execution with re-analysis for {} changes",
            changes.len()
        );

        // Start merge stall monitor if enabled
        let merge_stall_monitor_handle = if let Some(cancel_token) = &self.cancel_token {
            let merge_stall_config = self.config.get_merge_stall_detection();
            if merge_stall_config.enabled {
                match self
                    .workspace_manager
                    .ensure_original_branch_initialized()
                    .await
                {
                    Ok(original_branch) => {
                        info!(
                            threshold_minutes = merge_stall_config.threshold_minutes,
                            check_interval_seconds = merge_stall_config.check_interval_seconds,
                            base_branch = %original_branch,
                            "Starting merge stall monitor for parallel execution"
                        );
                        let monitor = MergeStallMonitor::new(
                            merge_stall_config,
                            &self.repo_root,
                            original_branch.to_string(),
                        );
                        Some(monitor.spawn_monitor(cancel_token.clone()))
                    }
                    Err(e) => {
                        warn!("Cannot start merge stall monitor: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Prepare for parallel execution (clean check for git)
        info!("Preparing for parallel execution...");
        match self.workspace_manager.prepare_for_parallel().await {
            Ok(Some(warning)) => {
                warn!("{}", warning.message);
                send_event(
                    &self.event_tx,
                    ParallelEvent::Warning {
                        title: warning.title,
                        message: warning.message,
                    },
                )
                .await;
            }
            Ok(None) => {}
            Err(e) => {
                let error_msg = format!("Failed to prepare for parallel execution: {}", e);
                error!("{}", error_msg);
                send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                return Err(e.into());
            }
        }
        info!("Preparation complete");

        // Initialize scheduler state
        let max_parallelism = self.workspace_manager.max_concurrent();
        let semaphore = Arc::new(Semaphore::new(max_parallelism));
        let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
        let (merge_result_tx, mut merge_result_rx) = tokio::sync::mpsc::channel(64);
        let mut in_flight: HashSet<String> = HashSet::new();
        let mut queued: Vec<crate::openspec::Change> = changes;
        let mut iteration = 1u32;
        let mut cleanup_guard = WorkspaceCleanupGuard::new(
            self.workspace_manager.backend_type(),
            self.repo_root.clone(),
        );

        // Set needs_reanalysis to trigger first analysis
        self.needs_reanalysis = true;
        let mut reanalysis_reason = ReanalysisReason::Initial;
        let mut cancelled = false;

        // Main scheduler loop: wait for triggers and dispatch changes
        loop {
            // Check for cancellation
            if self.is_cancelled() {
                let remaining_changes: Vec<String> = queued.iter().map(|c| c.id.clone()).collect();
                let cancel_msg = format!(
                    "Cancelled parallel execution ({} queued, {} in-flight: queued=[{}], in-flight=[{}])",
                    remaining_changes.len(),
                    in_flight.len(),
                    remaining_changes.join(", "),
                    in_flight.iter().cloned().collect::<Vec<_>>().join(", ")
                );
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::warn(&cancel_msg)),
                )
                .await;
                cancelled = true;
                break;
            }

            // Step 1: Check dynamic queue for newly added changes (TUI mode)
            self.check_dynamic_queue_and_add_changes(
                &mut queued,
                &in_flight,
                &mut reanalysis_reason,
            )
            .await;

            // Step 2: Re-analysis if needed and debounce elapsed
            if self.needs_reanalysis
                && queued.is_empty()
                && in_flight.is_empty()
                && self.resolve_wait_changes.is_empty()
                && self.manual_resolve_active() == 0
                && self.pending_merge_count.load(Ordering::Relaxed) == 0
            {
                // All work completed.
                // Keep the scheduler alive while ResolveWait retries or manual resolve are active.
                info!(
                    "All changes completed (queued/in-flight/resolve_wait/manual_resolve empty), stopping"
                );
                break;
            }

            if self.needs_reanalysis && !queued.is_empty() {
                let (should_break, new_iteration) = self
                    .perform_reanalysis_and_dispatch(
                        &mut queued,
                        &mut in_flight,
                        max_parallelism,
                        iteration,
                        reanalysis_reason,
                        &analyzer,
                        semaphore.clone(),
                        &mut join_set,
                        &mut cleanup_guard,
                    )
                    .await?;

                iteration = new_iteration;

                if should_break {
                    break;
                }
            }

            // Step 3: Check if all work is done (before waiting on select)
            if join_set.is_empty()
                && queued.is_empty()
                && self.resolve_wait_changes.is_empty()
                && self.manual_resolve_active() == 0
                && self.pending_merge_count.load(Ordering::Relaxed) == 0
            {
                info!(
                    "All work completed (join_set/queued/resolve_wait/manual_resolve empty), exiting scheduler loop"
                );
                break;
            }

            // Step 4: Wait for events using tokio::select!
            // This makes the loop non-blocking and responsive to multiple triggers
            tokio::select! {
                // Join completion: task finished (apply+archive)
                Some(result) = join_set.join_next() => {
                    match result {
                        Ok(workspace_result) => {
                            self.handle_workspace_completion(workspace_result, max_parallelism, &mut in_flight, &merge_result_tx).await;

                            // Trigger re-analysis on next iteration.
                            // If a manual resolve is still active, keep the generic completion reason;
                            // otherwise treat the slot release as resolve-aware capacity recovery.
                            self.needs_reanalysis = true;
                            let manual_resolves_active = self
                                .manual_resolve_count
                                .as_ref()
                                .map(|counter| counter.load(std::sync::atomic::Ordering::Relaxed))
                                .unwrap_or(0);
                            reanalysis_reason = if manual_resolves_active == 0 {
                                ReanalysisReason::ResolveCompletion
                            } else {
                                ReanalysisReason::Completion
                            };
                        }
                        Err(e) => {
                            error!("Task panicked: {:?}", e);
                        }
                    }
                }

                // Background merge completion: merge+cleanup finished asynchronously
                Some(merge_result) = merge_result_rx.recv() => {
                    self.handle_merge_result(merge_result).await;
                    reanalysis_reason = ReanalysisReason::ResolveCompletion;
                }

                // Queue notification: dynamic queue has new items
                Some(_) = async {
                    if let Some(queue) = &self.dynamic_queue {
                        queue.notified().await;
                        Some(())
                    } else {
                        std::future::pending().await
                    }
                } => {
                    info!("Queue notification received, will check queue on next iteration");
                    // Queue check happens at loop start
                }

                // Debounce timer: wait before allowing re-analysis
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    // Timer expired, loop will re-check needs_reanalysis and debounce
                }
            }
        }

        // Clean up merge stall monitor
        if let Some(handle) = merge_stall_monitor_handle {
            handle.abort();
        }

        // Drop cleanup guard without calling commit()
        // Workspaces are preserved by default for resume/debugging
        // Cleanup is only performed explicitly after successful merge via cleanup_workspace()
        drop(cleanup_guard);

        // Send appropriate completion event based on how we exited
        if cancelled {
            send_event(&self.event_tx, ParallelEvent::Stopped).await;
        } else {
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
        }
        Ok(())
    }
}
