//! Scheduler-side behaviour of an explicitly resumed stopped change.
//!
//! The unit coverage in `orchestration::run_control` proves the shared Start
//! transaction accepts a preserved stopped mark and commits ordinary queue
//! intent for it. That is not the whole claim: `queued` is a display string, and
//! the regression this change exists for is a change that never runs again. What
//! has to be true is that a *scheduler* picks the resumed row up and performs a
//! new dependency analysis for it, without an owner restart.
//!
//! So both halves run through production code over one reducer. The resume half
//! is the real `RunControlService` — the same transaction TUI F5 and `/api/v2`
//! Start submit — and the scheduler half is the production loop steps (queue
//! ingestion, `reconcile_queued_candidates_from_shared_state`,
//! `derive_pass_reanalysis_reason`, `evaluate_queued_reanalysis_and_dispatch`)
//! over a real `DynamicQueue`. No test seeds the candidate list and no test
//! supplies a `ReanalysisReason`: a pass that reaches the analyzer proves the
//! production path produced the authority for it.
//!
//! Paused Tokio time throughout, and the queue debounce window is armed before
//! every observation, so no assertion depends on a wall-clock threshold and no
//! analysis can be attributed to an expired timer.

use crate::analyzer::{AnalysisOutcome, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::Change;
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

/// The force-stopped change whose mark the operator preserved.
const ALPHA: &str = "alpha";
/// An unrelated change, to prove the resume is target-scoped.
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
/// Reconciliation loads its candidates from the catalog, so a change that
/// existed only in the reducer would be dropped as unloadable — which would make
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

/// What one drained batch of scheduler events observed.
#[derive(Debug, Default, PartialEq, Eq)]
struct ObservedEvents {
    /// How many dependency analyses the scheduler announced.
    analysis_started: usize,
    /// Changes whose workspace preparation really started, in dispatch order.
    dispatch_started: Vec<String>,
}

/// A scheduler loop plus the production operator services that address it.
struct Harness {
    executor: ParallelExecutor,
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
        // *same* queue: the resume has to reach this scheduler through the
        // transaction a real F5 or `/api/v2` Start submits.
        let marks = Arc::new(ExecutionMarkStore::new());
        let scheduler = Arc::new(RecordingScheduler::new());
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
            // scheduler has already run its first analysis, so every analysis
            // observed below has to be caused by the resume.
            iteration: 2,
            max_parallelism,
            analyses: Arc::new(AtomicUsize::new(0)),
            events,
            _repo_dir: repo_dir,
            _workspace_base: workspace_base,
        }
    }

    /// Make the queue debounce window fresh, so a timer wake would be deferred.
    async fn arm_queue_debounce(&self) {
        let mut last_change = self.executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    /// Reproduce the observed row: settled operator `stopped`, mark preserved.
    ///
    /// The mark is asserted rather than assumed, because the whole regression is
    /// about the combination — a stopped row with no mark is not the state that
    /// refused to start.
    async fn force_stop_with_preserved_mark(&self, change_id: &str) {
        self.state
            .write()
            .await
            .apply_command(ReducerCommand::StopChange(change_id.to_string()));
        self.marks.set(change_id, true);
        assert_eq!(self.status(change_id).await, "stopped");
        assert!(self.marks.is_marked(change_id));
    }

    /// One scheduler loop pass: ingest, reconcile, then evaluate and dispatch.
    ///
    /// The pass's reanalysis reason is derived by the production
    /// `derive_pass_reanalysis_reason` from what those steps observed, so no
    /// test hands the analyzer its own authority to run.
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

    /// Drain the event channel, keeping dispatch-start attribution.
    fn drain_events(&mut self) -> ObservedEvents {
        let mut observed = ObservedEvents::default();
        while let Ok(event) = self.events.try_recv() {
            match event {
                ExecutionEvent::AnalysisStarted { .. } => observed.analysis_started += 1,
                // Preparation is announced by `dispatch_change_to_workspace`
                // itself, for a change that has passed the stop and terminal
                // gates and now owns an execution slot.
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

    /// Candidate IDs the scheduler is currently holding, sorted.
    fn queued_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.queued.iter().map(|change| change.id.clone()).collect();
        ids.sort();
        ids
    }

    async fn shutdown(mut self) {
        self.join_set.abort_all();
        while self.join_set.join_next().await.is_some() {}
    }
}

/// The whole regression, in the order it happens to an operator.
///
/// A force-stopped change with a preserved mark is invisible to the scheduler;
/// the operator presses F5 in `Stopped`; the shared transaction resumes it over
/// exactly one fresh boundary; and the next scheduler pass admits it as an
/// ordinary candidate and performs a *new* dependency analysis for it — no owner
/// restart, no expired debounce, and no explicit-retry semantics.
#[tokio::test(start_paused = true)]
async fn stopped_marked_resume_resumed_change_reaches_a_new_dependency_analysis() {
    let mut harness = Harness::new(&[ALPHA, BETA], 2);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    harness.force_stop_with_preserved_mark(ALPHA).await;

    // ── Nothing resumes on its own ────────────────────────────────────────
    // The preserved mark is selection, not intent. Ordinary timer wakes and
    // queue reconciliation must leave the stopped row exactly where it is.
    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }
    assert_eq!(
        harness.drain_events(),
        ObservedEvents::default(),
        "a preserved mark must not produce an analysis or a dispatch on its own"
    );
    assert!(
        harness.queued_ids().is_empty(),
        "a stopped row carries no ordinary queue intent for reconciliation to admit"
    );
    assert_eq!(harness.analyses(), 0);

    // ── The operator presses F5 in Stopped ────────────────────────────────
    let outcome = harness
        .run_control
        .start(OperatorMode::Stopped)
        .await
        .expect("an explicit Start in Stopped resumes the preserved stopped mark");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: false,
            scheduler: SchedulerEffect::Started,
            excluded: Vec::new(),
        },
        "the resume starts one ordinary boundary over exactly the resumed target"
    );
    assert_eq!(
        harness.scheduler.calls(),
        vec![SchedulerCall::Started {
            targets: vec![ALPHA.to_string()],
            explicit_retry: false,
        }],
        "exactly one fresh scheduler boundary, with ordinary semantics"
    );
    assert_eq!(harness.status(ALPHA).await, "queued");
    assert!(
        harness.marks.is_marked(ALPHA),
        "an accepted resume preserves the execution mark"
    );

    // ── The scheduler analyses the resumed candidate ──────────────────────
    // The debounce window is re-armed first, so the analysis below cannot be
    // attributed to an expired timer: only the resumed reducer intent can
    // produce it.
    harness.arm_queue_debounce().await;
    harness.run_loop_iteration(&analyzer).await;

    let observed = harness.drain_events();
    assert_eq!(
        harness.analyses(),
        1,
        "the resumed candidate must produce a new dependency analysis"
    );
    assert_eq!(
        observed.analysis_started, 1,
        "and the scheduler must announce exactly one analysis edge for it"
    );
    assert_eq!(
        observed.dispatch_started,
        vec![ALPHA.to_string()],
        "the resumed change is dispatched, without an owner restart"
    );
    assert_eq!(
        harness.in_flight.iter().cloned().collect::<Vec<_>>(),
        vec![ALPHA.to_string()]
    );

    harness.shutdown().await;
}

/// Acceptance criterion 3 at the scheduler boundary: marking is not resuming.
///
/// Marking, unmarking, and re-marking a stopped change — the gestures whose
/// `unchanged` results made the original incident look like a stuck owner — leave
/// the scheduler with nothing to admit, nothing to analyse, and nothing to
/// dispatch, over a run whose ordinary work proves the loop is doing real passes.
#[tokio::test(start_paused = true)]
async fn stopped_marked_resume_mark_mutation_creates_no_analysis_edge() {
    let mut harness = Harness::new(&[ALPHA, BETA], 2);
    let analyzer = counting_analyzer(harness.analyses.clone());
    harness.arm_queue_debounce().await;

    harness.force_stop_with_preserved_mark(ALPHA).await;
    // The positive control: an ordinary explicit queue addition really does
    // reach this scheduler, so the negative assertions below are about the
    // stopped row rather than about an inert loop.
    harness
        .state
        .write()
        .await
        .apply_command(ReducerCommand::AddToQueue(BETA.to_string()));
    harness.run_loop_iteration(&analyzer).await;

    let observed = harness.drain_events();
    assert_eq!(observed.analysis_started, 1, "the control analysed");
    assert_eq!(
        observed.dispatch_started,
        vec![BETA.to_string()],
        "the control dispatched, and the stopped row did not"
    );

    // Every mark gesture, over a scheduler that is demonstrably live.
    harness.arm_queue_debounce().await;
    harness.marks.set(ALPHA, true);
    harness.marks.set(ALPHA, false);
    harness.marks.set(ALPHA, true);
    harness.marks.replace([ALPHA.to_string(), BETA.to_string()]);

    for _ in 0..3 {
        harness.timer_wake().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    let observed = harness.drain_events();
    assert_eq!(
        observed.dispatch_started,
        Vec::<String>::new(),
        "no mark gesture may dispatch the stopped change"
    );
    assert_eq!(
        harness.status(ALPHA).await,
        "stopped",
        "the terminal stop evidence is untouched by every mark gesture"
    );
    assert!(
        !harness.queued_ids().contains(&ALPHA.to_string()),
        "and the scheduler never admits it as a candidate: {:?}",
        harness.queued_ids()
    );

    harness.shutdown().await;
}
