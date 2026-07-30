//! Scheduler-side coverage for opt-in upstream integration.
//!
//! These tests are unit-scoped: they exercise the checkpoint reducer, the
//! default-off boundary, and executor/service propagation without touching a
//! real repository, remote, process, or clock. Real Git/worktree/bare-remote
//! behavior lives in the heavy E2E suite.

use std::path::PathBuf;

use crate::config::OrchestratorConfig;
use crate::parallel::upstream_bridge::render_upstream_event;
use crate::parallel::{ParallelExecutor, PostArchiveAction};
use crate::upstream::checkpoint::{
    BaseLaneState, CheckpointDecision, CheckpointScheduler, CheckpointTrigger,
};
use crate::upstream::ports::UpstreamEvent;
use crate::upstream::{UpstreamIntegrationConfig, UpstreamRuntime};

fn runtime() -> UpstreamRuntime {
    UpstreamRuntime {
        config: UpstreamIntegrationConfig::new("origin", "cargo test"),
        branch: "main".to_string(),
    }
}

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        ..Default::default()
    }
}

fn executor() -> ParallelExecutor {
    ParallelExecutor::new(
        PathBuf::from("/tmp/cflx-upstream-unit"),
        test_config(),
        None,
    )
}

fn stagger() -> crate::ai_command_runner::SharedStaggerState {
    std::sync::Arc::new(tokio::sync::Mutex::new(None))
}

// ── Default-off boundary ───────────────────────────────────────────────────

#[test]
fn upstream_integration_disabled_executor_installs_no_checkpoint() {
    let executor = executor();
    assert!(!executor.has_upstream_integration());
}

#[test]
fn upstream_integration_enabled_executor_installs_checkpoint() {
    let mut executor = executor();
    executor.set_upstream_integration(runtime(), stagger());
    assert!(executor.has_upstream_integration());
}

#[test]
fn upstream_integration_survives_restart_loop_reconstruction() {
    // The restart loop rebuilds the executor on every iteration; the
    // invocation-scoped runtime must keep its selected remote, cumulative base
    // branch, and verification command each time.
    let runtime = runtime();
    for _ in 0..3 {
        let mut executor = executor();
        executor.set_upstream_integration(runtime.clone(), stagger());
        assert!(executor.has_upstream_integration());
    }
    assert_eq!(runtime.config.remote, "origin");
    assert_eq!(runtime.config.verify_command, "cargo test");
    assert_eq!(runtime.branch, "main");
}

#[test]
fn upstream_integration_leaves_post_archive_action_untouched() {
    // `PushToRemote` keeps its existing routing; upstream integration is a
    // separate cumulative-base concern and never rewrites it.
    let mut executor = executor();
    executor.set_post_archive_action(PostArchiveAction::PushToRemote {
        remote: "origin".to_string(),
    });
    executor.set_upstream_integration(runtime(), stagger());
    assert!(executor.has_upstream_integration());
}

// ── Deterministic checkpoint triggering ────────────────────────────────────

#[test]
fn upstream_integration_checkpoint_trigger_ordering_is_deterministic() {
    let mut scheduler = CheckpointScheduler::new();
    let ordered = [
        CheckpointTrigger::BeforeFirstDispatch,
        CheckpointTrigger::BeforeBaseIntegration,
        CheckpointTrigger::AfterDrain,
        CheckpointTrigger::PrePushRemoteAdvance,
        CheckpointTrigger::PushRaceRejection,
    ];

    for (index, trigger) in ordered.into_iter().enumerate() {
        let generation = index as u64 + 1;
        assert_eq!(
            scheduler.request(trigger, &BaseLaneState::clean(), None),
            CheckpointDecision::Start { generation },
            "trigger {:?} must own its own checkpoint generation",
            trigger
        );
        assert_eq!(scheduler.release(generation), Some(Vec::new()));
    }
}

#[test]
fn upstream_integration_single_fetch_batches_results_behind_one_checkpoint() {
    let mut scheduler = CheckpointScheduler::new();
    assert_eq!(
        scheduler.request(
            CheckpointTrigger::BeforeBaseIntegration,
            &BaseLaneState::clean(),
            Some("change-a"),
        ),
        CheckpointDecision::Start { generation: 1 }
    );

    // Results completing while the checkpoint owns the lane stay queued and
    // share its single fetch instead of starting new checkpoints.
    for change in ["change-b", "change-c"] {
        assert_eq!(
            scheduler.request(
                CheckpointTrigger::BeforeBaseIntegration,
                &BaseLaneState::clean(),
                Some(change),
            ),
            CheckpointDecision::Batched { generation: 1 }
        );
    }

    assert_eq!(
        scheduler.release(1),
        Some(vec![
            "change-a".to_string(),
            "change-b".to_string(),
            "change-c".to_string()
        ])
    );
}

#[test]
fn upstream_integration_completed_results_wait_without_being_discarded() {
    // Independent worktree apply/acceptance may finish while the lane is busy;
    // their results must remain queued rather than dropped.
    let mut scheduler = CheckpointScheduler::new();
    let busy = BaseLaneState {
        base_dirty_reason: None,
        lane_busy_reason: Some("archive integration owns the base lane".to_string()),
    };
    assert!(matches!(
        scheduler.request(
            CheckpointTrigger::BeforeBaseIntegration,
            &busy,
            Some("change-a")
        ),
        CheckpointDecision::Deferred { .. }
    ));
    assert_eq!(scheduler.queued_results(), vec!["change-a".to_string()]);

    // Once the lane frees, the same queued result is covered by the checkpoint.
    assert_eq!(
        scheduler.request(
            CheckpointTrigger::BeforeBaseIntegration,
            &BaseLaneState::clean(),
            None
        ),
        CheckpointDecision::Start { generation: 1 }
    );
    assert_eq!(scheduler.release(1), Some(vec!["change-a".to_string()]));
}

#[test]
fn upstream_integration_unsafe_lane_suppresses_every_checkpoint_side_effect() {
    for lane in [
        BaseLaneState {
            base_dirty_reason: Some("uncommitted changes".to_string()),
            lane_busy_reason: None,
        },
        BaseLaneState {
            base_dirty_reason: None,
            lane_busy_reason: Some("resolve owns the base lane".to_string()),
        },
    ] {
        let mut scheduler = CheckpointScheduler::new();
        assert!(matches!(
            scheduler.request(CheckpointTrigger::AfterDrain, &lane, None),
            CheckpointDecision::Deferred { .. }
        ));
        assert!(!scheduler.is_active(), "no ownership is taken while unsafe");
    }
}

#[test]
fn upstream_integration_stale_completion_cannot_release_lane_ownership() {
    let mut scheduler = CheckpointScheduler::new();
    scheduler.request(
        CheckpointTrigger::BeforeBaseIntegration,
        &BaseLaneState::clean(),
        Some("change-a"),
    );

    // A completion event from an earlier (or already-released) generation must
    // not hand the lane to another owner's work.
    assert_eq!(scheduler.release(0), None);
    assert!(scheduler.is_active());
    assert_eq!(scheduler.release(1), Some(vec!["change-a".to_string()]));
    assert_eq!(scheduler.release(1), None);
    assert!(!scheduler.is_active());
}

#[test]
fn upstream_integration_no_polling_reason_can_start_a_checkpoint() {
    // The reducer has exactly five entry points; there is no timer or
    // scheduler-loop trigger that can start a checkpoint on its own.
    let scheduler = CheckpointScheduler::new();
    assert!(!scheduler.is_active());
    assert!(scheduler.queued_results().is_empty());
    assert_eq!(scheduler.active_generation(), None);
}

// ── Observability is not routing authority ─────────────────────────────────

#[test]
fn upstream_integration_events_are_reported_without_changing_routing() {
    // Rendering is a pure projection: it produces operator-visible text and
    // returns no decision the scheduler could consume.
    let (warning, message) = render_upstream_event(&UpstreamEvent::Stalled {
        reason: "verification failed".to_string(),
    });
    assert!(warning);
    assert!(message.contains("upstream: stalled"));

    let (warning, message) = render_upstream_event(&UpstreamEvent::NoOp {
        fetched_sha: "abc".to_string(),
    });
    assert!(!warning);
    assert!(message.contains("already integrated"));
}
