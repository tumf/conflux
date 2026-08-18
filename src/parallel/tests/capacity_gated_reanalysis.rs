//! Scheduler behaviour when dispatch capacity is exhausted.
//!
//! The expensive dependency analyzer is a *dispatch* input. Running it with zero
//! free slots produces an order nothing can consume, and — worse — records that
//! the input was analysed, which is what previously let a full scheduler swallow
//! the queue, completion, repair, and slot-recovery edges that were still
//! waiting for capacity to come back.
//!
//! So the gate is: classification, reducer-visible queued-intent reconciliation,
//! and operator diagnostics stay available at zero slots; the analyzer and
//! ordinary dispatch do not. A gated evaluation records no signature and
//! consumes no edge, so the recovery wake still has something to act on.
//!
//! Everything runs through the production loop step
//! (`evaluate_queued_reanalysis_and_dispatch`) over a real `DynamicQueue` and a
//! real reducer. Paused Tokio time throughout: no assertion depends on a
//! wall-clock threshold.
//!
//! Evidence class: integration. The harness initialises a real Git repository
//! and a real workspace directory because queue classification reads them, so
//! these tests exercise real filesystem and process boundaries rather than
//! in-memory doubles.

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

    /// Publish reducer queue intent and the matching runtime hint.
    async fn queue_change(&self, change_id: &str) {
        self.state
            .write()
            .await
            .apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
        assert!(
            self.queue.push(change_id.to_string()).await,
            "the queue addition must be real"
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

    /// Drain the event channel, returning analysis starts and diagnostic logs.
    fn drain_events(&mut self) -> (usize, Vec<String>) {
        let (mut analysis_started, mut logs) = (0, Vec::new());
        while let Ok(event) = self.events.try_recv() {
            match event {
                ExecutionEvent::AnalysisStarted { .. } => analysis_started += 1,
                ExecutionEvent::Log(entry) => logs.push(entry.message),
                _ => {}
            }
        }
        (analysis_started, logs)
    }

    /// The signature/suppression state a gated evaluation must leave untouched.
    fn records_no_suppression(&self) -> bool {
        self.executor.last_completed_analysis_input.is_none()
            && self.executor.next_analysis_signature_probe_at.is_none()
            && self.executor.analysis_retry_throttle.is_none()
    }

    async fn shutdown(mut self) {
        self.join_set.abort_all();
        while self.join_set.join_next().await.is_some() {}
    }
}

#[tokio::test(start_paused = true)]
async fn reanalysis_zero_capacity_gates_analyzer() {
    // Every slot held by the base-mutating lane: classification and diagnostics
    // must still run, the analyzer and dispatch must not.
    let mut harness = Harness::new(&["beta"], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;
    harness.hold_resolve_slots(1);

    harness.queue_change("beta").await;
    harness.run_loop_iteration(&analyzer).await;

    let (analysis_started, logs) = harness.drain_events();
    assert_eq!(
        harness.analyses(),
        0,
        "zero capacity must not start the expensive dependency analyzer"
    );
    assert_eq!(
        analysis_started, 0,
        "a suppressed evaluation is not a distinct analysis attempt"
    );
    assert!(
        harness.in_flight.is_empty(),
        "zero capacity must suppress ordinary apply dispatch"
    );
    assert!(
        harness.join_set.is_empty(),
        "no workspace task may be spawned at zero capacity"
    );
    assert_eq!(
        harness.queued.len(),
        1,
        "the queued candidate is retained until capacity recovers"
    );
    assert!(
        logs.iter()
            .any(|message| message.contains("analysis_capacity_zero")),
        "the reason must be operator-visible: {logs:?}"
    );

    harness.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn reanalysis_zero_capacity_preserves_edge_and_signature() {
    let mut harness = Harness::new(&["beta"], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;
    harness.hold_resolve_slots(1);

    // Ingest first, so the queue hint is already drained and the one-shot edge
    // below is the only trigger the gated evaluation is carrying.
    harness.queue_change("beta").await;
    harness.run_loop_iteration(&analyzer).await;
    assert!(harness.queue.pop().await.is_none());

    // An explicit one-shot state-transition edge, arriving with no capacity.
    harness.reanalysis_reason = ReanalysisReason::ResolveCompletion;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(harness.analyses(), 0);
    assert_eq!(
        harness.reanalysis_reason,
        ReanalysisReason::ResolveCompletion,
        "an edge that never reached the analyzer must stay unconsumed"
    );
    assert!(
        harness.records_no_suppression(),
        "an input the analyzer never saw must leave no completed or suppression record"
    );

    // Repeated gated wakes must stay just as inert: no analysis, no record, and
    // the edge still intact.
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }
    assert_eq!(harness.analyses(), 0);
    assert_eq!(
        harness.reanalysis_reason,
        ReanalysisReason::ResolveCompletion
    );
    assert!(harness.records_no_suppression());

    let (analysis_started, logs) = harness.drain_events();
    assert_eq!(analysis_started, 0);
    let capacity_diagnostics: Vec<_> = logs
        .iter()
        .filter(|message| message.contains("analysis_capacity_zero"))
        .collect();
    assert_eq!(
        capacity_diagnostics.len(),
        1,
        "identical gated wakes report one deduplicated operator-visible reason: {capacity_diagnostics:?}"
    );

    harness.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn reanalysis_slot_recovery_after_capacity_gate_resumes_from_the_preserved_edge() {
    let mut harness = Harness::new(&["beta"], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;
    harness.hold_resolve_slots(1);

    harness.queue_change("beta").await;
    harness.run_loop_iteration(&analyzer).await;
    harness.reanalysis_reason = ReanalysisReason::ResolveCompletion;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 0);

    // The resolve finishes. No new queue mutation and no second operator action:
    // the edge preserved by the gate is the whole liveness signal.
    harness.hold_resolve_slots(0);
    let outcome = harness
        .run_loop_iteration(&analyzer)
        .await
        .expect("queued work must be evaluated after capacity recovery");

    assert!(!outcome.0, "capacity recovery must resume the scheduler");
    assert_eq!(harness.analyses(), 1);
    assert_eq!(
        harness.in_flight.len(),
        1,
        "the recovered slot dispatches the queued candidate"
    );
    assert!(harness.queued.is_empty());
    assert_eq!(
        harness.reanalysis_reason,
        ReanalysisReason::Initial,
        "the edge is consumed by the evaluation that actually used it"
    );

    harness.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn reanalysis_slot_recovery_after_capacity_gate_follows_in_flight_completion() {
    // Capacity recovered by a finished apply rather than by a resolve, and with
    // no explicit edge left over: the queue trigger promoted to `SlotRecovery`
    // is what re-evaluates the remaining work.
    let mut harness = Harness::new(&["beta"], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;
    harness.in_flight.insert("alpha".to_string());

    harness.queue_change("beta").await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 0, "the pass ran with no free slot");
    assert!(harness.records_no_suppression());

    // The in-flight apply completes.
    harness.in_flight.remove("alpha");
    harness.reanalysis_reason = ReanalysisReason::QueueNotification;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        1,
        "recovered capacity re-evaluates work"
    );
    assert_eq!(harness.in_flight.len(), 1);
    assert!(harness.queued.is_empty());

    harness.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn reanalysis_empty_or_noop_suppressed() {
    // An empty eligible queue never starts Analyze, however the loop was woken.
    let mut harness = Harness::new(&["beta"], 2);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    for reason in [
        ReanalysisReason::Initial,
        ReanalysisReason::QueueNotification,
        ReanalysisReason::ResolveCompletion,
        ReanalysisReason::SlotRecovery,
    ] {
        harness.reanalysis_reason = reason;
        assert!(
            harness.run_loop_iteration(&analyzer).await.is_none(),
            "an empty eligible queue must produce no evaluation for {reason}"
        );
    }

    let (analysis_started, _) = harness.drain_events();
    assert_eq!(harness.analyses(), 0);
    assert_eq!(analysis_started, 0);
    assert!(harness.join_set.is_empty());

    // A settled batch that applied nothing notifies no scheduler, so the loop
    // sees only its ordinary timer inside a fresh debounce window. That must not
    // manufacture an analysis attempt either.
    harness.reanalysis_reason = ReanalysisReason::Initial;
    harness.queued.push(test_change("beta"));
    for _ in 0..4 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    let (analysis_started, _) = harness.drain_events();
    assert_eq!(
        harness.analyses(),
        0,
        "a no-op batch adds no queue edge, so the fresh debounce window still defers"
    );
    assert_eq!(analysis_started, 0);

    harness.shutdown().await;
}
