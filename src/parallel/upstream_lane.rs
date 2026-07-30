//! Base-lane wiring for opt-in upstream integration.
//!
//! Every method here short-circuits when no coordinator is installed, which is
//! what makes the default-off path a hard compatibility boundary: a disabled run
//! never fetches, merges, verifies, pushes, or emits upstream evidence.
//!
//! Checkpoints are requested only at the deterministic boundaries the spec
//! defines. There is no scheduler-loop polling and no time-based polling: the
//! scheduler asks at an edge, and [`crate::upstream::CheckpointScheduler`]
//! decides whether the checkpoint starts, batches, or defers.

use crate::error::{OrchestratorError, Result};
use crate::upstream::checkpoint::{BaseLaneState, CheckpointTrigger};
use crate::upstream::coordinator::{FinalizeOutcome, SchedulerOutcome, UpstreamStepOutcome};

use super::ParallelExecutor;

impl ParallelExecutor {
    /// Whether this run opted in to upstream integration.
    pub(super) fn upstream_enabled(&self) -> bool {
        self.upstream.is_some()
    }

    /// Observe base-lane safety for a checkpoint request.
    ///
    /// `lane_owned` is true when the caller already holds the project base lane
    /// (the global merge lock) and has confirmed the base is clean.
    async fn observe_base_lane(&self, lane_owned: bool) -> BaseLaneState {
        if lane_owned {
            return BaseLaneState::clean();
        }

        let base_dirty_reason = super::merge::base_dirty_reason(&self.repo_root)
            .await
            .unwrap_or_else(|err| Some(format!("base state unavailable: {}", err)));

        let lane_busy_reason = if base_dirty_reason.is_some() {
            None
        } else if super::global_merge_lock().try_lock().is_err() {
            Some("project base lane is owned by another base operation".to_string())
        } else {
            None
        };

        BaseLaneState {
            base_dirty_reason,
            lane_busy_reason,
        }
    }

    /// Run an upstream checkpoint at a deterministic boundary.
    ///
    /// A deferred or batched checkpoint is not an error: the requesting edge
    /// simply keeps its result queued. A stall is an error so the base lane stays
    /// closed and later base-dependent dispatch is blocked.
    pub(super) async fn run_upstream_checkpoint(
        &self,
        trigger: CheckpointTrigger,
        pending_result: Option<&str>,
        lane_owned: bool,
    ) -> Result<()> {
        let Some(upstream) = self.upstream.clone() else {
            return Ok(());
        };

        let lane = self.observe_base_lane(lane_owned).await;
        let mut coordinator = upstream.lock().await;
        match coordinator
            .checkpoint(trigger, &lane, pending_result)
            .await?
        {
            UpstreamStepOutcome::Stalled { reason } => Err(OrchestratorError::GitCommand(format!(
                "upstream checkpoint stalled: {}",
                reason
            ))),
            UpstreamStepOutcome::NoOp { .. }
            | UpstreamStepOutcome::Integrated { .. }
            | UpstreamStepOutcome::Deferred { .. } => Ok(()),
        }
    }

    /// Run the complete verification command after a completed change result
    /// merged into cumulative base.
    ///
    /// The caller holds the base lane, so a failure keeps it closed.
    pub(super) async fn run_upstream_base_result_verification(
        &self,
        change_id: &str,
    ) -> Result<()> {
        let Some(upstream) = self.upstream.clone() else {
            return Ok(());
        };

        let mut coordinator = upstream.lock().await;
        match coordinator.verify_base_result(change_id).await? {
            UpstreamStepOutcome::Stalled { reason } => Err(OrchestratorError::GitCommand(format!(
                "upstream verification blocked base integration: {}",
                reason
            ))),
            _ => Ok(()),
        }
    }

    /// Own finalization for an opted-in run.
    ///
    /// Returns `true` when the run may report completion. Only a successful
    /// drain can reach verification, push, and remote confirmation.
    pub(super) async fn finalize_upstream(&self, outcome: SchedulerOutcome) -> bool {
        let Some(upstream) = self.upstream.clone() else {
            // Disabled runs keep their existing completion semantics.
            return true;
        };

        let mut coordinator = upstream.lock().await;
        match coordinator.finalize(outcome).await {
            Ok(FinalizeOutcome::Completed { pushed_head }) => {
                tracing::info!(
                    pushed_head = %pushed_head,
                    "Upstream integration published verified cumulative base"
                );
                true
            }
            Ok(FinalizeOutcome::NoWork) => {
                tracing::info!("Upstream integration completed with no work to publish");
                true
            }
            Ok(FinalizeOutcome::Skipped { reason }) => {
                tracing::warn!(reason = %reason, "Upstream finalization skipped");
                false
            }
            Ok(FinalizeOutcome::Stalled { reason }) => {
                tracing::error!(reason = %reason, "Upstream finalization stalled");
                false
            }
            Err(err) => {
                tracing::error!(error = %err, "Upstream finalization failed");
                false
            }
        }
    }
}
