//! Scheduler-side behaviour of an accepted Start/F5 retry.
//!
//! The regression this reproduces is the real one: an Apply invocation is
//! terminated by its absolute runtime limit, the change settles into terminal
//! Error, and the scheduler stays alive. Two things must then be true at once —
//! the failed invocation is never redispatched on its own, and a later explicit
//! operator retry *does* release it.
//!
//! The retry half runs through the production `RunControlService` over the same
//! reducer and the same `DynamicQueue` the scheduler reads, so what reaches the
//! scheduler here is the edge a real F5 publishes rather than a hand-written
//! one. The scheduler half runs the production loop steps
//! (`consume_explicit_retry_edges`, queue ingestion, reconciliation,
//! `evaluate_queued_reanalysis_and_dispatch`) over a real reducer.
//!
//! Paused Tokio time throughout: no assertion depends on a wall-clock threshold.

use crate::analyzer::{AnalysisOutcome, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, OperatorMode,
};
use crate::orchestration::run_control::testing::{RecordingScheduler, SchedulerCall};
use crate::orchestration::run_control::{
    ResolveReservations, RunControlOutcome, RunControlService, SchedulerEffect, StartEligibility,
};
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

/// The change the runtime limit terminated.
const ALPHA: &str = "alpha";
/// An unrelated failed change, to prove the retry edge is target-specific.
const BETA: &str = "beta";
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

/// What one drained batch of scheduler events observed.
#[derive(Debug, Default)]
struct ObservedEvents {
    analysis_started: usize,
    /// Changes whose workspace preparation really started, in dispatch order.
    dispatch_started: Vec<String>,
}

/// A live scheduler plus the production operator services that address it.
struct Harness {
    executor: ParallelExecutor,
    queue: Arc<DynamicQueue>,
    state: Arc<RwLock<OrchestratorState>>,
    run_control: RunControlService,
    marks: Arc<ExecutionMarkStore>,
    scheduler: Arc<RecordingScheduler>,
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
    repo_root: std::path::PathBuf,
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

        // The operator half is production wiring over the *same* reducer and the
        // *same* queue: an accepted retry has to reach this scheduler through
        // the edge a real F5 publishes, not through a hand-written one.
        let marks = Arc::new(ExecutionMarkStore::new());
        let scheduler = Arc::new(RecordingScheduler::new());
        scheduler.set_running(true);
        let operator = Arc::new(OperatorCommandService::new(
            state.clone(),
            queue.clone(),
            Arc::new(NoopQueueHooks),
            marks.clone(),
        ));
        let run_control = RunControlService::new(
            state.clone(),
            operator,
            scheduler.clone(),
            Arc::new(ResolveReservations::new()),
            Arc::new(StartEligibility::new()),
        );

        Self {
            executor,
            queue,
            state,
            run_control,
            marks,
            scheduler,
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
            repo_root: repo_dir.path().to_path_buf(),
            _repo_dir: repo_dir,
            _workspace_base: workspace_base,
        }
    }

    /// Make the queue debounce window fresh, so a timer wake would be deferred.
    ///
    /// Every reanalysis this test observes therefore has to come from a real
    /// operator-intent edge rather than from an expired debounce.
    async fn arm_queue_debounce(&self) {
        let mut last_change = self.executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    /// Reproduce an Apply terminated by its absolute runtime limit.
    ///
    /// Reducer terminal Error plus the scheduler-local failed classification is
    /// exactly what `handle_workspace_completion` leaves behind for a failed
    /// change, and the invocation is deliberately not retried by the cycle that
    /// observed it.
    async fn fail_with_runtime_limit(&mut self, change_id: &str) {
        {
            let mut guard = self.state.write().await;
            guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
            guard.apply_execution_event(&ExecutionEvent::ProcessingError {
                id: change_id.to_string(),
                error: "Apply exceeded its absolute runtime limit of 3600s".to_string(),
            });
        }
        self.executor.failed_tracker.mark_failed(change_id);
    }

    /// Declare `dependent` as depending on `dependency`.
    ///
    /// A dependent is the observable for a failed classification: the tracker's
    /// own set is private, but whether it still skips a dependent is exactly
    /// what the classification decides.
    ///
    /// Re-declared immediately before each observation because analysis owns
    /// the dependency map in production and replaces it on every pass. It never
    /// touches the failed set, so re-declaring cannot restore a classification
    /// an accepted retry released.
    fn depends_on(&mut self, edges: &[(&str, &str)]) {
        let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
        for (dependent, dependency) in edges {
            dependencies
                .entry((*dependent).to_string())
                .or_default()
                .push((*dependency).to_string());
        }
        self.executor.failed_tracker.set_dependencies(dependencies);
    }

    /// True while `dependent` is still gated by a failed dependency.
    fn is_gated_by_failure(&self, dependent: &str) -> bool {
        self.executor
            .failed_tracker
            .should_skip(dependent)
            .is_some()
    }

    /// The production loop's Step 0, without the reason derivation that follows
    /// it: the drain applies its route-scoped releases and latches whatever
    /// analysis-bypass authority the edges carried.
    async fn consume_retry_edges(&mut self) -> bool {
        self.executor
            .consume_explicit_retry_edges()
            .await
            .bypass_armed
    }

    /// One scheduler loop pass: ingest, reconcile, then evaluate and dispatch.
    ///
    /// Step 0, hint ingestion, and reconciliation run in the loop's own order,
    /// and the pass's reanalysis reason is derived by the production
    /// `derive_pass_reanalysis_reason` from what those steps observed. No test
    /// supplies a `ReanalysisReason`, so a pass that reaches the analyzer proves
    /// the production path produced the authority for it.
    async fn run_loop_iteration<F>(&mut self, analyzer: &F) -> Option<(bool, u32)>
    where
        for<'a> F: Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync,
    {
        let retry_edges = self.executor.consume_explicit_retry_edges().await;
        let in_flight = self.in_flight.clone();
        let dynamic_queue_added = self
            .executor
            .check_dynamic_queue_and_add_changes(
                &mut self.queued,
                &in_flight,
                &mut self.reanalysis_reason,
            )
            .await;
        let reconciliation = self
            .executor
            .reconcile_queued_candidates_from_shared_state(&mut self.queued, &in_flight)
            .await;
        self.reanalysis_reason = ParallelExecutor::derive_pass_reanalysis_reason(
            self.reanalysis_reason,
            retry_edges,
            reconciliation,
            dynamic_queue_added,
        );

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

    /// Drain the event channel, counting analysis starts and dispatch starts.
    fn drain_events(&mut self) -> (usize, usize) {
        let observed = self.drain_observed_events();
        (observed.analysis_started, observed.dispatch_started.len())
    }

    /// The catalog entry queue reconciliation admits for `change_id`.
    ///
    /// Built from the same `list_changes_native_from` read reconciliation uses,
    /// so a candidate restored here is byte-identical to the one the scheduler
    /// admitted — which matters because the analysis-input signature is derived
    /// from the candidate's own fields.
    fn catalog_candidate(&self, change_id: &str) -> Change {
        crate::openspec::list_changes_native_from(&self.repo_root)
            .expect("load the OpenSpec catalog")
            .into_iter()
            .find(|change| change.id == change_id)
            .expect("the catalog contains the change")
    }

    /// Settle a dispatched change the way a finished workspace task plus the next
    /// reconciliation pass would.
    ///
    /// This harness never joins the `JoinSet`, so the spawned task is aborted and
    /// reaped here; what it leaves behind is the two facts that matter to the
    /// scheduler — the change is no longer in flight, and reducer queue intent
    /// still holds it as a scheduler-local candidate. Nothing about analysis
    /// suppression, retry edges, or the failed tracker is touched.
    async fn settle_dispatch_back_to_queued(&mut self, change_id: &str) {
        self.join_set.abort_all();
        while self.join_set.join_next().await.is_some() {}
        self.in_flight.remove(change_id);
        if !self.queued.iter().any(|change| change.id == change_id) {
            let candidate = self.catalog_candidate(change_id);
            self.queued.push(candidate);
        }
    }

    /// Record an Acceptance-owned resumable stall, exactly as an `AcceptanceGated`
    /// report classifies for repository-fixable facts with no unblock condition.
    async fn stall_in_acceptance(&self, change_id: &str) {
        self.state
            .write()
            .await
            .apply_execution_event(&ExecutionEvent::AcceptanceGated {
                change_id: change_id.to_string(),
                blocker: crate::events::StalledBlocker {
                    category: "acceptance_finding".to_string(),
                    phase: "acceptance".to_string(),
                    gate: "acceptance".to_string(),
                    error_summary: "unresolved acceptance finding".to_string(),
                    evidence: vec!["tests/acceptance.rs:1".to_string()],
                    unblock_condition: None,
                    prerequisite_owner: None,
                    next_action: "resolve finding and retry".to_string(),
                    resumable: true,
                    worktree_preserved: true,
                },
            });
    }

    /// Drain the event channel, keeping dispatch-start attribution.
    fn drain_observed_events(&mut self) -> ObservedEvents {
        let mut observed = ObservedEvents::default();
        while let Ok(event) = self.events.try_recv() {
            match event {
                ExecutionEvent::AnalysisStarted { .. } => observed.analysis_started += 1,
                // Preparation is announced by `dispatch_change_to_workspace`
                // itself, for a change that has passed the stop and terminal
                // gates and now owns an execution slot. It is the reducer's
                // `preparing` transition, so a stubbed or no-op dispatch emits
                // none of these.
                ExecutionEvent::WorkspacePreparationStarted { change_id } => {
                    observed.dispatch_started.push(change_id)
                }
                _ => {}
            }
        }
        observed
    }

    async fn status(&self, change_id: &str) -> String {
        self.state
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }

    async fn shutdown(mut self) {
        self.join_set.abort_all();
        while self.join_set.join_next().await.is_some() {}
    }
}

/// The whole regression, in the order it happens to an operator.
///
/// A runtime-limit failure is not redispatched by the live scheduler; the
/// operator then marks the change and presses F5; the accepted retry clears the
/// terminal error through retry intent, publishes one target-specific edge, the
/// scheduler consumes it, and a distinct analysis follows immediately — with no
/// mark-settlement interval and no expired debounce involved.
#[tokio::test(start_paused = true)]
async fn change_error_f5_retry_runtime_limit_failure_is_retried_only_by_explicit_intent() {
    let mut harness = Harness::new(&[ALPHA, BETA], 2);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    harness.fail_with_runtime_limit(ALPHA).await;
    harness.fail_with_runtime_limit(BETA).await;
    assert_eq!(harness.status(ALPHA).await, "error");

    // ── No automatic retry ────────────────────────────────────────────────
    // The scheduler stays alive and keeps reconciling. Ordinary timer wakes,
    // queue reconciliation, and the surviving worktree must not put the
    // terminated invocation back into Apply.
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }
    let (_, apply_started) = harness.drain_events();
    assert_eq!(
        apply_started, 0,
        "a runtime-limit failure must not be redispatched without explicit intent"
    );
    assert!(
        harness.in_flight.is_empty(),
        "no terminal-error change may be admitted for execution"
    );
    harness.depends_on(&[("alpha-dep", ALPHA), ("beta-dep", BETA)]);
    assert!(
        harness.is_gated_by_failure("alpha-dep"),
        "the failed classification stands until an accepted retry releases it"
    );
    let analyses_before_retry = harness.analyses();

    // ── The operator marks the row and presses F5 ─────────────────────────
    harness.marks.set(ALPHA, true);
    let outcome = harness
        .run_control
        .start(OperatorMode::Running)
        .await
        .expect("a marked retry-eligible error is startable while the run is live");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Notified,
            excluded: Vec::new(),
        },
        "the live scheduler is woken rather than joined by a second boundary"
    );
    assert_eq!(
        harness.scheduler.calls(),
        vec![SchedulerCall::Notified],
        "exactly one wake"
    );
    assert_eq!(
        harness.status(ALPHA).await,
        "queued",
        "the terminal error was cleared through retry intent"
    );
    assert_eq!(
        harness.status(BETA).await,
        "error",
        "an unretried failure keeps its terminal evidence"
    );

    // ── The scheduler consumes the target-specific edge ───────────────────
    assert!(
        harness.consume_retry_edges().await,
        "the accepted retry published an edge for this scheduler to consume"
    );
    harness.depends_on(&[("alpha-dep", ALPHA), ("beta-dep", BETA)]);
    assert!(
        !harness.is_gated_by_failure("alpha-dep"),
        "the retried change's failed classification is released"
    );
    assert!(
        harness.is_gated_by_failure("beta-dep"),
        "the edge is target-specific: another failed change stays gated"
    );

    // ── A distinct analysis follows, with the debounce window still fresh ──
    harness.run_loop_iteration(&analyzer).await;
    let (analysis_started, _) = harness.drain_events();
    assert_eq!(
        harness.analyses(),
        analyses_before_retry + 1,
        "the consumed edge arms exactly one reevaluation"
    );
    assert_eq!(
        analysis_started, 1,
        "a distinct AnalysisStarted follows the accepted retry without \
         waiting for mark settlement or debounce expiry"
    );
    assert!(
        harness.in_flight.contains(ALPHA),
        "the released change is dispatched once dependency and capacity guards allow it"
    );

    harness.shutdown().await;
}

/// The negative half, isolated: without an accepted retry there is no edge, so
/// an ordinary queue notification cannot manufacture one.
#[tokio::test(start_paused = true)]
async fn change_error_f5_retry_ordinary_notification_creates_no_retry_edge() {
    let mut harness = Harness::new(&[ALPHA], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    harness.fail_with_runtime_limit(ALPHA).await;

    // A generic wake, exactly as an unrelated queue mutation would produce.
    harness.queue.notify_scheduler();
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    assert!(
        !harness.consume_retry_edges().await,
        "a generic notification is not an explicit-retry edge"
    );
    harness.depends_on(&[("alpha-dep", ALPHA)]);
    assert!(
        harness.is_gated_by_failure("alpha-dep"),
        "no failed classification may be released without accepted retry intent"
    );
    assert_eq!(harness.status(ALPHA).await, "error");
    let (_, apply_started) = harness.drain_events();
    assert_eq!(apply_started, 0);

    harness.shutdown().await;
}

/// Marking alone is next-run intent, not retry authorization: the mark store
/// changes and nothing else does.
#[tokio::test(start_paused = true)]
async fn change_error_f5_retry_marking_alone_publishes_no_edge() {
    let mut harness = Harness::new(&[ALPHA], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    harness.fail_with_runtime_limit(ALPHA).await;

    harness
        .run_control
        .operator()
        .set_execution_mark(ALPHA, true)
        .await
        .expect("a non-terminal row accepts a mark at any time");

    harness.timer_wake().await;
    harness.run_loop_iteration(&analyzer).await;

    assert!(
        !harness.consume_retry_edges().await,
        "an execution mark must not publish an explicit-retry edge"
    );
    assert_eq!(
        harness.status(ALPHA).await,
        "error",
        "marking does not clear terminal error evidence"
    );
    harness.depends_on(&[("alpha-dep", ALPHA)]);
    assert!(harness.is_gated_by_failure("alpha-dep"));
    let (_, apply_started) = harness.drain_events();
    assert_eq!(apply_started, 0);

    harness.shutdown().await;
}

// ============================================================================
// Settled Apply iteration limit: fresh budget for the explicit retry
// ============================================================================
//
// The runtime-limit case above proves the *edge* is what releases a failed
// change. The iteration-limit case adds the second half the operator actually
// hit: the Apply-dispatch budget is owned by this scheduler boundary, not by the
// invocation that spent it, so a persistent scheduler that outlives the
// exhausted invocation would refuse the very first dispatch of the retry the
// operator just authorized. The explicit edge is what releases that budget, and
// nothing else produces one.

impl Harness {
    /// Reproduce an Apply invocation terminated at its iteration ceiling.
    ///
    /// Spends the budget through the real `ApplyBudget`, records the typed
    /// reducer diagnostic the finish path reads, and settles the change into
    /// terminal Error with the scheduler-local failed classification — which is
    /// exactly what `handle_workspace_completion` leaves behind.
    async fn fail_with_iteration_limit(&mut self, change_id: &str, max: u32) {
        for _ in 0..max {
            self.executor.apply_budget.reserve(change_id, max);
        }
        {
            let mut guard = self.state.write().await;
            guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
            guard.apply_execution_event(&ExecutionEvent::ProcessingError {
                id: change_id.to_string(),
                error: format!("reached maximum iterations ({max}/{max}) without completion"),
            });
            guard.record_apply_iteration_limit(change_id, max, max);
        }
        self.executor.failed_tracker.mark_failed(change_id);
    }

    /// Whether the next Apply dispatch for `change_id` would be refused.
    fn apply_budget_exhausted(&self, change_id: &str, max: u32) -> bool {
        self.executor
            .apply_budget
            .exhaustion(change_id, max)
            .is_some()
    }
}

/// The whole regression, in the order it happens to an operator: an exhausted
/// ceiling is not redispatched by the live scheduler, and the later explicit
/// retry receives a fresh budget rather than reaching the same ceiling at once.
#[tokio::test(start_paused = true)]
async fn change_error_f5_retry_iteration_limit_receives_fresh_budget_on_explicit_retry() {
    const MAX: u32 = 3;

    let mut harness = Harness::new(&[ALPHA, BETA], 2);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    harness.fail_with_iteration_limit(ALPHA, MAX).await;
    harness.fail_with_iteration_limit(BETA, MAX).await;
    assert_eq!(harness.status(ALPHA).await, "error");
    assert!(
        harness.apply_budget_exhausted(ALPHA, MAX),
        "the invocation really spent its ceiling"
    );

    // ── No automatic retry ────────────────────────────────────────────────
    // The scheduler stays alive and keeps reconciling. Neither the retained
    // diagnostic nor the spent budget may be released without explicit intent.
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }
    let (_, apply_started) = harness.drain_events();
    assert_eq!(
        apply_started, 0,
        "an iteration-limit failure must not be redispatched without explicit intent"
    );
    assert!(
        harness.apply_budget_exhausted(ALPHA, MAX),
        "and reconciliation grants no budget"
    );
    assert!(
        harness
            .state
            .read()
            .await
            .apply_iteration_limit(ALPHA)
            .is_some(),
        "the diagnostic survives every automatic cycle"
    );

    // ── The operator marks the row and presses F5 ─────────────────────────
    harness.marks.set(ALPHA, true);
    let outcome = harness
        .run_control
        .start(OperatorMode::Running)
        .await
        .expect("a settled iteration-limit error is startable while the run is live");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Notified,
            excluded: Vec::new(),
        },
        "the live scheduler is woken rather than joined by a second boundary"
    );
    assert_eq!(
        harness.status(ALPHA).await,
        "queued",
        "the terminal error was cleared through retry intent"
    );
    assert!(
        harness
            .state
            .read()
            .await
            .apply_iteration_limit(ALPHA)
            .is_none(),
        "the diagnostic is consumed by the same explicit intent"
    );
    assert!(
        harness
            .state
            .read()
            .await
            .apply_iteration_limit(BETA)
            .is_some(),
        "and an unretried failure keeps its own diagnostic"
    );

    // ── The scheduler consumes the edge and releases the budget ───────────
    assert!(
        harness.consume_retry_edges().await,
        "the accepted retry published an edge for this scheduler to consume"
    );
    assert!(
        !harness.apply_budget_exhausted(ALPHA, MAX),
        "the retried change enters its new invocation with a fresh Apply budget"
    );
    assert!(
        harness.apply_budget_exhausted(BETA, MAX),
        "release is target-specific: another exhausted change keeps its ceiling"
    );

    // ── And the released change really reaches dispatch again ─────────────
    // This scheduler boundary admits work by starting the change's next
    // analysis; the Apply the fresh budget pays for happens downstream of that
    // dispatch, so the evidence here is the admission plus the unspent ceiling.
    harness.run_loop_iteration(&analyzer).await;
    let (analysis_started, _) = harness.drain_events();
    assert!(
        harness.in_flight.contains(ALPHA),
        "the released change is dispatched once dependency and capacity guards allow it"
    );
    assert_eq!(
        analysis_started, 1,
        "exactly one reevaluation follows the accepted retry"
    );
    assert!(
        !harness.in_flight.contains(BETA),
        "and the unretried failure is still not admitted"
    );
    assert!(
        matches!(
            harness.executor.apply_budget.reserve(ALPHA, MAX),
            crate::execution::apply::ApplyBudgetReservation::Reserved { attempt: 1, .. }
        ),
        "the new invocation's first dispatch is admitted as attempt 1 rather than refused"
    );

    harness.shutdown().await;
}

/// The negative half for the budget: a generic notification releases nothing.
#[tokio::test(start_paused = true)]
async fn change_error_f5_retry_iteration_limit_budget_survives_ordinary_notification() {
    const MAX: u32 = 3;

    let mut harness = Harness::new(&[ALPHA], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    harness.fail_with_iteration_limit(ALPHA, MAX).await;

    // A generic wake, exactly as an unrelated queue mutation would produce, plus
    // an ordinary mark: neither is explicit retry intent.
    harness.queue.notify_scheduler();
    harness
        .run_control
        .operator()
        .set_execution_mark(ALPHA, true)
        .await
        .expect("a settled limited row still accepts next-run intent");
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    assert!(
        !harness.consume_retry_edges().await,
        "no automatic path publishes an explicit-retry edge"
    );
    assert!(
        harness.apply_budget_exhausted(ALPHA, MAX),
        "so the exhausted budget is never released"
    );
    assert_eq!(harness.status(ALPHA).await, "error");
    let (_, apply_started) = harness.drain_events();
    assert_eq!(apply_started, 0);

    harness.shutdown().await;
}

// ============================================================================
// Acceptance-stall retry: bypassing a matching analysis cache and dispatching
// ============================================================================
//
// The two cases above are terminal-error retries, whose reducer command is
// `RetryError` and which therefore always armed an edge. The reproduced defect
// lives on the other route: a `stalled` change is retried with `AddToQueue`, and
// the reducer already listed it as queued work, so the retry changes no
// scheduler-visible queue membership at all. Without an edge, the live scheduler
// reduced the resulting wake to an ordinarily suppressible one and the matching
// analysis-input signature swallowed it — the observed
// `Queue notification received while scheduler idle` /
// `No analysis started: reason=unchanged_analysis_input` pair, with the queued
// work left undispatched until the operator toggled queue intent off and on.

impl Harness {
    /// Whether the next Apply dispatch for `change_id` would find its ceiling spent.
    fn budget_spent(&self, change_id: &str, max: u32) -> bool {
        self.executor
            .apply_budget
            .exhaustion(change_id, max)
            .is_some()
    }

    /// Occupy `count` execution slots with work this harness does not model.
    ///
    /// Capacity is derived from the in-flight set, so this is how a pass reaches
    /// the production zero-capacity gate — the gate that ends a scheduler pass
    /// after classification but before the analyzer runs.
    fn occupy_slots(&mut self, count: usize) {
        for index in 0..count {
            self.in_flight.insert(format!("unrelated-{index}"));
        }
    }

    fn release_occupied_slots(&mut self) {
        self.in_flight.retain(|id| !id.starts_with("unrelated-"));
    }
}

/// The whole regression, in the order it happens to an operator.
///
/// A completed dependency analysis records its input; the change then stalls in
/// Acceptance and sits as reducer-visible queued work whose signature still
/// matches that record. `retry_change` accepts it through the acceptance-stall
/// route, and from there everything is production: the accepted retry arms an
/// edge, the scheduler's own Step 0 latches its bypass authority, the loop's own
/// `derive_pass_reanalysis_reason` turns that into a bypass-carrying reason, and
/// the matching signature no longer suppresses the evaluation. The test supplies
/// no `ReanalysisReason` anywhere.
#[tokio::test(start_paused = true)]
async fn retry_change_bypasses_unchanged_analysis_input_and_dispatches() {
    const MAX: u32 = 3;
    let mut harness = Harness::new(&[ALPHA, BETA], 1);
    let analyzer = counting_analyzer(harness.analyses.clone());

    // An unrelated failed change, so every release below can be shown to be
    // target-specific.
    harness.fail_with_runtime_limit(BETA).await;
    harness.depends_on(&[("beta-dep", BETA)]);
    assert!(harness.is_gated_by_failure("beta-dep"));

    // ── A completed analysis records this input ───────────────────────────
    {
        let mut guard = harness.state.write().await;
        guard.apply_command(ReducerCommand::AddToQueue(ALPHA.to_string()));
    }
    harness.run_loop_iteration(&analyzer).await;
    let baseline = harness.drain_observed_events();
    assert_eq!(baseline.analysis_started, 1);
    assert_eq!(
        baseline.dispatch_started,
        vec![ALPHA.to_string()],
        "the first evaluation really dispatched the change it analysed"
    );

    // The invocation returns having stalled in Acceptance, and the change goes
    // back to being reducer-visible queued work the scheduler already holds. Its
    // Apply ceiling is spent: this scheduler boundary owns that budget.
    harness.settle_dispatch_back_to_queued(ALPHA).await;
    for _ in 0..MAX {
        harness.executor.apply_budget.reserve(ALPHA, MAX);
    }
    harness.stall_in_acceptance(ALPHA).await;
    assert_eq!(harness.status(ALPHA).await, "stalled");
    assert!(harness.budget_spent(ALPHA, MAX));

    // ── A stalled change is held, not analysed ───────────────────────────
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }
    assert_eq!(
        harness.analyses(),
        1,
        "an acceptance hold is classified blocked-only; no wake analyses it"
    );
    assert!(
        harness.queued.iter().any(|change| change.id == ALPHA),
        "the held change stays reducer-visible queued work the scheduler already holds, \
         so a later retry produces no scheduler-visible queue addition"
    );

    // ── The operator retries the stalled change ──────────────────────────
    let plan = harness
        .run_control
        .operator()
        .retry_change(ALPHA)
        .await
        .expect("a resumable acceptance stall is retryable");
    assert_eq!(
        plan.change_ids,
        vec![ALPHA.to_string()],
        "the retry is accepted through the acceptance-stall route"
    );
    assert_eq!(harness.status(ALPHA).await, "queued");

    // ── A pass that ends before its analysis keeps the authority ─────────
    // Zero capacity is a production gate that ends the pass after classification
    // and before the analyzer. The authority the retry armed must outlive it.
    harness.occupy_slots(1);
    harness.timer_wake().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        1,
        "the pass ended at the capacity gate without evaluating anything"
    );
    assert_eq!(
        harness.executor.pending_retry_bypass_targets(),
        vec![ALPHA.to_string()],
        "an abandoned pass must not discard the authority it took"
    );
    harness.release_occupied_slots();
    harness.drain_observed_events();

    // ── The authorized evaluation runs against the matching signature ────
    harness.timer_wake().await;
    harness.run_loop_iteration(&analyzer).await;
    let retried = harness.drain_observed_events();
    assert_eq!(
        harness.analyses(),
        2,
        "the retry's edge produced one fresh analyzer invocation even though the \
         analysis input still matches the last completed one"
    );
    assert_eq!(retried.analysis_started, 1);
    assert_eq!(
        retried.dispatch_started,
        vec![ALPHA.to_string()],
        "and the retried change reached real Apply dispatch, not just re-analysis"
    );
    assert!(harness.in_flight.contains(ALPHA));

    // The stall route granted analysis bypass and nothing else.
    assert!(
        harness.budget_spent(ALPHA, MAX),
        "a stall-route edge must not reset the retried target's Apply budget"
    );
    harness.depends_on(&[("beta-dep", BETA)]);
    assert!(
        harness.is_gated_by_failure("beta-dep"),
        "and it must not release another change's failed classification"
    );

    // ── One-shot: later unchanged wakes are suppressed again ─────────────
    harness.settle_dispatch_back_to_queued(ALPHA).await;
    assert!(
        harness.executor.pending_retry_bypass_targets().is_empty(),
        "the authorized evaluation spent the bypass"
    );
    harness.queue.notify_scheduler();
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }
    let after = harness.drain_observed_events();
    assert_eq!(
        harness.analyses(),
        2,
        "ordinary timer wakes and a generic notification observe the same analysis \
         input and are suppressed exactly as before"
    );
    assert_eq!(after.analysis_started, 0);
    assert!(after.dispatch_started.is_empty());

    harness.shutdown().await;
}
