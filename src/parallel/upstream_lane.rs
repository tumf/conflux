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
use crate::upstream::coordinator::{
    scan_pending_publications, FinalizeOutcome, PublicationOutcome, SchedulerOutcome,
    UpstreamStepOutcome,
};
use crate::upstream::git_ops::GitUpstreamOps;
use crate::upstream::PublicationEvidence;

use super::ParallelExecutor;

/// What a change-scoped publication produced for the base lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PublicationLaneOutcome {
    /// Remote observation confirms cumulative HEAD contains this change.
    Confirmed { head: String },
    /// The bounded cycle did not converge. The change is not published, the base
    /// lane stays closed to later results, and retry resumes from Git evidence.
    Unpublished { reason: String },
}

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

    /// Selected remote and cumulative base branch, when upstream is installed.
    ///
    /// Used only to label change-scoped publication events; it is never routing
    /// input.
    pub(super) async fn upstream_identity(&self) -> Option<(String, String)> {
        let upstream = self.upstream.clone()?;
        let coordinator = upstream.lock().await;
        Some((
            coordinator.config().remote.clone(),
            coordinator.branch().to_string(),
        ))
    }

    /// Publication-required integrations that are not proven remote-reachable.
    ///
    /// This is repository evidence, not process memory, so it answers the same
    /// way after a restart as it does mid-run. A disabled run never calls it.
    pub(super) async fn pending_publications(&self) -> Vec<PublicationEvidence> {
        if !self.upstream_enabled() {
            return Vec::new();
        }
        let git = GitUpstreamOps::new(&self.repo_root);
        match scan_pending_publications(&git).await {
            Ok(evidence) => evidence,
            Err(err) => {
                // An unreadable repository must not silently unblock the lane;
                // the caller treats "unknown" as "nothing pending" only because
                // the merge path re-checks base cleanliness independently.
                tracing::warn!(error = %err, "Pending publication scan unavailable");
                Vec::new()
            }
        }
    }

    /// Change ID of a pending publication owned by a *different* change.
    ///
    /// While this is `Some`, no later completed result may enter cumulative base:
    /// the prior change's published revision is not yet known, so attribution of
    /// a later publication would be ambiguous.
    pub(super) async fn blocking_publication_change(&self, current: &str) -> Option<String> {
        self.pending_publications()
            .await
            .into_iter()
            .map(|evidence| evidence.trailers.change_id)
            .find(|change_id| change_id != current)
    }

    /// Whether this change already has durable publication-required evidence.
    ///
    /// True means the change is already integrated into cumulative base and owes
    /// only publication, so retry must resume at the publication boundary instead
    /// of merging it a second time.
    pub(super) async fn has_pending_publication_for(&self, change_id: &str) -> bool {
        self.pending_publications()
            .await
            .iter()
            .any(|evidence| evidence.trailers.change_id == change_id)
    }

    /// Record durable publication-required identity for a locally integrated
    /// change, before it may be treated as integrated.
    pub(super) async fn record_publication_intent(&self, change_id: &str) -> Result<()> {
        let Some(upstream) = self.upstream.clone() else {
            return Ok(());
        };
        let mut coordinator = upstream.lock().await;
        let marker = coordinator.record_publication_intent(change_id).await?;
        tracing::info!(
            change_id = %change_id,
            marker = %marker,
            "Recorded publication-required identity for cumulative base integration"
        );
        Ok(())
    }

    /// Run one change-scoped publication cycle while the base lane is held.
    pub(super) async fn publish_completed_change(
        &self,
        change_id: &str,
    ) -> Result<PublicationLaneOutcome> {
        let Some(upstream) = self.upstream.clone() else {
            return Ok(PublicationLaneOutcome::Confirmed {
                head: String::new(),
            });
        };

        let mut coordinator = upstream.lock().await;
        Ok(match coordinator.publish_change(change_id).await? {
            PublicationOutcome::Published { head }
            | PublicationOutcome::AlreadyConfirmed { head } => {
                PublicationLaneOutcome::Confirmed { head }
            }
            PublicationOutcome::Stalled { reason } => {
                PublicationLaneOutcome::Unpublished { reason }
            }
        })
    }

    /// Resume publication for every change that repository evidence shows is
    /// integrated but unpublished.
    ///
    /// This is the restart and explicit-retry entry point. It creates no apply or
    /// acceptance dispatch: the change is already in cumulative base, so the only
    /// outstanding work is verification, native push, and remote confirmation.
    /// The base lane is taken for each attempt and released when it finishes, so
    /// a stalled publication keeps later results waiting rather than silently
    /// letting them integrate.
    ///
    /// Every attempt is attributed to the change ID recorded in its marker, so
    /// resumption produces the same change-scoped `PushStarted`/`PushCompleted`
    /// pair a fresh integration does. Nothing here may quietly give up: an
    /// abandoned marker would let run-final publication push the same cumulative
    /// HEAD with no change attribution, and the run would report completion for
    /// a change the reducer never saw confirmed.
    ///
    /// Returns the change IDs that are still unpublished afterwards.
    pub(super) async fn resume_pending_publications(&mut self) -> Vec<String> {
        /// Base-lane contention and transient base dirtiness are both expected
        /// to clear; a bounded wait is the difference between "resumed" and
        /// "silently skipped".
        const BASE_LANE_ATTEMPTS: u32 = 30;
        const BASE_LANE_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);
        /// Bounded so a wedged lane holder surfaces as owed publication — which
        /// withholds run completion — instead of hanging the caller forever.
        const BASE_LANE_ACQUIRE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

        let pending: Vec<String> = self
            .pending_publications()
            .await
            .into_iter()
            .map(|evidence| evidence.trailers.change_id)
            .collect();

        let mut unpublished = Vec::new();
        for change_id in pending {
            // Wait for the base lane rather than skipping: this runs before the
            // scheduler dispatches and at run finalization, so contention here is
            // another base operation finishing, not a permanent condition.
            let lane =
                tokio::time::timeout(BASE_LANE_ACQUIRE_TIMEOUT, super::global_merge_lock().lock())
                    .await;
            let Ok(_lane) = lane else {
                tracing::error!(
                    change_id = %change_id,
                    "Base lane never became available; publication remains owed"
                );
                unpublished.push(change_id);
                break;
            };

            let mut base_ready = false;
            for attempt in 1..=BASE_LANE_ATTEMPTS {
                match super::merge::base_dirty_reason(&self.repo_root).await {
                    Ok(None) => {
                        base_ready = true;
                        break;
                    }
                    Ok(Some(reason)) => {
                        tracing::warn!(
                            change_id = %change_id,
                            reason = %reason,
                            attempt,
                            "Cumulative base is not clean; retrying publication resumption"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(
                            change_id = %change_id,
                            error = %err,
                            attempt,
                            "Base state unavailable; retrying publication resumption"
                        );
                    }
                }
                tokio::time::sleep(BASE_LANE_RETRY_DELAY).await;
            }

            if !base_ready {
                tracing::error!(
                    change_id = %change_id,
                    "Cumulative base never became usable; publication remains owed"
                );
                unpublished.push(change_id);
                // Later markers depend on this one's published revision for
                // unambiguous attribution, so stop here rather than publishing
                // a cumulative HEAD that carries an unconfirmed change.
                break;
            }

            tracing::info!(
                change_id = %change_id,
                "Resuming unpublished cumulative-base integration from repository evidence"
            );
            if let Err(err) = self.publish_base_integration(&change_id, None, true).await {
                tracing::warn!(
                    change_id = %change_id,
                    error = %err,
                    "Publication resumption did not complete; change remains resumable"
                );
                // A stalled publication keeps the lane closed for later results
                // by leaving its marker in place. Attribution of any later
                // marker would be ambiguous while this one is unconfirmed.
                unpublished.push(change_id);
                break;
            }

            // Repository evidence, not the call's return value, decides whether
            // the marker is discharged.
            if self.has_pending_publication_for(&change_id).await {
                tracing::warn!(
                    change_id = %change_id,
                    "Publication resumption reported success but the marker is still unpublished"
                );
                unpublished.push(change_id);
                break;
            }
        }

        unpublished
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
