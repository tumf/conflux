//! Deterministic project base-lane checkpoint triggering.
//!
//! A checkpoint becomes due only at five repository-derivable edges. There is
//! no scheduler-loop polling and no time-based polling: the scheduler asks this
//! reducer at an edge, and the reducer answers whether the checkpoint starts,
//! is batched behind an already-active checkpoint, or is deferred because the
//! base lane is unsafe.

use std::collections::VecDeque;

/// The five deterministic checkpoint boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckpointTrigger {
    /// Before the first worktree dispatch of the run.
    BeforeFirstDispatch,
    /// Immediately before a completed change result enters cumulative base.
    BeforeBaseIntegration,
    /// After a normal scheduler drain, before finalization.
    AfterDrain,
    /// After a fresh pre-push fetch observed a remote advance.
    PrePushRemoteAdvance,
    /// After a race-time non-fast-forward push rejection.
    PushRaceRejection,
}

/// Observed base-lane safety at the moment a checkpoint is requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseLaneState {
    /// Reason the cumulative base is unusable, when it is not clean.
    pub base_dirty_reason: Option<String>,
    /// Archive integration, merge, resolve, or rejection review owns the lane.
    pub lane_busy_reason: Option<String>,
}

impl BaseLaneState {
    pub fn clean() -> Self {
        Self {
            base_dirty_reason: None,
            lane_busy_reason: None,
        }
    }

    fn unsafe_reason(&self) -> Option<String> {
        self.base_dirty_reason
            .clone()
            .or_else(|| self.lane_busy_reason.clone())
    }
}

/// What the scheduler must do with a checkpoint request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointDecision {
    /// Start a checkpoint now; `generation` identifies this ownership epoch.
    Start { generation: u64 },
    /// A checkpoint already owns the lane; this request shares its single fetch.
    Batched { generation: u64 },
    /// The base lane is unsafe; the entire checkpoint (including fetch) is deferred.
    Deferred { reason: String },
}

/// Reducer owning checkpoint triggering, batching, and stale-event protection.
///
/// This is intentionally in-memory: it never establishes completion. The
/// authoritative routing decision is always recomputed from repository state
/// when the checkpoint actually runs.
#[derive(Debug, Default)]
pub struct CheckpointScheduler {
    generation: u64,
    active: Option<u64>,
    /// Completed change results queued behind the active checkpoint.
    queued_results: VecDeque<String>,
    /// Whether the pre-dispatch checkpoint has already run in this process.
    first_dispatch_done: bool,
}

impl CheckpointScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Current ownership epoch, when a checkpoint owns the lane.
    pub fn active_generation(&self) -> Option<u64> {
        self.active
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Results accumulated behind the active checkpoint, in arrival order.
    pub fn queued_results(&self) -> Vec<String> {
        self.queued_results.iter().cloned().collect()
    }

    /// Request a checkpoint at a deterministic boundary.
    ///
    /// `pending_result` names the completed change result waiting behind this
    /// request, when the trigger is a base integration.
    pub fn request(
        &mut self,
        trigger: CheckpointTrigger,
        lane: &BaseLaneState,
        pending_result: Option<&str>,
    ) -> CheckpointDecision {
        if let Some(result) = pending_result {
            if !self.queued_results.iter().any(|queued| queued == result) {
                self.queued_results.push_back(result.to_string());
            }
        }

        if let Some(generation) = self.active {
            // A checkpoint already owns the lane: this request shares its fetch
            // instead of starting a second one.
            return CheckpointDecision::Batched { generation };
        }

        if let Some(reason) = lane.unsafe_reason() {
            return CheckpointDecision::Deferred { reason };
        }

        if trigger == CheckpointTrigger::BeforeFirstDispatch && self.first_dispatch_done {
            return CheckpointDecision::Deferred {
                reason: "pre-dispatch checkpoint already completed for this run".to_string(),
            };
        }

        self.generation += 1;
        self.active = Some(self.generation);
        if trigger == CheckpointTrigger::BeforeFirstDispatch {
            self.first_dispatch_done = true;
        }
        CheckpointDecision::Start {
            generation: self.generation,
        }
    }

    /// Release lane ownership held by `generation`.
    ///
    /// Returns the batched results the released checkpoint covered. A stale
    /// completion event (an older generation, or one arriving after release)
    /// releases nothing and returns `None`, so it cannot hand the lane to
    /// another owner's work.
    pub fn release(&mut self, generation: u64) -> Option<Vec<String>> {
        if self.active != Some(generation) {
            return None;
        }
        self.active = None;
        Some(self.queued_results.drain(..).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn busy_lane(reason: &str) -> BaseLaneState {
        BaseLaneState {
            base_dirty_reason: None,
            lane_busy_reason: Some(reason.to_string()),
        }
    }

    #[test]
    fn upstream_integration_first_dispatch_checkpoint_starts_once() {
        let mut scheduler = CheckpointScheduler::new();
        assert_eq!(
            scheduler.request(
                CheckpointTrigger::BeforeFirstDispatch,
                &BaseLaneState::clean(),
                None
            ),
            CheckpointDecision::Start { generation: 1 }
        );
        assert_eq!(scheduler.release(1), Some(Vec::new()));
        assert!(matches!(
            scheduler.request(
                CheckpointTrigger::BeforeFirstDispatch,
                &BaseLaneState::clean(),
                None
            ),
            CheckpointDecision::Deferred { .. }
        ));
    }

    #[test]
    fn upstream_integration_checkpoints_do_not_overlap() {
        let mut scheduler = CheckpointScheduler::new();
        let first = scheduler.request(
            CheckpointTrigger::BeforeFirstDispatch,
            &BaseLaneState::clean(),
            None,
        );
        assert_eq!(first, CheckpointDecision::Start { generation: 1 });
        assert_eq!(
            scheduler.request(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None),
            CheckpointDecision::Batched { generation: 1 }
        );
    }

    #[test]
    fn upstream_integration_batches_queued_results_behind_one_fetch() {
        let mut scheduler = CheckpointScheduler::new();
        assert_eq!(
            scheduler.request(
                CheckpointTrigger::BeforeBaseIntegration,
                &BaseLaneState::clean(),
                Some("change-a")
            ),
            CheckpointDecision::Start { generation: 1 }
        );
        assert_eq!(
            scheduler.request(
                CheckpointTrigger::BeforeBaseIntegration,
                &BaseLaneState::clean(),
                Some("change-b")
            ),
            CheckpointDecision::Batched { generation: 1 }
        );
        // Duplicate arrival of the same result must not double-queue it.
        assert_eq!(
            scheduler.request(
                CheckpointTrigger::BeforeBaseIntegration,
                &BaseLaneState::clean(),
                Some("change-b")
            ),
            CheckpointDecision::Batched { generation: 1 }
        );
        assert_eq!(
            scheduler.queued_results(),
            vec!["change-a".to_string(), "change-b".to_string()]
        );
        assert_eq!(
            scheduler.release(1),
            Some(vec!["change-a".to_string(), "change-b".to_string()])
        );
    }

    #[test]
    fn upstream_integration_dirty_base_defers_all_checkpoint_side_effects() {
        let mut scheduler = CheckpointScheduler::new();
        let dirty = BaseLaneState {
            base_dirty_reason: Some("uncommitted changes".to_string()),
            lane_busy_reason: None,
        };
        assert_eq!(
            scheduler.request(CheckpointTrigger::AfterDrain, &dirty, None),
            CheckpointDecision::Deferred {
                reason: "uncommitted changes".to_string()
            }
        );
        assert!(!scheduler.is_active());
    }

    #[test]
    fn upstream_integration_busy_lane_defers_checkpoint() {
        let mut scheduler = CheckpointScheduler::new();
        assert_eq!(
            scheduler.request(
                CheckpointTrigger::BeforeBaseIntegration,
                &busy_lane("merge in progress"),
                Some("change-a")
            ),
            CheckpointDecision::Deferred {
                reason: "merge in progress".to_string()
            }
        );
        // The completed result stays queued so it is not discarded.
        assert_eq!(scheduler.queued_results(), vec!["change-a".to_string()]);
    }

    #[test]
    fn upstream_integration_stale_completion_cannot_release_ownership() {
        let mut scheduler = CheckpointScheduler::new();
        scheduler.request(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None);
        assert_eq!(scheduler.release(0), None);
        assert_eq!(scheduler.release(99), None);
        assert!(scheduler.is_active());
        assert_eq!(scheduler.release(1), Some(Vec::new()));
        // A duplicate release for the same generation is also stale.
        assert_eq!(scheduler.release(1), None);
    }

    #[test]
    fn upstream_integration_release_allows_next_checkpoint_generation() {
        let mut scheduler = CheckpointScheduler::new();
        scheduler.request(
            CheckpointTrigger::PrePushRemoteAdvance,
            &BaseLaneState::clean(),
            None,
        );
        scheduler.release(1);
        assert_eq!(
            scheduler.request(
                CheckpointTrigger::PushRaceRejection,
                &BaseLaneState::clean(),
                None
            ),
            CheckpointDecision::Start { generation: 2 }
        );
    }
}
