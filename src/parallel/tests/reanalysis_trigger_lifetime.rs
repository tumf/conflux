//! Loop-level regression tests for edge-triggered re-analysis trigger lifetime.
//!
//! `ResolveCompletion`, `RepairCandidate`, and `SlotRecovery` represent scheduler
//! state-transition edges and bypass queue debounce. They must be consumed after the
//! scheduler loop actually evaluates queued work with them, so that plain 500 ms timer
//! wakes cannot replay a consumed edge and restart expensive dependency analysis on
//! every scheduler tick.
//!
//! These tests drive `evaluate_queued_reanalysis_and_dispatch`, the loop-owned
//! transition step that `execute_with_order_based_reanalysis` calls, so the trigger
//! lifetime under test is the production code path rather than a test-only copy.

use crate::analyzer::{AnalysisOutcome, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::dynamic_queue::ReanalysisReason;
use crate::parallel::queue_state::ReanalysisDispatchContext;
use crate::parallel::{
    MergeResult, MergeResultOrigin, MergeTaskOutcome, ParallelExecutor, WorkspaceResult,
};
use crate::vcs::VcsBackend;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// The scheduler's ordinary debounce timer branch duration.
const SCHEDULER_TIMER: std::time::Duration = std::time::Duration::from_millis(500);

fn create_test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        analyze_command: Some("echo analyze".to_string()),
        acceptance_command: Some("echo acceptance".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        ..Default::default()
    }
}

fn test_change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: String::new(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

fn init_minimal_git_repo(repo_root: &std::path::Path) {
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test User"],
    ] {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git setup command");
        assert!(output.status.success(), "git setup command failed");
    }
    std::fs::write(repo_root.join("README.md"), "base\n").expect("write base file");
    for args in [vec!["add", "-A"], vec!["commit", "-m", "Base"]] {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git commit command");
        assert!(output.status.success(), "git commit command failed");
    }
}

/// The analyzer callback's return type, as required by the scheduler loop.
type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

/// Analyzer test double that counts how many dependency analyses were actually started.
fn counting_analyzer(
    invocations: Arc<AtomicUsize>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        invocations.fetch_add(1, Ordering::SeqCst);
        let order: Vec<String> = changes.iter().map(|change| change.id.clone()).collect();
        Box::pin(async move {
            AnalysisResult {
                order,
                dependencies: HashMap::new(),
                groups: None,
            }
            .into()
        })
    }
}

/// Deterministic harness that owns exactly the scheduler-loop state relevant to
/// re-analysis trigger lifetime and replays it through the production loop step.
struct SchedulerLoopHarness {
    executor: ParallelExecutor,
    queued: Vec<Change>,
    in_flight: HashSet<String>,
    join_set: JoinSet<WorkspaceResult>,
    cleanup_guard: WorkspaceCleanupGuard,
    semaphore: Arc<Semaphore>,
    reanalysis_reason: ReanalysisReason,
    iteration: u32,
    max_parallelism: usize,
    analysis_invocations: Arc<AtomicUsize>,
    events: mpsc::Receiver<ExecutionEvent>,
}

impl SchedulerLoopHarness {
    fn new(repo_root: std::path::PathBuf, queued: Vec<Change>) -> Self {
        Self::with_config(repo_root, queued, create_test_config())
    }

    fn with_config(
        repo_root: std::path::PathBuf,
        queued: Vec<Change>,
        config: OrchestratorConfig,
    ) -> Self {
        let (tx, events) = mpsc::channel(256);
        let executor = ParallelExecutor::new(repo_root.clone(), config, Some(tx));
        Self {
            executor,
            queued,
            in_flight: HashSet::new(),
            join_set: JoinSet::new(),
            cleanup_guard: WorkspaceCleanupGuard::new(VcsBackend::Git, repo_root),
            semaphore: Arc::new(Semaphore::new(1)),
            reanalysis_reason: ReanalysisReason::Initial,
            // Iteration 1 unconditionally skips debounce, so trigger-lifetime tests
            // start where the live scheduler already ran its first analysis.
            iteration: 2,
            max_parallelism: 1,
            analysis_invocations: Arc::new(AtomicUsize::new(0)),
            events,
        }
    }

    /// Arm the queue debounce window so non-bypass reasons are actually debounceable.
    /// This mirrors a scheduler that has recently ingested queued work.
    async fn arm_queue_debounce(&self) {
        let mut last_change = self.executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    /// Occupy all dispatch capacity, as an active manual resolve does.
    fn occupy_all_capacity(&mut self) -> Arc<AtomicUsize> {
        let counter = Arc::new(AtomicUsize::new(self.max_parallelism));
        self.executor.set_manual_resolve_counter(counter.clone());
        counter
    }

    /// One scheduler loop iteration's queued re-analysis/dispatch evaluation, including
    /// the loop-owned consumption of one-shot edge triggers.
    async fn run_loop_iteration<F>(&mut self, analyzer: &F) -> Option<(bool, u32)>
    where
        for<'a> F: Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync,
    {
        let outcome = self
            .executor
            .evaluate_queued_reanalysis_and_dispatch(
                ReanalysisDispatchContext {
                    queued: &mut self.queued,
                    in_flight: &mut self.in_flight,
                    max_parallelism: self.max_parallelism,
                    iteration: self.iteration,
                    reanalysis_reason: self.reanalysis_reason,
                    analyzer,
                    semaphore: self.semaphore.clone(),
                    join_set: &mut self.join_set,
                    cleanup_guard: &mut self.cleanup_guard,
                },
                &mut self.reanalysis_reason,
            )
            .await
            .expect("scheduler re-analysis evaluation should not fail");

        if let Some((_, new_iteration)) = outcome {
            self.iteration = new_iteration;
        }
        outcome
    }

    /// The scheduler's plain 500 ms timer branch: it wakes the loop and contributes no
    /// new re-analysis reason.
    async fn timer_wake(&self) {
        tokio::time::sleep(SCHEDULER_TIMER).await;
    }

    fn deliver_edge(&mut self, reason: ReanalysisReason) {
        self.reanalysis_reason = reason;
    }

    fn analyses(&self) -> usize {
        self.analysis_invocations.load(Ordering::SeqCst)
    }

    fn drain_apply_started(&mut self) -> usize {
        let mut apply_started = 0;
        while let Ok(event) = self.events.try_recv() {
            if matches!(event, ExecutionEvent::ApplyStarted { .. }) {
                apply_started += 1;
            }
        }
        apply_started
    }
}

#[test]
fn only_edge_triggered_reasons_are_one_shot() {
    for reason in [
        ReanalysisReason::ResolveCompletion,
        ReanalysisReason::SlotRecovery,
        ReanalysisReason::RepairCandidate,
    ] {
        assert!(
            reason.is_one_shot_edge_trigger(),
            "{reason} represents a state-transition edge and must be consumed once"
        );
    }

    for reason in [
        ReanalysisReason::Initial,
        ReanalysisReason::Completion,
        // Queue notification keeps its existing candidate-addition reconciliation and
        // reset semantics in the scheduler loop.
        ReanalysisReason::QueueNotification,
    ] {
        assert!(
            !reason.is_one_shot_edge_trigger(),
            "{reason} must keep its existing loop lifetime"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn completion_edge_analyzes_once_and_timer_wakes_do_not_replay_it() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SchedulerLoopHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-b")]);
    let analyzer = counting_analyzer(harness.analysis_invocations.clone());
    harness.arm_queue_debounce().await;
    let resolve_counter = harness.occupy_all_capacity();

    // A real resolve/workspace/merge completion edge, as delivered by
    // `wait_for_scheduler_event`.
    harness.deliver_edge(ReanalysisReason::ResolveCompletion);
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        1,
        "a completion edge must run exactly one immediate analysis"
    );
    assert_eq!(
        harness.reanalysis_reason,
        ReanalysisReason::Initial,
        "an evaluated edge trigger must be consumed back to the non-bypass state"
    );

    // Nothing changes: capacity stays at zero, the queued set stays identical, and only
    // the ordinary 500 ms timer fires.
    for _ in 0..5 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    assert_eq!(
        resolve_counter.load(Ordering::SeqCst),
        1,
        "test must hold dispatch capacity at zero for the whole scenario"
    );
    assert_eq!(
        harness.analyses(),
        1,
        "timer wakes must not replay a consumed completion edge; saw {} analyses",
        harness.analyses()
    );
    assert_eq!(
        harness.queued.len(),
        1,
        "queued work must be retained while capacity is zero"
    );
    assert!(
        harness.in_flight.is_empty(),
        "zero capacity must suppress ordinary apply dispatch"
    );
    assert!(
        harness.join_set.is_empty(),
        "no workspace task should be spawned at zero capacity"
    );
    assert_eq!(
        harness.drain_apply_started(),
        0,
        "no apply should start while capacity is zero"
    );
}

#[tokio::test(start_paused = true)]
async fn repair_candidate_edge_bypasses_debounce_once_only() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness = SchedulerLoopHarness::new(
        temp_dir.path().to_path_buf(),
        vec![test_change("repair-candidate")],
    );
    let analyzer = counting_analyzer(harness.analysis_invocations.clone());
    harness.arm_queue_debounce().await;
    harness.occupy_all_capacity();

    harness.deliver_edge(ReanalysisReason::RepairCandidate);
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        1,
        "a repair-candidate edge must bypass debounce once"
    );

    for _ in 0..4 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    assert_eq!(
        harness.analyses(),
        1,
        "timer wakes must not replay a consumed repair-candidate edge"
    );
    assert_eq!(harness.reanalysis_reason, ReanalysisReason::Initial);
}

#[tokio::test(start_paused = true)]
async fn slot_recovery_edge_bypasses_debounce_once_only() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness = SchedulerLoopHarness::new(
        temp_dir.path().to_path_buf(),
        vec![test_change("slot-recovery-candidate")],
    );
    let analyzer = counting_analyzer(harness.analysis_invocations.clone());
    harness.arm_queue_debounce().await;
    harness.occupy_all_capacity();

    harness.deliver_edge(ReanalysisReason::SlotRecovery);
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        1,
        "a slot-recovery edge must bypass debounce once"
    );

    for _ in 0..4 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    assert_eq!(
        harness.analyses(),
        1,
        "timer wakes must not replay a consumed slot-recovery edge"
    );
    assert_eq!(harness.reanalysis_reason, ReanalysisReason::Initial);
}

#[tokio::test(start_paused = true)]
async fn queued_empty_loop_does_not_consume_an_unevaluated_edge() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness = SchedulerLoopHarness::new(temp_dir.path().to_path_buf(), Vec::new());
    let analyzer = counting_analyzer(harness.analysis_invocations.clone());
    harness.arm_queue_debounce().await;
    harness.occupy_all_capacity();

    harness.deliver_edge(ReanalysisReason::ResolveCompletion);
    let outcome = harness.run_loop_iteration(&analyzer).await;

    assert!(
        outcome.is_none(),
        "an empty queue performs no re-analysis/dispatch evaluation"
    );
    assert_eq!(
        harness.analyses(),
        0,
        "no queued work means no dependency analysis"
    );
    assert_eq!(
        harness.reanalysis_reason,
        ReanalysisReason::ResolveCompletion,
        "an edge that was never evaluated must stay armed instead of being discarded"
    );

    // Queued work becomes available on a later iteration (queue ingestion or
    // reducer-visible reconciliation); the retained edge is only consumed now.
    harness.queued.push(test_change("late-candidate"));
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        1,
        "the retained edge must still be able to trigger its immediate analysis"
    );
    assert_eq!(harness.reanalysis_reason, ReanalysisReason::Initial);

    // A later explicit queue/reconciliation event must not be suppressed by the reset.
    harness.deliver_edge(ReanalysisReason::QueueNotification);
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        2,
        "queue notification keeps its existing debounce-bypass behavior after the reset"
    );
    assert_eq!(
        harness.queued.len(),
        1,
        "queued work stays pending while capacity is zero"
    );
}

#[tokio::test(start_paused = true)]
async fn second_completion_edge_rearms_analysis_and_capacity_recovery_dispatches() {
    let repo_dir = TempDir::new().unwrap();
    let workspace_base = TempDir::new().unwrap();
    init_minimal_git_repo(repo_dir.path());

    let config = OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..create_test_config()
    };
    let mut harness = SchedulerLoopHarness::with_config(
        repo_dir.path().to_path_buf(),
        vec![test_change("queued-b")],
        config,
    );
    let analyzer = counting_analyzer(harness.analysis_invocations.clone());
    harness.arm_queue_debounce().await;
    let resolve_counter = harness.occupy_all_capacity();

    harness.deliver_edge(ReanalysisReason::ResolveCompletion);
    harness.run_loop_iteration(&analyzer).await;
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }
    assert_eq!(
        harness.analyses(),
        1,
        "the first edge must be consumed exactly once"
    );
    assert!(
        harness.in_flight.is_empty(),
        "dispatch stays suppressed while capacity is zero"
    );

    // A later completion releases the occupied slot and wakes the scheduler with a new
    // edge; no queue addition or user action is involved.
    resolve_counter.store(0, Ordering::SeqCst);
    harness.deliver_edge(ReanalysisReason::ResolveCompletion);
    let (should_break, iteration) = harness
        .run_loop_iteration(&analyzer)
        .await
        .expect("queued work must be evaluated");

    assert!(!should_break, "capacity recovery must resume the scheduler");
    assert_eq!(
        harness.analyses(),
        2,
        "a second real edge must re-arm immediate analysis"
    );
    assert_eq!(
        iteration, 3,
        "recovered capacity must advance a dispatch iteration"
    );
    assert!(
        harness.queued.is_empty(),
        "eligible queued work must be dispatched after capacity recovery"
    );
    assert_eq!(
        harness.in_flight.len(),
        1,
        "the recovered slot must be used by the queued change"
    );
    assert_eq!(
        harness.reanalysis_reason,
        ReanalysisReason::Initial,
        "the second edge is consumed too"
    );

    harness.join_set.abort_all();
    while harness.join_set.join_next().await.is_some() {}
}

/// Capacity-recovery audit coverage for background base-lane work.
///
/// Every background merge outcome decrements the scheduler-accounted pending merge
/// counter, so occupancy is released on merged, deferred, and failed paths alike. Only
/// a merged outcome arms an immediate `ResolveCompletion` edge; the non-terminal
/// outcomes fall back to the bounded timer/debounce evaluation instead of relying on a
/// sticky trigger from an earlier edge.
#[tokio::test]
async fn every_background_merge_outcome_releases_capacity_and_only_merged_arms_an_edge() {
    let cases: Vec<(&str, MergeTaskOutcome, ReanalysisReason)> = vec![
        (
            "merged",
            MergeTaskOutcome::Merged,
            ReanalysisReason::ResolveCompletion,
        ),
        (
            "deferred",
            MergeTaskOutcome::deferred("merge lane busy", true),
            ReanalysisReason::Initial,
        ),
        (
            "resolve exhausted",
            MergeTaskOutcome::resolve_exhausted(
                "change-a",
                3,
                crate::parallel::ResolveFailureClassification::UnresolvedConflict,
                "conflicts remain",
            ),
            ReanalysisReason::Initial,
        ),
        (
            "already reported",
            MergeTaskOutcome::already_reported(
                "change-a",
                crate::parallel::AlreadyReportedFailureKind::Push,
                "push already reported",
            ),
            ReanalysisReason::Initial,
        ),
        (
            "run fatal",
            MergeTaskOutcome::run_fatal("background merge failed"),
            ReanalysisReason::Initial,
        ),
    ];

    for (label, outcome, expected_reason) in cases {
        let temp_dir = TempDir::new().unwrap();
        let (tx, _rx) = mpsc::channel(32);
        let mut executor = ParallelExecutor::new(
            temp_dir.path().to_path_buf(),
            create_test_config(),
            Some(tx),
        );

        let (merge_result_tx, mut merge_result_rx) = mpsc::channel(8);
        executor
            .pending_merge_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        merge_result_tx
            .send(MergeResult {
                change_id: "change-a".to_string(),
                workspace_name: "ws-change-a".to_string(),
                origin: MergeResultOrigin::PostArchiveMerge,
                outcome,
            })
            .await
            .expect("merge result channel should accept the outcome");

        let mut reanalysis_reason = ReanalysisReason::Initial;
        executor
            .wait_for_persistent_idle_wake_with_tx(
                &mut reanalysis_reason,
                &merge_result_tx,
                &mut merge_result_rx,
            )
            .await;

        assert_eq!(
            executor
                .pending_merge_count
                .load(std::sync::atomic::Ordering::Relaxed),
            0,
            "{label} outcome must release its scheduler-accounted merge occupancy"
        );
        assert_eq!(
            reanalysis_reason, expected_reason,
            "{label} outcome produced an unexpected scheduler wake reason"
        );
    }
}
