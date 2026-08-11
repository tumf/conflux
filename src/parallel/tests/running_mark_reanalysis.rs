//! Scheduler-side behaviour after a settled mark reaches the queue service.
//!
//! Mark stability is a *pre-admission* policy: by the time the scheduler sees
//! anything, all that has happened is an ordinary explicit queue addition. These
//! tests assert exactly that — a settled addition reaches the scheduler as a real
//! `DynamicQueue` candidate and takes the existing queue-addition reanalysis
//! edge, so the ten-second stability interval never stacks with the scheduler's
//! own queue debounce, analysis still runs while a resolve occupies the
//! base-mutating lane and while capacity is zero, and dispatch waits for
//! capacity rather than for another operator action.
//!
//! Everything runs through the production loop step
//! (`evaluate_queued_reanalysis_and_dispatch`) over a real `DynamicQueue` and a
//! real reducer, so the ingestion admission rules are the ones under test rather
//! than a test-only copy. Paused Tokio time throughout: no assertion depends on
//! a wall-clock threshold.

use crate::analyzer::{AnalysisOutcome, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::state::{OrchestratorState, ReducerCommand};
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::dynamic_queue::ReanalysisReason;
use crate::parallel::queue_state::ReanalysisDispatchContext;
use crate::parallel::{ParallelExecutor, WorkspaceResult};
use crate::tui::queue::DynamicQueue;
use crate::vcs::VcsBackend;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::task::JoinSet;

/// The scheduler's ordinary timer branch duration.
const SCHEDULER_TIMER: std::time::Duration = std::time::Duration::from_millis(500);

type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

fn test_config(workspace_base: &std::path::Path) -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        analyze_command: Some("echo analyze".to_string()),
        acceptance_command: Some("echo acceptance".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
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

fn git(repo_root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .expect("run git command");
    assert!(output.status.success(), "git {args:?} failed");
}

/// A minimal repository whose OpenSpec catalog really contains `change_ids`.
///
/// Ingestion loads the candidate from the catalog, so a change that exists only
/// in the reducer would be refused as `candidate_not_found` — which would make
/// every assertion below pass for the wrong reason.
fn repo_with_changes(repo_root: &std::path::Path, change_ids: &[&str]) {
    git(repo_root, &["init", "-b", "main"]);
    git(repo_root, &["config", "user.email", "test@example.com"]);
    git(repo_root, &["config", "user.name", "Test User"]);
    std::fs::write(repo_root.join("README.md"), "base\n").expect("write base file");
    for change_id in change_ids {
        let dir = repo_root.join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&dir).expect("create change dir");
        std::fs::write(dir.join("proposal.md"), format!("# {change_id}\n"))
            .expect("write proposal");
        std::fs::write(dir.join("tasks.md"), "- [ ] work\n").expect("write tasks");
    }
    git(repo_root, &["add", "-A"]);
    git(repo_root, &["commit", "-m", "Base"]);
}

/// Analyzer double that counts analyses and orders whatever it is handed.
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

/// The scheduler-loop state a settled queue addition actually lands in.
struct Harness {
    executor: ParallelExecutor,
    queue: Arc<DynamicQueue>,
    state: Arc<RwLock<OrchestratorState>>,
    queued: Vec<Change>,
    in_flight: HashSet<String>,
    join_set: JoinSet<WorkspaceResult>,
    cleanup_guard: WorkspaceCleanupGuard,
    semaphore: Arc<Semaphore>,
    reanalysis_reason: ReanalysisReason,
    iteration: u32,
    max_parallelism: usize,
    analyses: Arc<AtomicUsize>,
    events: mpsc::Receiver<ExecutionEvent>,
    /// Slots held by an active resolve, as the base-mutating lane holds them.
    resolve_slots: Arc<AtomicUsize>,
    /// Kept alive so the temp directories outlive the executor.
    _repo_dir: TempDir,
    _workspace_base: TempDir,
}

impl Harness {
    /// A scheduler already past its first analysis, with `catalog` loadable.
    fn new(catalog: &[&str], max_parallelism: usize) -> Self {
        let repo_dir = TempDir::new().expect("create repo dir");
        let workspace_base = TempDir::new().expect("create workspace base");
        repo_with_changes(repo_dir.path(), catalog);

        let (tx, events) = mpsc::channel(256);
        let mut executor = ParallelExecutor::new(
            repo_dir.path().to_path_buf(),
            test_config(workspace_base.path()),
            Some(tx),
        );
        let queue = Arc::new(DynamicQueue::new());
        executor.set_dynamic_queue(queue.clone());
        let state = Arc::new(RwLock::new(OrchestratorState::new(
            catalog.iter().map(|id| (*id).to_string()).collect(),
            10,
        )));
        executor.set_shared_orchestrator_state(state.clone());
        let resolve_slots = Arc::new(AtomicUsize::new(0));
        executor.set_manual_resolve_counter(resolve_slots.clone());

        Self {
            executor,
            queue,
            state,
            queued: Vec::new(),
            in_flight: HashSet::new(),
            join_set: JoinSet::new(),
            cleanup_guard: WorkspaceCleanupGuard::new(
                VcsBackend::Git,
                repo_dir.path().to_path_buf(),
            ),
            semaphore: Arc::new(Semaphore::new(max_parallelism)),
            reanalysis_reason: ReanalysisReason::Initial,
            // Iteration 1 unconditionally skips debounce; start where a live
            // scheduler has already run its first analysis.
            iteration: 2,
            max_parallelism,
            analyses: Arc::new(AtomicUsize::new(0)),
            events,
            resolve_slots,
            _repo_dir: repo_dir,
            _workspace_base: workspace_base,
        }
    }

    /// Make the queue debounce window fresh, so a timer wake would be deferred.
    async fn arm_queue_debounce(&self) {
        let mut last_change = self.executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    /// Occupy `count` dispatch slots, as an active resolve does.
    fn hold_resolve_slots(&self, count: usize) {
        self.resolve_slots.store(count, Ordering::SeqCst);
    }

    /// Apply the exact effect stable mark settlement produces.
    ///
    /// Reducer intent first, then the runtime hint — the same order the shared
    /// queue command path uses, and the order ingestion validates against.
    async fn settle_mark_addition(&self, change_id: &str) {
        self.state
            .write()
            .await
            .apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
        assert!(
            self.queue.push(change_id.to_string()).await,
            "settlement must produce a real DynamicQueue addition"
        );
    }

    /// One scheduler loop pass: ingest the queue, then evaluate and dispatch.
    async fn run_loop_iteration<F>(&mut self, analyzer: &F) -> Option<(bool, u32)>
    where
        for<'a> F: Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync,
    {
        let in_flight = self.in_flight.clone();
        self.executor
            .check_dynamic_queue_and_add_changes(
                &mut self.queued,
                &in_flight,
                &mut self.reanalysis_reason,
            )
            .await;

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
                    work_snapshot: None,
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

    /// The scheduler's plain 500 ms timer branch, contributing no new reason.
    async fn timer_wake(&self) {
        tokio::time::sleep(SCHEDULER_TIMER).await;
    }

    fn analyses(&self) -> usize {
        self.analyses.load(Ordering::SeqCst)
    }

    /// Drain the event channel, counting the two events under test.
    fn drain_events(&mut self) -> (usize, usize) {
        let (mut analysis_started, mut apply_started) = (0, 0);
        while let Ok(event) = self.events.try_recv() {
            match event {
                ExecutionEvent::AnalysisStarted { .. } => analysis_started += 1,
                ExecutionEvent::ApplyStarted { .. } => apply_started += 1,
                _ => {}
            }
        }
        (analysis_started, apply_started)
    }

    async fn shutdown(mut self) {
        self.join_set.abort_all();
        while self.join_set.join_next().await.is_some() {}
    }
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_settled_addition_analyzes_during_active_resolve() {
    // One slot held by an active resolve, one left for ordinary dispatch.
    let mut harness = Harness::new(&["beta"], 2);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;
    harness.hold_resolve_slots(1);

    harness.settle_mark_addition("beta").await;
    harness.run_loop_iteration(&analyzer).await;

    let (analysis_started, apply_started) = harness.drain_events();
    assert_eq!(
        harness.analyses(),
        1,
        "a settled queue addition must start analysis without another debounce period"
    );
    assert_eq!(
        analysis_started, 1,
        "the addition takes the existing queue-addition reanalysis edge"
    );
    assert_eq!(
        harness.resolve_slots.load(Ordering::SeqCst),
        1,
        "the resolve must still be active when analysis started"
    );
    assert_eq!(
        apply_started, 0,
        "an apply event is published by the spawned task, not by the dispatch decision"
    );
    // Dispatch is asserted from the loop's own synchronous state rather than
    // from an event a spawned task will publish later: admission is what this
    // test is about, and a race on the event channel would make it flaky.
    assert_eq!(
        harness.in_flight.len(),
        1,
        "recomputed capacity of one admits ordinary dispatch alongside the resolve"
    );
    assert!(harness.queued.is_empty());

    harness.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_settled_addition_analyzes_at_zero_capacity() {
    // Every slot held by resolve/manual work: analysis must still run, dispatch
    // must not.
    let mut harness = Harness::new(&["beta"], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;
    harness.hold_resolve_slots(1);

    harness.settle_mark_addition("beta").await;
    harness.run_loop_iteration(&analyzer).await;

    let (analysis_started, apply_started) = harness.drain_events();
    assert_eq!(
        analysis_started, 1,
        "zero capacity must not suppress analysis"
    );
    assert_eq!(
        apply_started, 0,
        "zero capacity must suppress ordinary apply dispatch"
    );
    assert!(harness.in_flight.is_empty());
    assert!(
        harness.join_set.is_empty(),
        "no workspace task may be spawned at zero capacity"
    );
    assert_eq!(
        harness.queued.len(),
        1,
        "the queued candidate is retained until capacity recovers"
    );

    harness.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_capacity_recovery_dispatches_without_another_operator_action() {
    let mut harness = Harness::new(&["beta"], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;
    harness.hold_resolve_slots(1);

    harness.settle_mark_addition("beta").await;
    harness.run_loop_iteration(&analyzer).await;
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }
    let (_, apply_started) = harness.drain_events();
    assert_eq!(
        apply_started, 0,
        "dispatch stays suppressed while capacity is zero"
    );
    assert!(harness.in_flight.is_empty());

    // The resolve completes and the scheduler wakes itself with its own edge. No
    // second mark, no second settlement, and no Start action is involved.
    harness.hold_resolve_slots(0);
    harness.reanalysis_reason = ReanalysisReason::ResolveCompletion;
    let outcome = harness
        .run_loop_iteration(&analyzer)
        .await
        .expect("queued work must be evaluated after capacity recovery");

    assert!(!outcome.0, "capacity recovery must resume the scheduler");
    assert_eq!(
        harness.in_flight.len(),
        1,
        "the recovered slot must dispatch the settled candidate"
    );
    assert!(
        harness.queued.is_empty(),
        "the settled candidate leaves the queued set once dispatched"
    );
    assert!(
        harness.queue.pop().await.is_none(),
        "the DynamicQueue addition was consumed exactly once"
    );

    harness.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_unsettled_mark_creates_no_analysis_edge() {
    // The negative half of the contract: without a real queue addition there is
    // nothing for the scheduler to see, so a pending mark must not make it poll.
    let mut harness = Harness::new(&["beta"], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    // Marked but unsettled: no reducer intent, no DynamicQueue entry.
    harness.queued.push(test_change("beta"));
    for _ in 0..4 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    let (analysis_started, apply_started) = harness.drain_events();
    assert_eq!(
        analysis_started, 0,
        "a fresh debounce window still defers timer-driven reanalysis"
    );
    assert_eq!(apply_started, 0);
    assert_eq!(harness.analyses(), 0);

    harness.shutdown().await;
}
