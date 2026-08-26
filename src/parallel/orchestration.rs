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
//! An evaluation that found zero slots is the one case where an edge survives its
//! pass: the expensive analyzer never ran, so nothing the trigger was asking for was
//! evaluated, and consuming it would leave capacity recovery with no immediate signal
//! at all. See `ParallelExecutor::analyzer_capacity_suppressed`.
//!
//! Coverage lives in `src/parallel/tests/reanalysis_trigger_lifetime.rs` (edge lifetime,
//! merge-outcome capacity release), `src/parallel/tests/capacity_gated_reanalysis.rs`
//! (the zero-capacity analyzer gate and slot recovery through it),
//! `src/parallel/tests/conflict.rs` (auto-resolve guard release on success / failure /
//! early return), and `src/parallel/tests/auto_resolve.rs` (deferred retry convergence).

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
use super::queue_state::{
    QueueReconciliationOutcome, ReanalysisDispatchContext, RetryEdgeConsumption,
};
use super::types::WorkspaceResult;
use super::work_snapshot::ReducerWorkSnapshot;
use super::ParallelEvent;
use super::ParallelExecutor;
use super::SchedulerLifetime;
use super::SchedulerRunReport;
use crate::upstream::coordinator::SchedulerOutcome;

/// Bounded deadline for handling pending background merge/base-lane results on the
/// cancellation exit.
///
/// The scheduler must not establish terminal stop while a detached post-archive merge
/// or base-lane retry may still be mutating the base repository, so the cancellation
/// exit keeps receiving merge results until `pending_merge_count` reaches zero. This
/// deadline is deliberately shorter than the outer boundary's cleanup barrier
/// (`crate::tui::orchestrator::PARALLEL_CANCELLATION_CLEANUP_DEADLINE`, 120s) so a
/// merge task that never reports escalates through this bounded wait — with an
/// operator-visible diagnostic — instead of consuming the whole outer barrier.
pub(super) const CANCELLATION_MERGE_DRAIN_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(90);

/// Bounded deadline for proving that every run-owned AI command is quiescent on
/// the cancellation and run-fatal exits.
///
/// Shared with the command-runner layer so there is one number, and it is
/// consumed from the same absolute cancellation start as the merge drain:
/// 30s + 90s exactly fills the 120s outer boundary rather than stacking two
/// fresh budgets on top of it.
pub(super) const RUN_COMMAND_CLEANUP_DEADLINE: std::time::Duration =
    crate::ai_command_runner::RUN_COMMAND_CLEANUP_DEADLINE;

/// Remaining slice of one absolute cleanup timeline.
///
/// Nested waits share `started` instead of each restarting a full budget, so
/// the sum of the command-scope wait and the merge drain can never exceed the
/// outer scheduler boundary.
pub(super) fn remaining_cleanup_budget(
    started: std::time::Instant,
    outer: std::time::Duration,
    cap: std::time::Duration,
) -> std::time::Duration {
    cap.min(outer.saturating_sub(started.elapsed()))
}

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

    /// Whether a finite run may stop here.
    ///
    /// Both halves — fully drained and blocked-only — are terminal claims about
    /// reducer-owned work, so both require the evaluation's reducer view to be
    /// complete. A run that could not read reducer evidence has not observed an
    /// empty queue; it has observed nothing, and must loop instead of reporting
    /// `DrainedSuccessfully` or `BlockedOrStalled`.
    ///
    /// `work_snapshot` is the evaluation-scoped view when the loop already
    /// captured one, or `None` to await an equivalent view here.
    pub(super) async fn should_exit_when_idle(
        &self,
        join_set_empty: bool,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        work_snapshot: Option<&ReducerWorkSnapshot>,
    ) -> bool {
        if self.scheduler_lifetime != SchedulerLifetime::Finite || !join_set_empty {
            return false;
        }
        let captured;
        let work_snapshot = match work_snapshot {
            Some(snapshot) => snapshot,
            None => {
                captured = self.capture_reducer_work_snapshot().await;
                &captured
            }
        };
        if !work_snapshot.is_complete() {
            return false;
        }
        self.is_fully_drained(join_set_empty, queued.is_empty(), in_flight.is_empty())
            || self
                .is_blocked_only_scheduler_state_with_snapshot(queued, in_flight, work_snapshot)
                .await
    }

    /// Whether a persistent run may park in the event-driven idle wait.
    ///
    /// That wait has no timer by design, so entering it on incomplete reducer
    /// evidence is what let queued intent sit untouched until an unrelated
    /// queue, merge, or cancellation event happened to arrive.
    pub(super) async fn should_enter_persistent_idle_wait(
        &self,
        join_set_empty: bool,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        work_snapshot: Option<&ReducerWorkSnapshot>,
    ) -> bool {
        if self.scheduler_lifetime != SchedulerLifetime::Persistent || !join_set_empty {
            return false;
        }
        let captured;
        let work_snapshot = match work_snapshot {
            Some(snapshot) => snapshot,
            None => {
                captured = self.capture_reducer_work_snapshot().await;
                &captured
            }
        };
        if !work_snapshot.is_complete() {
            return false;
        }
        self.is_fully_drained(join_set_empty, queued.is_empty(), in_flight.is_empty())
            || (queued.is_empty()
                && in_flight.is_empty()
                && (!self.resolve_wait_changes.is_empty() || !self.reject_wait_changes.is_empty())
                && self.manual_resolve_active() == 0
                && self.pending_merge_count.load(Ordering::Relaxed) == 0)
            || self
                .is_blocked_only_scheduler_state_with_snapshot(queued, in_flight, work_snapshot)
                .await
    }

    /// Decide whether to park, emitting the one idle transition for this episode.
    ///
    /// The decision and the emission are the same step on purpose: the event
    /// exists to describe *this* park, so it reuses
    /// [`Self::should_enter_persistent_idle_wait`] and its coherent snapshot
    /// rather than a second drain calculation that could disagree with the park
    /// it claims to describe.
    ///
    /// Returns true when the caller must enter the event-driven idle wait.
    pub(super) async fn admit_persistent_idle_wait(
        &self,
        join_set_empty: bool,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        work_snapshot: Option<&ReducerWorkSnapshot>,
    ) -> bool {
        if !self
            .should_enter_persistent_idle_wait(join_set_empty, queued, in_flight, work_snapshot)
            .await
        {
            return false;
        }
        // The baseline is recorded from the same coherent view the park was
        // decided from, and *before* the edge is latched, so the intent this
        // episode is parking on can never be mistaken for intent that arrived
        // after it.
        let captured;
        let work_snapshot = match work_snapshot {
            Some(snapshot) => snapshot,
            None => {
                captured = self.capture_reducer_work_snapshot().await;
                &captured
            }
        };
        self.record_persistent_idle_baseline(work_snapshot);
        if self.latch_persistent_idle() {
            send_event(&self.event_tx, ParallelEvent::PersistentSchedulerIdle).await;
        }
        true
    }

    /// Whether the run owner has an accepted graceful stop pending.
    ///
    /// Read-only: the request belongs to shared run control, which records it
    /// through the scheduler port and withdraws it on cancel-stop. A run no
    /// owner bound a request to never observes one.
    pub(super) fn graceful_stop_requested(&self) -> bool {
        self.graceful_stop
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst))
    }

    /// Settle a pending graceful stop that has no work boundary left to reach.
    ///
    /// A graceful stop means "stop at the next safe boundary". When no
    /// executable, queued, admitted, active, resolve, merge, or cleanup work
    /// remains, that boundary is *this* evaluation: there is nothing left whose
    /// completion could produce a later one. A persistent scheduler that parked
    /// instead would sit in its event-driven wait with no timer and no pending
    /// work to wake it, leaving every frontend in `Stopping` indefinitely.
    ///
    /// The drain evidence is [`Self::is_fully_drained`] over the evaluation's own
    /// coherent reducer view, so this settles only the genuinely empty case: a
    /// blocked-only park, a resolve or reject wait, and a pending merge all keep
    /// work the stop is still owed, and reach their existing boundary as before.
    /// An incomplete view has observed nothing and never settles.
    ///
    /// Emits the run's one terminal `Stopped` and reports whether the caller must
    /// leave its loop.
    pub(super) async fn settle_graceful_stop_when_no_work(
        &self,
        join_set_empty: bool,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        work_snapshot: &ReducerWorkSnapshot,
    ) -> bool {
        if !self.graceful_stop_requested() || !work_snapshot.is_complete() {
            return false;
        }
        if !self.is_fully_drained(join_set_empty, queued.is_empty(), in_flight.is_empty()) {
            return false;
        }
        info!(
            "Graceful stop requested with no work remaining; settling the scheduler at this boundary"
        );
        send_event(&self.event_tx, ParallelEvent::Stopped).await;
        true
    }

    /// Drive the graceful-stop boundary from outside a scheduler pass.
    ///
    /// The loop calls [`Self::settle_graceful_stop_when_no_work`] with the
    /// coherent view and the reconciled candidate list its own evaluation
    /// produced. Cross-adapter coverage owns no such pass, so this captures an
    /// equivalent view over an idle executor with nothing in flight and takes
    /// `queued` — the reducer queue intent a pass would have reconciled into
    /// scheduler candidates — from the caller, then returns the same decision
    /// through the same code path, including its terminal event.
    #[cfg(test)]
    pub(crate) async fn settle_graceful_stop_at_idle_boundary(
        &self,
        queued: &[crate::openspec::Change],
    ) -> bool {
        let work_snapshot = self.capture_reducer_work_snapshot().await;
        self.settle_graceful_stop_when_no_work(true, queued, &HashSet::new(), &work_snapshot)
            .await
    }

    /// Record the queue intent the episode about to park has already evaluated.
    pub(super) fn record_persistent_idle_baseline(&self, work_snapshot: &ReducerWorkSnapshot) {
        let baseline: HashSet<String> = work_snapshot.queued_intent_ids().iter().cloned().collect();
        *self
            .persistent_idle_baseline
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(baseline);
    }

    /// Rearm the idle edge when this pass observes intent the episode has not
    /// already parked on.
    ///
    /// Level-based on purpose. An individual `AddToQueue` reduction can be a
    /// no-op — a concurrent client enqueue may have queued the same row a moment
    /// earlier — while the coherent queue intent an accepted Start committed is
    /// plainly there to see; rearming from the reduction outcome would lose that
    /// episode's closing Ready edge and leave the frontends Running forever.
    ///
    /// An accepted explicit-retry hold consumed by *this* pass counts the same
    /// way: it is committed operator intent that admits no queued row of its
    /// own.
    ///
    /// A bare notification, a catalog refresh, and dependency analysis observe
    /// neither, so none of them rearms. Neither does a blocked-only park being
    /// woken: its rows are in the baseline, so nothing about them is new.
    pub(super) fn rearm_persistent_idle_from_observed_intent(
        &self,
        work_snapshot: &ReducerWorkSnapshot,
        explicit_retry_edge: bool,
    ) {
        // An incomplete view has observed nothing; treating it as "no new
        // intent" is right, and treating it as new intent would rearm on a lost
        // reducer read.
        if !work_snapshot.is_complete() {
            return;
        }
        let observed_new_intent = {
            let baseline = self
                .persistent_idle_baseline
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match baseline.as_ref() {
                // No episode is open, so there is no edge to rearm.
                None => return,
                Some(parked) => work_snapshot
                    .queued_intent_ids()
                    .iter()
                    .any(|id| !parked.contains(id)),
            }
        };
        if observed_new_intent || explicit_retry_edge {
            self.rearm_persistent_idle();
        }
    }

    /// Open a persistent-idle episode, reporting whether this is its first edge.
    ///
    /// The latch is what separates "the scheduler parked" from "the scheduler is
    /// still parked": the first park in an episode emits, and every later
    /// evaluation or no-op wake that reaches the same park emits nothing.
    pub(super) fn latch_persistent_idle(&self) -> bool {
        !self
            .persistent_idle_latched
            .swap(true, std::sync::atomic::Ordering::SeqCst)
    }

    /// End the current persistent-idle episode so the next park is a new edge.
    ///
    /// Called from the boundaries where admitted work really begins — ordinary
    /// workspace preparation and a scheduler-owned base-lane retry — and from
    /// the coherent pass that observes committed queue or explicit-retry intent
    /// the open episode never parked on. Never from a bare wake, because a wake
    /// that admits nothing and observes nothing must leave the frontend Ready.
    ///
    /// The parked baseline is dropped with the episode: whatever intent the next
    /// park observes belongs to that park, not to this one.
    pub(super) fn rearm_persistent_idle(&self) {
        self.persistent_idle_latched
            .store(false, std::sync::atomic::Ordering::SeqCst);
        *self
            .persistent_idle_baseline
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }

    /// Whether a persistent-idle episode is currently open (tests).
    #[cfg(test)]
    pub(super) fn persistent_idle_is_latched(&self) -> bool {
        self.persistent_idle_latched
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Whether this invocation recorded any change-local base-lane failure.
    ///
    /// Invocation-scoped and in-memory: it exists so a finite run cannot report
    /// plain success while a change sits in manual `MergeWait`.
    pub(super) fn had_change_failures(&self) -> bool {
        !self.change_failures_this_run.is_empty()
    }

    /// Derive one scheduler pass's reanalysis reason from what that pass observed.
    ///
    /// This is where an explicit-retry edge becomes bypass-carrying authority
    /// rather than an ordinarily suppressible wake. Armed bypass authority ranks
    /// with a real queue addition, because both are committed operator state
    /// transitions the retried target is owed an immediate evaluation for — and
    /// an acceptance-stall retry produces *no* queue addition at all, since the
    /// reducer already listed the target as queued work.
    ///
    /// Expressing the authority as [`ReanalysisReason::QueueNotification`] is not
    /// a downgrade: that reason already bypasses queue debounce and
    /// unchanged-analysis-input suppression. Reducing it to `Initial` before the
    /// authorized evaluation is what loses it, which is exactly what the last
    /// branch must not do while an edge is still armed.
    pub(super) fn derive_pass_reanalysis_reason(
        current: ReanalysisReason,
        retry_edges: RetryEdgeConsumption,
        reconciliation: QueueReconciliationOutcome,
        dynamic_queue_added: bool,
    ) -> ReanalysisReason {
        if reconciliation.has_queued_additions() || retry_edges.bypass_armed {
            ReanalysisReason::QueueNotification
        } else if reconciliation.has_repair_additions() {
            ReanalysisReason::RepairCandidate
        } else if matches!(current, ReanalysisReason::QueueNotification) && !dynamic_queue_added {
            // A scheduler wake without scheduler-visible queue additions remains
            // debounceable.  `QueueNotification` only carries operator-intent
            // priority into analysis when the current loop ingested new queued work.
            ReanalysisReason::Initial
        } else {
            current
        }
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
    ///
    /// Returns the run's terminal report. A scheduler failure — including a
    /// run-fatal `AbortRun` — is the `Err` half; `Ok` distinguishes a clean
    /// completion from one that drained with unresolved change-local failures.
    pub async fn execute_with_order_based_reanalysis<F>(
        &mut self,
        changes: Vec<crate::openspec::Change>,
        analyzer: F,
    ) -> Result<SchedulerRunReport>
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
        // A deferred explicit-target plan or an enabled upstream run must reach
        // the checkpoint/classification boundary below even with an empty queue:
        // classification can still discover resumable work, and an all-completed
        // run still owes upstream recovery/finalization.
        let must_reach_upstream_boundary =
            self.upstream_enabled() || self.explicit_target_plan.is_some();

        if changes.is_empty() && !must_reach_upstream_boundary {
            // Awaited, not attempted: this gate reports a *completed* run, so a
            // reducer write that happened to overlap startup must delay it, not
            // convert an unreadable reducer into "there was never any queued
            // work". An incomplete view (cancellation) falls through to the loop,
            // whose cancellation branch owns that exit.
            let startup_snapshot = self.capture_reducer_work_snapshot().await;
            let reducer_has_queued_intent = !startup_snapshot.queued_intent_ids().is_empty();
            let reducer_has_lane_wait = !startup_snapshot.resolve_wait_ids().is_empty()
                || !startup_snapshot.reject_wait_ids().is_empty();
            if startup_snapshot.is_complete()
                && !reducer_has_queued_intent
                && !reducer_has_lane_wait
            {
                return Ok(self.finish_completed_run().await);
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

        // The scope observes the run's global token directly, so a caller
        // blocked on command output far from this loop — inline dependency
        // analysis, for one — starts cleaning up the instant cancellation
        // happens instead of waiting to return here first.
        if let Some(token) = &self.cancel_token {
            self.run_command_scope.link_cancellation(token.clone());
        }

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

        // Deterministic checkpoint boundary: before the first worktree dispatch.
        // A disabled run does nothing here.
        if self.upstream_enabled() {
            if let Err(err) = self
                .run_upstream_checkpoint(
                    crate::upstream::checkpoint::CheckpointTrigger::BeforeFirstDispatch,
                    None,
                    false,
                )
                .await
            {
                let error_msg = format!("Upstream pre-dispatch checkpoint failed: {}", err);
                error!("{}", error_msg);
                send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                return Err(err);
            }

            // Restart and explicit-retry recovery. A publication-required
            // integration that is not remote-reachable is resumable publication
            // work; it must never become ordinary apply or acceptance dispatch,
            // and it must not be reported as terminal `merged`. Anything left
            // unpublished here is re-attempted at the completion gate below.
            let _ = self.resume_pending_publications().await;
        }

        // Deferred explicit-target classification boundary.
        //
        // This runs after the initial upstream checkpoint, so an enabled `-u`
        // run classifies against the resulting cumulative base, and before any
        // change-worktree creation or reuse registration, so an unresolvable
        // target set never mutates a workspace.
        let changes = match self.apply_explicit_target_plan(changes).await {
            Ok(changes) => changes,
            Err(err) => {
                let error_msg = format!("Explicit target resolution failed: {}", err);
                error!("{}", error_msg);
                send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                return Err(err);
            }
        };

        // Initialize scheduler state
        let max_parallelism = self.workspace_manager.max_concurrent();
        let semaphore = Arc::new(Semaphore::new(max_parallelism));
        let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
        let (merge_result_tx, mut merge_result_rx) = self.take_merge_result_channel();
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
        // Set when an accepted graceful stop had no work boundary left to reach
        // and settled the run at its own idle evaluation. Its terminal `Stopped`
        // was emitted there, so the exits below owe no second one.
        let mut graceful_stop_settled = false;
        // Set when the loop exits because nothing is dispatchable while blocked
        // or waiting work remains. Such a run owes a truthful blocked report,
        // never `AllCompleted`.
        let mut blocked_exit = false;
        // Explicit scheduler outcome. Only `DrainedSuccessfully` may enter
        // upstream finalization (final checkpoint, verification, push,
        // confirmation); blocked/stalled and cancelled outcomes never push.
        let mut scheduler_outcome = SchedulerOutcome::BlockedOrStalled;

        // Main scheduler loop: wait for triggers and dispatch changes
        loop {
            // Run-fatal abort. The queue boundary already emitted the single
            // global Error, so this owes the other half of that contract: stop
            // admitting work, bounded-drain everything this run owns through the
            // same managed cleanup path cancellation uses, and fail. A frontend
            // Error without a run that actually stopped is exactly what this
            // branch exists to prevent.
            if self.run_fatal_abort.is_some() {
                let remaining: Vec<String> = queued.iter().map(|c| c.id.clone()).collect();
                error!(
                    queued = remaining.len(),
                    in_flight = in_flight.len(),
                    "Run-fatal base-lane outcome; stopping dispatch and draining owned work"
                );
                // One absolute timeline for every nested cleanup wait below.
                let shutdown_started = std::time::Instant::now();
                // Admission closes and runner tasks are signalled *before* the
                // workspace futures are aborted: an aborted future drops its
                // `StreamingChildHandle`, which is not a cancellation channel
                // this run may depend on.
                self.run_command_scope.close();
                join_set.abort_all();
                while let Some(result) = join_set.join_next().await {
                    if let Err(err) = result {
                        if !err.is_cancelled() {
                            warn!(error = %err, "In-flight workspace task failed while draining after run-fatal abort");
                        }
                    }
                }
                // Nothing Conflux owns — preparation release, workspace
                // cleanup, or the truthful `done` handshakes — may run while a
                // command registration for that worktree is still live.
                self.await_run_command_quiescence(shutdown_started, "run-fatal abort")
                    .await;
                self.release_execution_handles_after_cancellation().await;
                self.clear_preparation_for_aborted_changes(&in_flight).await;
                in_flight.clear();
                queued.clear();
                self.drain_pending_merge_results_after_cancellation(
                    &merge_result_tx,
                    &mut merge_result_rx,
                    remaining_cleanup_budget(
                        shutdown_started,
                        crate::tui::orchestrator::PARALLEL_CANCELLATION_CLEANUP_DEADLINE,
                        CANCELLATION_MERGE_DRAIN_DEADLINE,
                    ),
                )
                .await;
                break;
            }

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
                scheduler_outcome = SchedulerOutcome::Cancelled;
                // One absolute timeline for every nested cleanup wait below.
                let shutdown_started = std::time::Instant::now();
                // Close run command admission and signal active runner tasks
                // before the workspace futures are aborted. Abort drops the
                // caller-held `StreamingChildHandle`, so a run that relied on
                // that handle would have no way left to reach the process group.
                self.run_command_scope.close();
                join_set.abort_all();
                while let Some(result) = join_set.join_next().await {
                    if let Err(err) = result {
                        if !err.is_cancelled() {
                            warn!(error = %err, "In-flight workspace task failed while draining after cancellation");
                        }
                    }
                }
                // Dropping the workspace futures proves nothing about the
                // processes they owned, so the command barrier comes before
                // handle release, preparation clearing, and terminal stop.
                self.await_run_command_quiescence(shutdown_started, "operator cancellation")
                    .await;
                // Aborted tasks never reach `handle_workspace_completion`, so their
                // registered execution handles are released here instead.
                self.release_execution_handles_after_cancellation().await;
                self.clear_preparation_for_aborted_changes(&in_flight).await;
                in_flight.clear();
                // Detached background merge / base-lane tasks may still be mutating the
                // base repository. Terminal stop follows their result handling.
                self.drain_pending_merge_results_after_cancellation(
                    &merge_result_tx,
                    &mut merge_result_rx,
                    remaining_cleanup_budget(
                        shutdown_started,
                        crate::tui::orchestrator::PARALLEL_CANCELLATION_CLEANUP_DEADLINE,
                        CANCELLATION_MERGE_DRAIN_DEADLINE,
                    ),
                )
                .await;
                break;
            }

            // Step 0: Take accepted explicit-retry edges before reconciliation and
            // classification, so a terminal-error retry's ephemeral failed
            // classification is already gone when this pass classifies its
            // dependents, and so any retried target's analysis-bypass authority is
            // armed before the gates below can suppress it.
            let retry_edges = self.consume_explicit_retry_edges().await;

            // Step 1: Capture one coherent reducer work view for this evaluation.
            //
            // Hint admission, lane-wait synchronization, queue reconciliation,
            // queue/dependency classification, and the drain/idle decision below
            // all read *this* view. Previously each took its own `try_read`, so a
            // single short reducer write could make one stage refuse a hint,
            // another return an empty reconciliation, and a third report every
            // candidate unavailable — leaving a scheduler pass that looked
            // drained or stably blocked while reducer intent was still queued.
            //
            // Acquisition awaits rather than polls, so a writer only delays this
            // pass; Tokio resumes the same evaluation on release without needing
            // another queue mutation or wake notification.
            let work_snapshot = self.capture_reducer_work_snapshot().await;
            if self.is_cancelled() {
                // Acquisition lost the race to cancellation. The top-of-loop
                // cancellation branch owns termination, so nothing here may
                // classify, terminate, or idle on the incomplete view.
                continue;
            }

            // Step 2: Check dynamic queue for newly added changes (TUI mode)
            let dynamic_queue_added = self
                .check_dynamic_queue_and_add_changes_with_snapshot(
                    &mut queued,
                    &in_flight,
                    &mut reanalysis_reason,
                    &work_snapshot,
                )
                .await;

            // Step 3: Sync reducer-owned ResolveWait intent before scheduler drain/idle checks.
            // This keeps manual resolve dispatch reducer-owned while making scheduler work detection truthful.
            self.sync_resolve_wait_from_snapshot(&work_snapshot);
            self.maybe_dispatch_resolve_wait_retry_with_tx(&merge_result_tx)
                .await;

            // Step 4: Reconcile reducer-visible queue intent into scheduler-local candidates.
            let reconciliation = self
                .reconcile_queued_candidates_with_snapshot(&mut queued, &in_flight, &work_snapshot)
                .await;

            // An accepted Start against persistent-idle Ready has already
            // projected Running from its own outcome. This is the other half of
            // that episode: the pass that reconciles the intent it committed
            // rearms the idle edge, so if analysis or classification admits
            // nothing the next park can publish the Ready that closes it. Read
            // as a level from the same coherent view above rather than from the
            // reconciliation outcome, so a concurrent queue addition that made
            // one reduction a no-op cannot swallow the closing edge.
            self.rearm_persistent_idle_from_observed_intent(
                &work_snapshot,
                retry_edges.newly_drained > 0,
            );
            reanalysis_reason = Self::derive_pass_reanalysis_reason(
                reanalysis_reason,
                retry_edges,
                reconciliation,
                dynamic_queue_added,
            );

            // Step 5: Re-analysis decision is derived from scheduler state.
            //
            // `queued` is only trustworthy as drain evidence when reconciliation
            // above ran on complete reducer facts. An incomplete view leaves the
            // local list untouched, and an untouched empty list is absence of
            // evidence, not evidence of absence.
            let work_drained = work_snapshot.is_complete()
                && queued.is_empty()
                && in_flight.is_empty()
                && self.resolve_wait_changes.is_empty()
                && self.reject_wait_changes.is_empty()
                && self.manual_resolve_active() == 0
                && self.pending_merge_count.load(Ordering::Relaxed) == 0;
            if work_drained && self.scheduler_lifetime == SchedulerLifetime::Finite {
                info!(
                    "All changes completed (queued/in-flight/resolve_wait/manual_resolve empty), stopping"
                );
                scheduler_outcome = SchedulerOutcome::DrainedSuccessfully;
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
                        work_snapshot: Some(&work_snapshot),
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

            // Step 6: Check if all work is done (before waiting on select)
            if self
                .should_exit_when_idle(
                    join_set.is_empty(),
                    &queued,
                    &in_flight,
                    Some(&work_snapshot),
                )
                .await
            {
                info!(
                    "All automatic scheduler work completed or blocked-only, exiting scheduler loop"
                );
                // A fully drained loop is a success; a blocked-only exit is not,
                // and must never reach finalization.
                scheduler_outcome = if self.is_fully_drained(
                    join_set.is_empty(),
                    queued.is_empty(),
                    in_flight.is_empty(),
                ) {
                    SchedulerOutcome::DrainedSuccessfully
                } else {
                    blocked_exit = true;
                    SchedulerOutcome::BlockedOrStalled
                };
                break;
            }

            // An accepted graceful stop with nothing left to wait for. This
            // evaluation is the safe boundary the request asked for, so the run
            // ends here instead of parking in a wait that no remaining work
            // could ever end — which is what left the frontends `Stopping` for a
            // scheduler that had already settled.
            if self
                .settle_graceful_stop_when_no_work(
                    join_set.is_empty(),
                    &queued,
                    &in_flight,
                    &work_snapshot,
                )
                .await
            {
                graceful_stop_settled = true;
                break;
            }

            // One typed transition per idle episode, emitted from the same
            // coherent admission decision that is about to park, so the
            // frontends stop claiming Running for a scheduler that has nothing
            // left to execute.
            if self
                .admit_persistent_idle_wait(
                    join_set.is_empty(),
                    &queued,
                    &in_flight,
                    Some(&work_snapshot),
                )
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

        // Send appropriate completion event based on how we exited.
        //
        // Operator cancellation is checked first on purpose: a fatal result can
        // arrive during the cancellation drain, and a cancelled run still owes
        // its one terminal `Stopped` rather than an execution failure.
        if cancelled {
            send_event(&self.event_tx, ParallelEvent::Stopped).await;
            return Ok(SchedulerRunReport::Stopped);
        }

        // A graceful stop that settled at an empty boundary is a stop, not a
        // completion: nothing drained here, so it must never reach upstream
        // finalization or announce `AllCompleted`. Its terminal event was already
        // emitted at the boundary that decided it.
        if graceful_stop_settled {
            return Ok(SchedulerRunReport::Stopped);
        }

        // A run-fatal abort terminates the scheduler future as failure. It has
        // already emitted its one global Error and drained the work it owned, so
        // it never reaches completion or upstream finalization.
        if let Some(detail) = self.run_fatal_abort.clone() {
            return Err(crate::error::OrchestratorError::GitCommand(detail));
        }

        // Every change that reached cumulative base owes change-scoped remote
        // confirmation before this run may report completion. Run-final
        // publication publishes the cumulative HEAD with no change attribution,
        // so a marker that survives to finalization would be published as
        // anonymous run work and the reducer would never see that change reach
        // `pushed`. Discharge markers here, under their own change IDs, first.
        if self.upstream_enabled() {
            let stranded = self.resume_pending_publications().await;
            if !stranded.is_empty() {
                send_event(
                    &self.event_tx,
                    ParallelEvent::Error {
                        message: format!(
                            "Upstream publication is still owed for {}; cumulative base was not published",
                            stranded.join(", ")
                        ),
                    },
                )
                .await;
                return Ok(self.terminal_report());
            }
        }

        // Upstream finalization owns completion ordering for an opted-in run:
        // final checkpoint, complete verification against final cumulative HEAD,
        // fresh ancestry check, one native non-force push, and remote
        // confirmation. `AllCompleted` follows confirmation, never precedes it.
        // A disabled run keeps its existing completion semantics.
        if !self.finalize_upstream(scheduler_outcome).await {
            send_event(
                &self.event_tx,
                ParallelEvent::Error {
                    message: format!(
                        "Upstream integration did not complete ({:?}); cumulative base was not published",
                        scheduler_outcome
                    ),
                },
            )
            .await;
            return Ok(self.terminal_report());
        }

        // Finalization can itself integrate upstream and record a new marker, so
        // repository evidence — not the pre-finalize scan — is the authoritative
        // completion gate.
        if self.upstream_enabled() {
            let stranded: Vec<String> = self
                .pending_publications()
                .await
                .into_iter()
                .map(|evidence| evidence.trailers.change_id)
                .collect();
            if !stranded.is_empty() {
                send_event(
                    &self.event_tx,
                    ParallelEvent::Error {
                        message: format!(
                            "Upstream publication is still owed for {}; the run is not complete",
                            stranded.join(", ")
                        ),
                    },
                )
                .await;
                return Ok(self.terminal_report());
            }
        }

        if blocked_exit {
            return Ok(self.finish_blocked_run(&queued).await);
        }

        Ok(self.finish_completed_run().await)
    }

    /// Emit the terminal output for a finite run that stopped with blocked work.
    ///
    /// No `AllCompleted`: nothing completed. The one operator-facing diagnostic
    /// names the work that is still held, and the typed report tells the calling
    /// boundary to withhold its success announcement.
    async fn finish_blocked_run(&self, queued: &[crate::openspec::Change]) -> SchedulerRunReport {
        let mut blocked: Vec<String> = queued.iter().map(|change| change.id.clone()).collect();
        blocked.sort();
        let message = format!(
            "Processing stopped with blocked work remaining; no dispatchable candidate is available: {}",
            blocked.join(", ")
        );
        warn!("{}", message);
        send_event(&self.event_tx, ParallelEvent::Log(LogEntry::warn(&message))).await;
        SchedulerRunReport::BlockedOrStalled
    }

    /// Emit the terminal completion output for a run that drained on its own.
    ///
    /// Every completion path goes through here so a completed-with-errors run
    /// cannot pick up a success announcement from one exit and a warning from
    /// another.
    async fn finish_completed_run(&self) -> SchedulerRunReport {
        let report = self.terminal_report();
        if report == SchedulerRunReport::CompletedWithErrors {
            // Truthful terminal reporting: eligible work drained, but changes
            // are still waiting for explicit retry. No success message and no
            // global Error — a warning plus the existing terminal event.
            let mut failed: Vec<String> = self.change_failures_this_run.iter().cloned().collect();
            failed.sort();
            let message = format!(
                "Processing completed with errors; unresolved change-local failures preserved for explicit retry: {}",
                failed.join(", ")
            );
            warn!("{}", message);
            send_event(&self.event_tx, ParallelEvent::Log(LogEntry::warn(&message))).await;
        }

        send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
        report
    }

    /// Terminal report for a scheduler run that returned on its own.
    ///
    /// Manual `MergeWait` left by a change-local failure does not block finite
    /// termination, so the report — not the exit path — is what keeps the run
    /// from being announced as plain success.
    fn terminal_report(&self) -> SchedulerRunReport {
        if self.had_change_failures() {
            SchedulerRunReport::CompletedWithErrors
        } else {
            SchedulerRunReport::Completed
        }
    }

    /// Channel used by the scheduler loop for background merge / base-lane results.
    ///
    /// Production runs always create a fresh channel here; only tests may install a
    /// double first (see `ParallelExecutor::merge_result_channel_override`).
    fn take_merge_result_channel(
        &mut self,
    ) -> (
        tokio::sync::mpsc::Sender<super::MergeResult>,
        tokio::sync::mpsc::Receiver<super::MergeResult>,
    ) {
        #[cfg(test)]
        {
            if let Some(channel) = self.merge_result_channel_override.take() {
                return channel;
            }
        }
        tokio::sync::mpsc::channel(64)
    }

    /// Release every registered per-change execution handle after cancellation aborted
    /// the in-flight workspace tasks.
    ///
    /// `handle_workspace_completion` owns handle release on the ordinary completion path,
    /// but an aborted task never returns through it. The dynamic queue outlives one run,
    /// so a handle left registered here would keep reporting positive execution evidence
    /// into a later stop decision in the same session and turn an idle stop into a
    /// "Force stopped" claim about an agent process that no longer exists.
    async fn release_execution_handles_after_cancellation(&self) {
        let Some(queue) = self.dynamic_queue.as_ref() else {
            return;
        };
        let scope = self.run_command_scope.clone();
        let release = queue
            .release_all_execution_handles(|change_id| scope.change_is_quiescent(change_id))
            .await;
        if release.confirmed > 0 {
            info!(
                confirmed = release.confirmed,
                "Released registered execution handles whose run-owned commands reached confirmed cleanup"
            );
        }
        for change_id in &release.unconfirmed {
            warn!(
                change_id = %change_id,
                "Execution handle released without confirmed command cleanup; the completion \
                 handshake stays unfired and its waiter times out truthfully"
            );
        }
    }

    /// Wait for every run-owned AI command to reach quiescence, or for bounded
    /// managed escalation to complete.
    ///
    /// A deadline or an escalation is a cleanup diagnostic, never a
    /// reclassification: the run stays operator cancellation or run-fatal.
    async fn await_run_command_quiescence(
        &self,
        shutdown_started: std::time::Instant,
        reason: &str,
    ) {
        #[allow(unused_mut)]
        let mut cap = RUN_COMMAND_CLEANUP_DEADLINE;
        #[cfg(test)]
        if let Some(override_budget) = self.run_command_cleanup_budget_override {
            cap = override_budget;
        }
        let budget = remaining_cleanup_budget(
            shutdown_started,
            crate::tui::orchestrator::PARALLEL_CANCELLATION_CLEANUP_DEADLINE,
            cap,
        );
        let cleanup = self.run_command_scope.shutdown(budget).await;
        if cleanup.is_quiescent() {
            info!(
                reason,
                escalated = cleanup.escalated,
                "Run-owned AI commands reached process quiescence"
            );
            return;
        }

        let message = format!(
            "Run-owned command cleanup could not be fully proven while stopping ({}): {}",
            reason,
            cleanup.diagnostics()
        );
        warn!("{}", message);
        send_event(&self.event_tx, ParallelEvent::Log(LogEntry::warn(&message))).await;
    }

    /// Clear ephemeral workspace preparation for tasks cancellation aborted.
    ///
    /// An aborted task never returns through `handle_workspace_completion`, so a
    /// change that was still creating or setting up its worktree would otherwise
    /// stay rendered as `preparing` for the rest of the process. The clear is a
    /// no-op for any change that already reached a real phase or a terminal
    /// state, so it cannot rewrite the outcome of work that did finish.
    async fn clear_preparation_for_aborted_changes(&self, in_flight: &HashSet<String>) {
        for change_id in in_flight {
            send_event(
                &self.event_tx,
                ParallelEvent::WorkspacePreparationEnded {
                    change_id: change_id.clone(),
                },
            )
            .await;
        }
    }

    /// Keep handling pending background merge / base-lane results after cancellation,
    /// under a bounded deadline.
    ///
    /// Post-archive merge and base-lane retry tasks are detached, so cancellation does not
    /// abort them: they may still be mutating the base repository when the scheduler loop
    /// breaks. Terminal `Stopped` must not be established before those outcomes have been
    /// received, so this waits for `pending_merge_count` to reach zero through the ordinary
    /// `handle_merge_result_with_tx` path. Exceeding `deadline` leaves the run cancelled and
    /// escalates to the outer bounded cleanup barrier; it is never an execution failure.
    async fn drain_pending_merge_results_after_cancellation(
        &mut self,
        merge_result_tx: &tokio::sync::mpsc::Sender<super::MergeResult>,
        merge_result_rx: &mut tokio::sync::mpsc::Receiver<super::MergeResult>,
        deadline: std::time::Duration,
    ) {
        let pending = self.pending_merge_count.load(Ordering::Relaxed);
        if pending == 0 {
            return;
        }

        let waiting_msg = format!(
            "Waiting for {} pending background merge/base-lane task(s) to reach a safe boundary before stopping",
            pending
        );
        info!("{}", waiting_msg);
        send_event(
            &self.event_tx,
            ParallelEvent::Log(LogEntry::info(&waiting_msg)),
        )
        .await;

        let drained = tokio::time::timeout(deadline, async {
            while self.pending_merge_count.load(Ordering::Relaxed) > 0 {
                // The scheduler still holds a sender, so `None` only happens if the
                // channel is closed elsewhere; treat it as nothing left to receive.
                let Some(merge_result) = merge_result_rx.recv().await else {
                    break;
                };
                self.handle_merge_result_with_tx(merge_result, merge_result_tx)
                    .await;
            }
        })
        .await;

        if drained.is_err() {
            let timeout_msg = format!(
                "Pending background merge/base-lane task(s) did not report within {}s while stopping; continuing shutdown",
                deadline.as_secs()
            );
            warn!("{}", timeout_msg);
            send_event(
                &self.event_tx,
                ParallelEvent::Log(LogEntry::warn(&timeout_msg)),
            )
            .await;
        } else {
            info!("Pending background merge/base-lane tasks reached a safe boundary; stopping");
        }
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

        // A pass that skipped the analyzer purely for lack of capacity evaluated
        // nothing this trigger was asking for. Consuming the edge there would
        // discard the one immediate liveness signal that survives a full
        // scheduler — the recovery wake would then have to rediscover the work
        // through the ordinary debounced timer, or not at all.
        let evaluated = !self.analyzer_capacity_suppressed();
        if evaluated
            && *reanalysis_reason == evaluated_reason
            && evaluated_reason.is_one_shot_edge_trigger()
        {
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
                // Only an actual merge arms success-only follow-up. A
                // change-local failure keeps the run alive without pretending
                // capacity was recovered by a completed merge, and a fatal
                // outcome is picked up by the loop's abort check.
                if self.handle_merge_result_with_tx(merge_result, merge_result_tx).await.is_merged() {
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
                // Only an actual merge arms success-only follow-up. A
                // change-local failure keeps the run alive without pretending
                // capacity was recovered by a completed merge, and a fatal
                // outcome is picked up by the loop's abort check.
                if self.handle_merge_result_with_tx(merge_result, merge_result_tx).await.is_merged() {
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
