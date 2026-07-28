//! Orchestration logic for parallel execution with order-based re-analysis.
//!
//! This module handles the main scheduler loop that:
//! - Does NOT block on dispatch (spawn tasks into JoinSet)
//! - Continues re-analysis even when apply commands are running
//! - Tracks in-flight changes to calculate available slots
//! - Responds to queue notifications, debounce timers, and task completions
//!
//! # Re-analysis trigger lifetime
//!
//! `ReanalysisReason::ResolveCompletion`, `SlotRecovery`, and `RepairCandidate` bypass
//! queue debounce, so they are treated as one-shot state-transition edges: the loop
//! consumes them back to the non-bypass `Initial` state once a queued re-analysis /
//! dispatch evaluation has actually used them (see
//! [`ParallelExecutor::evaluate_queued_reanalysis_and_dispatch`]). Keeping such a reason
//! across iterations would let the plain 500 ms timer branch replay an already-handled
//! edge and restart expensive dependency analysis on every scheduler tick.
//!
//! # Unchanged ordinary-timer input
//!
//! Consuming edge triggers fixes replay of an *explicit* edge, but it does not stop the
//! plain `Initial` timer branch from re-analyzing forever. The queue-coalescing debounce
//! only measures how long ago the queue changed, so once ten seconds have elapsed every
//! later 500 ms wake passes it — it is neither a cooldown nor an unchanged-input check.
//!
//! The second layer therefore compares the *input*: immediately before invoking the
//! analyzer, an ordinary timer evaluation builds a deterministic
//! [`crate::parallel::analysis_signature::AnalysisInputSignature`] over the queued analysis
//! fields, the queued and in-flight proposal content the prompt references, in-flight
//! membership, dispatch capacity, and the effective dependency-base revision. If that input
//! already completed with a usable result in this process, the analyzer is skipped and a
//! deduplicated `unchanged_analysis_input` reason is emitted. Cheap queue classification,
//! reconciliation, and blocker checks still run first, so nothing repository-visible is
//! cached — only duplicate expensive analysis is removed.
//!
//! Both layers are required: they solve different replay mechanisms, and neither subsumes
//! the other. Coverage lives in `src/parallel/tests/unchanged_analysis_input.rs`.
//!
//! # Bounded fail-open retry
//!
//! Neither layer can record anything when a pass produces no completed input: a failed
//! proposal read or effective-base revision lookup yields no signature, and an analyzer that
//! returns no usable order proves nothing was analyzed. Both stay fail-open — analysis is
//! permitted and the loop keeps running — but a persistent failure would then probe and
//! relaunch the analyzer on every 500 ms wake, which is the replay loop suppression exists to
//! remove.
//!
//! The third layer is therefore a
//! [`crate::parallel::analysis_signature::BoundedAnalysisRetry`] deadline, held separately
//! from the completed-input record so unavailable evidence is never mistaken for completed
//! analysis. It rate-limits the ordinary timer retry to the existing ten-second cadence;
//! explicit edges still bypass it once per event, and a later successful probe with a usable
//! result clears it. An unusable result also never ends the loop while queued work remains:
//! termination stays owned by the canonical drain checks. Loop-level coverage lives in
//! `src/parallel/tests/analysis_liveness_loop.rs`.
//!
//! # Capacity-recovery audit
//!
//! Because the sticky reason is gone, every path that releases scheduler-accounted
//! occupancy must reach queued work through a real wake edge, slot-recovery detection,
//! or the bounded timer/debounce evaluation:
//!
//! - Workspace task completion: `join_set.join_next()` in [`Self::wait_for_scheduler_event`]
//!   removes the change from `in_flight` via `handle_workspace_completion` and sets
//!   `ResolveCompletion` (or `Completion` while a manual resolve is still active).
//! - Manual resolve completion: the TUI decrements the shared manual-resolve counter and
//!   calls `DynamicQueue::notify_scheduler`, which wakes the queue-notification branch.
//! - Automatic conflict/merge resolution: `conflict::AutoResolveGuard` decrements
//!   `auto_resolve_count` on success, failure, and early return, and the owning
//!   background merge task always reports a `MergeResult` afterwards.
//! - Background merge results (merged / deferred / failed): `handle_merge_result_with_tx`
//!   decrements `pending_merge_count` for every outcome; merged additionally arms a
//!   `ResolveCompletion` edge and promotes deferred base-lane waiters.
//! - Deferred resolve/reject wait retries: `maybe_dispatch_resolve_wait_retry_with_tx`
//!   spawns through the same merge-result channel, so its completion is a wake edge too.
//! - Zero-to-positive slot transitions observed by `calculate_available_slots` /
//!   `last_available_slots` are promoted to `SlotRecovery`.
//! - Independently of any reason, changed capacity or in-flight membership changes the
//!   analysis-input signature, so the bounded timer evaluation re-analyzes even when a
//!   slot-recovery notification is lost.
//!
//! Coverage lives in `src/parallel/tests/reanalysis_trigger_lifetime.rs` (edge lifetime,
//! merge-outcome capacity release), `src/parallel/tests/conflict.rs` (auto-resolve guard
//! release on success / failure / early return), and
//! `src/parallel/tests/auto_resolve.rs` (deferred retry convergence).

use crate::error::Result;
use crate::events::LogEntry;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use super::cleanup::WorkspaceCleanupGuard;
use super::dynamic_queue::ReanalysisReason;
use super::events::send_event;
use super::queue_state::ReanalysisDispatchContext;
use super::types::WorkspaceResult;
use super::ParallelEvent;
use super::ParallelExecutor;
use super::SchedulerLifetime;

impl ParallelExecutor {
    pub(super) fn is_fully_drained(
        &self,
        join_set_empty: bool,
        queued_empty: bool,
        in_flight_empty: bool,
    ) -> bool {
        join_set_empty
            && queued_empty
            && in_flight_empty
            && self.resolve_wait_changes.is_empty()
            && self.reject_wait_changes.is_empty()
            && self.manual_resolve_active() == 0
            && self.pending_merge_count.load(Ordering::Relaxed) == 0
    }

    pub(super) async fn should_exit_when_idle(
        &self,
        join_set_empty: bool,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
    ) -> bool {
        if self.scheduler_lifetime != SchedulerLifetime::Finite || !join_set_empty {
            return false;
        }
        self.is_fully_drained(join_set_empty, queued.is_empty(), in_flight.is_empty())
            || self
                .is_blocked_only_scheduler_state(queued, in_flight)
                .await
    }

    pub(super) async fn should_enter_persistent_idle_wait(
        &self,
        join_set_empty: bool,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
    ) -> bool {
        if self.scheduler_lifetime != SchedulerLifetime::Persistent || !join_set_empty {
            return false;
        }
        self.is_fully_drained(join_set_empty, queued.is_empty(), in_flight.is_empty())
            || (queued.is_empty()
                && in_flight.is_empty()
                && (!self.resolve_wait_changes.is_empty() || !self.reject_wait_changes.is_empty())
                && self.manual_resolve_active() == 0
                && self.pending_merge_count.load(Ordering::Relaxed) == 0)
            || self
                .is_blocked_only_scheduler_state(queued, in_flight)
                .await
    }

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
    /// * `analyzer` - Async function that returns an `AnalysisOutcome` (order + dependencies
    ///   plus runtime-only provenance describing whether the result came from a healthy
    ///   analyzer run, intentional metadata-only analysis, or a recoverable-failure fallback)
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
                Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
            > + Send
            + Sync,
    {
        if changes.is_empty() {
            let (reducer_has_queued_intent, reducer_has_lane_wait) = self
                .shared_orchestrator_state
                .as_ref()
                .and_then(|state| state.try_read().ok())
                .map(|state| {
                    (
                        !state.queued_change_ids().is_empty(),
                        !state.resolve_wait_change_ids().is_empty()
                            || !state.reject_wait_change_ids().is_empty(),
                    )
                })
                .unwrap_or((false, false));
            if !reducer_has_queued_intent && !reducer_has_lane_wait {
                send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
                return Ok(());
            }
            if reducer_has_lane_wait {
                info!(
                    "Starting scheduler loop with reducer-visible base-lane wait retry intent and empty local queue"
                );
            } else {
                info!(
                    "Starting scheduler loop with reducer-visible queued intent and empty local queue"
                );
            }
        }

        info!(
            "Starting order-based execution with re-analysis for {} changes",
            changes.len()
        );

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

        // Reanalysis reason is derived from scheduler events/state each iteration.
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
                join_set.abort_all();
                while let Some(result) = join_set.join_next().await {
                    if let Err(err) = result {
                        if !err.is_cancelled() {
                            warn!(error = %err, "In-flight workspace task failed while draining after cancellation");
                        }
                    }
                }
                in_flight.clear();
                break;
            }

            // Step 1: Check dynamic queue for newly added changes (TUI mode)
            let dynamic_queue_added = self
                .check_dynamic_queue_and_add_changes(
                    &mut queued,
                    &in_flight,
                    &mut reanalysis_reason,
                )
                .await;

            // Step 2: Sync reducer-owned ResolveWait intent before scheduler drain/idle checks.
            // This keeps manual resolve dispatch reducer-owned while making scheduler work detection truthful.
            self.sync_resolve_wait_from_shared_state_nonblocking();
            self.maybe_dispatch_resolve_wait_retry_with_tx(&merge_result_tx)
                .await;

            // Step 2: Reconcile reducer-visible queue intent into scheduler-local candidates.
            let reconciliation = self
                .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
                .await;
            if reconciliation.has_queued_additions() {
                reanalysis_reason = ReanalysisReason::QueueNotification;
            } else if reconciliation.has_repair_additions() {
                reanalysis_reason = ReanalysisReason::RepairCandidate;
            } else if matches!(reanalysis_reason, ReanalysisReason::QueueNotification)
                && !dynamic_queue_added
            {
                // A scheduler wake without scheduler-visible queue additions remains
                // debounceable.  `QueueNotification` only carries operator-intent
                // priority into analysis when the current loop ingested new queued work.
                reanalysis_reason = ReanalysisReason::Initial;
            }

            // Step 3: Re-analysis decision is derived from scheduler state.
            let work_drained = queued.is_empty()
                && in_flight.is_empty()
                && self.resolve_wait_changes.is_empty()
                && self.reject_wait_changes.is_empty()
                && self.manual_resolve_active() == 0
                && self.pending_merge_count.load(Ordering::Relaxed) == 0;
            if work_drained && self.scheduler_lifetime == SchedulerLifetime::Finite {
                info!(
                    "All changes completed (queued/in-flight/resolve_wait/manual_resolve empty), stopping"
                );
                break;
            }
            if let Some((should_break, new_iteration)) = self
                .evaluate_queued_reanalysis_and_dispatch(
                    ReanalysisDispatchContext {
                        queued: &mut queued,
                        in_flight: &mut in_flight,
                        max_parallelism,
                        iteration,
                        reanalysis_reason,
                        analyzer: &analyzer,
                        semaphore: semaphore.clone(),
                        join_set: &mut join_set,
                        cleanup_guard: &mut cleanup_guard,
                    },
                    &mut reanalysis_reason,
                )
                .await?
            {
                iteration = new_iteration;

                if should_break {
                    break;
                }
            }

            // Step 3: Check if all work is done (before waiting on select)
            if self
                .should_exit_when_idle(join_set.is_empty(), &queued, &in_flight)
                .await
            {
                info!(
                    "All automatic scheduler work completed or blocked-only, exiting scheduler loop"
                );
                break;
            }

            if self
                .should_enter_persistent_idle_wait(join_set.is_empty(), &queued, &in_flight)
                .await
            {
                self.wait_for_persistent_idle_wake_with_tx(
                    &mut reanalysis_reason,
                    &merge_result_tx,
                    &mut merge_result_rx,
                )
                .await;
                continue;
            }

            self.wait_for_scheduler_event(
                &mut join_set,
                &mut in_flight,
                max_parallelism,
                &merge_result_tx,
                &mut merge_result_rx,
                &mut reanalysis_reason,
            )
            .await;
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

    /// Run the scheduler loop's queued re-analysis/dispatch evaluation and apply the
    /// loop-owned trigger lifetime.
    ///
    /// Returns `None` when there is no queued work: that loop iteration performs no
    /// evaluation, so an edge-triggered reason stays armed for the next eligible
    /// iteration instead of being silently discarded.
    ///
    /// Trigger lifetime is owned here rather than inside `perform_reanalysis_and_dispatch`
    /// so the helper keeps its single-evaluation direct-call semantics.  Once a queued
    /// evaluation has actually consumed an edge-triggered reason, it is reset to the
    /// non-bypass `Initial` state so later timer-only wakes fall back to the ordinary
    /// bounded queue-debounce policy instead of replaying the same edge.
    pub(super) async fn evaluate_queued_reanalysis_and_dispatch<F>(
        &mut self,
        ctx: ReanalysisDispatchContext<'_, F>,
        reanalysis_reason: &mut ReanalysisReason,
    ) -> Result<Option<(bool, u32)>>
    where
        for<'a> F: Fn(
                &'a [crate::openspec::Change],
                &'a [String],
                u32,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
            > + Send
            + Sync,
    {
        if ctx.queued.is_empty() {
            return Ok(None);
        }

        let evaluated_reason = ctx.reanalysis_reason;
        let result = self.perform_reanalysis_and_dispatch(ctx).await?;

        if *reanalysis_reason == evaluated_reason && evaluated_reason.is_one_shot_edge_trigger() {
            *reanalysis_reason = ReanalysisReason::Initial;
        }

        Ok(Some(result))
    }

    async fn wait_for_scheduler_event(
        &mut self,
        join_set: &mut JoinSet<WorkspaceResult>,
        in_flight: &mut HashSet<String>,
        max_parallelism: usize,
        merge_result_tx: &tokio::sync::mpsc::Sender<super::MergeResult>,
        merge_result_rx: &mut tokio::sync::mpsc::Receiver<super::MergeResult>,
        reanalysis_reason: &mut ReanalysisReason,
    ) {
        tokio::select! {
            // Join completion: task finished (apply+archive)
            Some(result) = join_set.join_next() => {
                match result {
                    Ok(workspace_result) => {
                        self.handle_workspace_completion(workspace_result, max_parallelism, in_flight, merge_result_tx).await;

                        // Re-analysis is state-derived each loop.
                        // If a manual resolve is still active, keep the generic completion reason;
                        // otherwise treat the slot release as resolve-aware capacity recovery.
                        let manual_resolves_active = self
                            .manual_resolve_count
                            .as_ref()
                            .map(|counter| counter.load(std::sync::atomic::Ordering::Relaxed))
                            .unwrap_or(0);
                        *reanalysis_reason = if manual_resolves_active == 0 {
                            ReanalysisReason::ResolveCompletion
                        } else {
                            ReanalysisReason::Completion
                        };
                        self.trigger_resolve_wait_retry_dispatch();
                    }
                    Err(e) => {
                        error!("Task panicked: {:?}", e);
                    }
                }
            }

            // Background merge completion: merge+cleanup finished asynchronously
            Some(merge_result) = merge_result_rx.recv() => {
                let merged = self.handle_merge_result_with_tx(merge_result, merge_result_tx).await;
                if merged {
                    self.trigger_resolve_wait_retry_dispatch();
                    *reanalysis_reason = ReanalysisReason::ResolveCompletion;
                }
            }

            // Queue notification: dynamic queue has new items or scheduler-owned retry work
            Some(_) = self.wait_for_dynamic_queue_notification() => {
                info!("Queue notification received, will check queue on next iteration");
                self.trigger_resolve_wait_retry_dispatch();
                *reanalysis_reason = ReanalysisReason::QueueNotification;
            }

            // Cancellation should wake promptly even while the scheduler is waiting for work.
            _ = self.wait_for_cancellation(), if self.cancel_token.is_some() => {
                info!("Cancellation received while scheduler is waiting for events");
            }

            // Debounce timer: wait before allowing re-analysis
            _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                // Timer expired; next loop derives re-analysis from current scheduler state.
            }
        }
    }

    #[allow(dead_code)]
    pub(super) async fn wait_for_persistent_idle_wake(
        &mut self,
        reanalysis_reason: &mut ReanalysisReason,
        merge_result_rx: &mut tokio::sync::mpsc::Receiver<super::MergeResult>,
    ) {
        let (merge_result_tx, _merge_result_rx) = tokio::sync::mpsc::channel(1);
        self.wait_for_persistent_idle_wake_with_tx(
            reanalysis_reason,
            &merge_result_tx,
            merge_result_rx,
        )
        .await;
    }

    pub(super) async fn wait_for_persistent_idle_wake_with_tx(
        &mut self,
        reanalysis_reason: &mut ReanalysisReason,
        merge_result_tx: &tokio::sync::mpsc::Sender<super::MergeResult>,
        merge_result_rx: &mut tokio::sync::mpsc::Receiver<super::MergeResult>,
    ) {
        info!(
            "Scheduler idle with no work; waiting for dynamic queue notifications (persistent lifetime)"
        );

        tokio::select! {
            Some(merge_result) = merge_result_rx.recv() => {
                let merged = self.handle_merge_result_with_tx(merge_result, merge_result_tx).await;
                if merged {
                    self.trigger_resolve_wait_retry_dispatch();
                    *reanalysis_reason = ReanalysisReason::ResolveCompletion;
                }
            }

            Some(_) = self.wait_for_dynamic_queue_notification() => {
                info!("Queue notification received while scheduler idle; resuming scheduler loop");
                self.trigger_resolve_wait_retry_dispatch();
                *reanalysis_reason = ReanalysisReason::QueueNotification;
            }

            _ = self.wait_for_cancellation(), if self.cancel_token.is_some() => {
                info!("Cancellation received while scheduler idle; resuming scheduler loop");
            }
        }
    }

    async fn wait_for_dynamic_queue_notification(&self) -> Option<()> {
        if let Some(queue) = &self.dynamic_queue {
            queue.notified().await;
            Some(())
        } else {
            std::future::pending().await
        }
    }

    async fn wait_for_cancellation(&self) {
        if let Some(token) = &self.cancel_token {
            token.cancelled().await;
        } else {
            std::future::pending::<()>().await;
        }
    }
}
