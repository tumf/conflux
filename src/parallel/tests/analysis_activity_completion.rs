//! Regression tests for closing the dependency-analysis activity at its real producer.
//!
//! `ParallelExecutor::run_dependency_analysis_attempt` used to emit `AnalysisStarted` and never
//! the matching `AnalysisCompleted`, so `ExecutionFactsStore` kept publishing
//! `dependency_analysis` as active work for a persistent scheduler that had nothing running.
//!
//! Every test here drives the production scheduler pass
//! (`evaluate_queued_reanalysis_and_dispatch`) with an analyzer double that only controls *when*
//! and *what* it returns; it never emits an event. Each event the pass produces is forwarded, in
//! order, through a real [`EventDispatcher`] bound to a real [`ExecutionFactsStore`]. Forwarding
//! happens on the test task itself rather than through a background bridge, so every assertion
//! observes a store that has already absorbed exactly the events seen so far. No test ever
//! constructs an `AnalysisCompleted`.

use super::support::create_test_config;
use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult, ParallelGroup};
use crate::events::{cli_event_sinks, EventDispatcher, ExecutionEvent};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::execution_facts::{ExecutionFactsStore, ProcessActivity};
use crate::orchestration::state::OrchestratorState;
use crate::parallel::analysis_signature::AnalysisInputProbe;
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::dynamic_queue::ReanalysisReason;
use crate::parallel::queue_state::ReanalysisDispatchContext;
use crate::parallel::{ParallelExecutor, WorkspaceResult};
use crate::vcs::VcsBackend;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

/// Generous hang safeguard only; no assertion depends on elapsed time.
const HANG_GUARD: Duration = Duration::from_secs(30);

/// Held in flight so an empty dispatch selection is not the idle-scheduler anomaly.
///
/// Every analyzed candidate depends on it, which keeps dispatch selection empty through an
/// ordinary in-flight dependency wait. A *missing* dependency is deliberately not used: its
/// diagnostic is a process-level `Error`, which closes every activity and would hide a missing
/// `AnalysisCompleted`.
const DISPATCH_HOLDER: &str = "dispatch-holder";

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

/// A real repository root, which dependency resolution reads to classify a wait.
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

/// Fixed analysis-input probe, so the unchanged-input gate never touches real VCS state.
struct FixedProbe;

#[async_trait::async_trait]
impl AnalysisInputProbe for FixedProbe {
    async fn base_revision(&self) -> Result<String, String> {
        Ok("base-rev".to_string())
    }

    fn proposal_digest(&self, change_id: &str) -> Result<String, String> {
        Ok(format!("digest-of-{change_id}"))
    }
}

/// What a scripted analyzer invocation returns once released.
#[derive(Clone)]
struct Reply {
    empty_order: bool,
    groups: Option<Vec<ParallelGroup>>,
    provenance: AnalysisProvenance,
}

impl Reply {
    fn usable(provenance: AnalysisProvenance) -> Self {
        Self {
            empty_order: false,
            groups: None,
            provenance,
        }
    }
}

/// Analyzer double gated on a oneshot: its future stays pending until the test releases it.
///
/// The released reply reports every candidate as depending on the in-flight holder, so the real
/// dispatch selection that follows starts no lifecycle child.
fn gated_analyzer(
    gate: Arc<StdMutex<Option<oneshot::Receiver<Reply>>>>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        let release = gate
            .lock()
            .expect("gate lock")
            .take()
            .expect("each test releases exactly one analyzer invocation");
        let ids: Vec<String> = changes.iter().map(|change| change.id.clone()).collect();
        Box::pin(async move {
            let reply = release.await.expect("test releases the analyzer");
            let order = if reply.empty_order { Vec::new() } else { ids };
            let dependencies: HashMap<String, Vec<String>> = order
                .iter()
                .map(|id| (id.clone(), vec![DISPATCH_HOLDER.to_string()]))
                .collect();
            AnalysisOutcome::new(
                AnalysisResult {
                    order,
                    dependencies,
                    groups: reply.groups,
                },
                reply.provenance,
            )
        })
    }
}

/// The production scheduler pass plus the real dispatch boundary it reports into.
struct Harness {
    _repo: TempDir,
    executor: ParallelExecutor,
    queued: Vec<Change>,
    in_flight: HashSet<String>,
    join_set: JoinSet<WorkspaceResult>,
    cleanup_guard: WorkspaceCleanupGuard,
    reanalysis_reason: ReanalysisReason,
    producer_events: mpsc::Receiver<ExecutionEvent>,
    dispatcher: EventDispatcher,
    facts: Arc<ExecutionFactsStore>,
    /// Every producer event, in the order it was dispatched.
    dispatched: Vec<ExecutionEvent>,
    gate: Arc<StdMutex<Option<oneshot::Receiver<Reply>>>>,
}

impl Harness {
    fn new(extra_reducer_changes: &[&str]) -> Self {
        let repo = TempDir::new().expect("create repo dir");
        init_minimal_git_repo(repo.path());
        let (tx, producer_events) = mpsc::channel(256);
        let mut executor =
            ParallelExecutor::new(repo.path().to_path_buf(), create_test_config(), Some(tx));
        executor.set_analysis_input_probe(Arc::new(FixedProbe));

        let mut reducer_changes = vec!["queued-a".to_string()];
        reducer_changes.extend(extra_reducer_changes.iter().map(|id| id.to_string()));
        let state = Arc::new(RwLock::new(OrchestratorState::new(reducer_changes, 0)));
        let facts = Arc::new(ExecutionFactsStore::new());
        let dispatcher = EventDispatcher::new(state, cli_event_sinks())
            .with_execution_facts(Some(facts.clone()));

        Self {
            cleanup_guard: WorkspaceCleanupGuard::new(VcsBackend::Git, repo.path().to_path_buf()),
            _repo: repo,
            executor,
            queued: vec![test_change("queued-a")],
            in_flight: [DISPATCH_HOLDER.to_string()].into_iter().collect(),
            join_set: JoinSet::new(),
            reanalysis_reason: ReanalysisReason::Initial,
            producer_events,
            dispatcher,
            facts,
            dispatched: Vec::new(),
            gate: Arc::new(StdMutex::new(None)),
        }
    }

    /// Arm the next analyzer invocation and return the handle that releases it.
    fn arm(&self) -> oneshot::Sender<Reply> {
        let (release, gate) = oneshot::channel();
        *self.gate.lock().expect("gate lock") = Some(gate);
        release
    }

    /// Forward one producer event through the real dispatcher and facts store.
    async fn forward(
        dispatcher: &EventDispatcher,
        dispatched: &mut Vec<ExecutionEvent>,
        event: ExecutionEvent,
    ) {
        dispatched.push(event.clone());
        dispatcher.dispatch(event).await;
    }

    /// Forward everything the producer has emitted so far.
    async fn drain(&mut self) {
        while let Ok(event) = self.producer_events.try_recv() {
            Self::forward(&self.dispatcher, &mut self.dispatched, event).await;
        }
    }

    fn analysis_events(&self) -> Vec<&ExecutionEvent> {
        self.dispatched
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    ExecutionEvent::AnalysisStarted { .. }
                        | ExecutionEvent::AnalysisCompleted { .. }
                )
            })
            .collect()
    }

    fn analysis_open(&self) -> bool {
        self.facts
            .snapshot()
            .activities
            .contains(&ProcessActivity::DependencyAnalysis)
    }

    fn assert_no_process_terminal(&self) {
        assert!(
            !self.dispatched.iter().any(|event| matches!(
                event,
                ExecutionEvent::AllCompleted
                    | ExecutionEvent::Stopped
                    | ExecutionEvent::Error { .. }
            )),
            "closing analysis must not require a process terminal: {:?}",
            self.dispatched
        );
    }

    /// Run one production scheduler pass with a gated analyzer.
    ///
    /// The pass is polled on this task while producer events are forwarded. Once
    /// `AnalysisStarted` has been dispatched, `while_pending` runs against the store with the
    /// analyzer still pending; then the analyzer is released with `reply` (or the pass is
    /// dropped when `reply` is `None`). Returns the pass outcome, or `None` when dropped.
    async fn run_gated_pass(
        &mut self,
        reply: Option<Reply>,
        while_pending: impl FnOnce(&PendingView),
    ) -> Option<Option<(bool, u32)>> {
        let release = self.arm();
        let analyzer = gated_analyzer(self.gate.clone());
        let reanalysis_reason = self.reanalysis_reason;
        let mut reason = reanalysis_reason;
        let max_parallelism = self.in_flight.len() + 1;

        let outcome = {
            let pass = self.executor.evaluate_queued_reanalysis_and_dispatch(
                ReanalysisDispatchContext {
                    queued: &mut self.queued,
                    in_flight: &mut self.in_flight,
                    max_parallelism,
                    iteration: 1,
                    reanalysis_reason,
                    analyzer: &analyzer,
                    join_set: &mut self.join_set,
                    cleanup_guard: &mut self.cleanup_guard,
                    work_snapshot: None,
                },
                &mut reason,
            );
            tokio::pin!(pass);

            // Phase 1: poll until the producer's own Started has been dispatched.
            let started = tokio::time::timeout(HANG_GUARD, async {
                loop {
                    tokio::select! {
                        biased;
                        event = self.producer_events.recv() => {
                            let event = event.expect("producer channel open");
                            let is_started = matches!(event, ExecutionEvent::AnalysisStarted { .. });
                            Self::forward(&self.dispatcher, &mut self.dispatched, event).await;
                            if is_started {
                                return;
                            }
                        }
                        outcome = &mut pass => {
                            panic!("pass ended before analysis started: {outcome:?}");
                        }
                    }
                }
            })
            .await;
            started.expect("analysis must start within the hang guard");

            // The analyzer future is genuinely pending here: its release has not been sent.
            while_pending(&PendingView {
                facts: self.facts.clone(),
                dispatched: self.dispatched.clone(),
            });

            let Some(reply) = reply else {
                // Interrupt: returning drops the pinned producer future (and the unsent
                // release) before the analyzer ever returns.
                return None;
            };
            release
                .send(reply)
                .unwrap_or_else(|_| panic!("analyzer gate dropped"));

            tokio::time::timeout(HANG_GUARD, async {
                loop {
                    tokio::select! {
                        biased;
                        Some(event) = self.producer_events.recv() => {
                            Self::forward(&self.dispatcher, &mut self.dispatched, event).await;
                        }
                        outcome = &mut pass => {
                            return outcome.expect("scheduler pass must not fail");
                        }
                    }
                }
            })
            .await
            .expect("pass must finish within the hang guard")
        };
        self.drain().await;
        Some(outcome)
    }
}

/// Snapshot handed to the pending-state callback (the harness itself is mutably borrowed by the
/// in-progress pass).
struct PendingView {
    facts: Arc<ExecutionFactsStore>,
    dispatched: Vec<ExecutionEvent>,
}

impl PendingView {
    fn analysis_open(&self) -> bool {
        self.facts
            .snapshot()
            .activities
            .contains(&ProcessActivity::DependencyAnalysis)
    }
}

fn assert_one_ordered_pair(harness: &Harness, expected_groups: usize) {
    let analysis = harness.analysis_events();
    assert_eq!(
        analysis.len(),
        2,
        "exactly one Started and one Completed: {analysis:?}"
    );
    assert!(matches!(
        analysis[0],
        ExecutionEvent::AnalysisStarted { .. }
    ));
    match analysis[1] {
        ExecutionEvent::AnalysisCompleted { groups_found } => {
            assert_eq!(*groups_found, expected_groups)
        }
        other => panic!("second analysis event must be Completed, got {other:?}"),
    }
}

/// Pending analysis stays open; releasing a healthy result closes it from production code, and
/// the pass reaches its ordinary no-runnable-phase disposition without a process terminal.
#[tokio::test]
async fn analysis_activity_completion_pending_then_returned_closes_activity() {
    let mut harness = Harness::new(&[]);
    let outcome = harness
        .run_gated_pass(Some(Reply::usable(AnalysisProvenance::HealthyLlm)), |h| {
            assert!(h.analysis_open(), "pending analysis must stay open");
            assert!(h.facts.snapshot().has_active_work());
            assert!(!h
                .dispatched
                .iter()
                .any(|e| matches!(e, ExecutionEvent::AnalysisCompleted { .. })));
        })
        .await
        .expect("pass returned");

    assert_one_ordered_pair(&harness, 0);
    assert!(!harness.analysis_open());
    let snapshot = harness.facts.snapshot();
    assert!(snapshot.activities.is_empty());
    assert!(
        !snapshot.has_active_work(),
        "analysis was the only activity, so nothing remains active"
    );
    assert_eq!(
        outcome,
        Some((false, 1)),
        "no runnable phase: nothing was dispatched and the scheduler stays alive"
    );
    assert_eq!(harness.queued.len(), 1, "queued work is not consumed");
    harness.assert_no_process_terminal();
}

/// Intentional metadata-only and recoverable-failure fallback results are returned invocations
/// too, each closing exactly once.
#[tokio::test]
async fn analysis_activity_completion_metadata_and_fallback_results_close_once() {
    for provenance in [
        AnalysisProvenance::IntentionalMetadataOnly,
        AnalysisProvenance::RecoverableFailureFallback,
    ] {
        let mut harness = Harness::new(&[]);
        let outcome = harness
            .run_gated_pass(Some(Reply::usable(provenance)), |h| {
                assert!(h.analysis_open())
            })
            .await
            .expect("pass returned");
        assert_one_ordered_pair(&harness, 0);
        assert!(
            !harness.facts.snapshot().has_active_work(),
            "{provenance:?}"
        );
        assert_eq!(outcome, Some((false, 1)), "{provenance:?}");
        harness.assert_no_process_terminal();
    }
}

/// An empty order still closes the activity, keeps the scheduler alive with bounded retry, and
/// never implies AllCompleted.
#[tokio::test]
async fn analysis_activity_completion_empty_order_closes_without_terminal() {
    let mut harness = Harness::new(&[]);
    let outcome = harness
        .run_gated_pass(
            Some(Reply {
                empty_order: true,
                groups: None,
                provenance: AnalysisProvenance::HealthyLlm,
            }),
            |h| assert!(h.analysis_open()),
        )
        .await
        .expect("pass returned");

    assert_one_ordered_pair(&harness, 0);
    assert!(!harness.facts.snapshot().has_active_work());
    assert_eq!(
        outcome,
        Some((false, 1)),
        "queued work remains, so the empty order must not end the scheduler"
    );
    assert_eq!(harness.queued.len(), 1);
    harness.assert_no_process_terminal();
}

/// `groups_found` is the returned legacy groups length, not the order length.
#[tokio::test]
async fn analysis_activity_completion_reports_legacy_groups_count() {
    let mut harness = Harness::new(&[]);
    let groups = vec![
        ParallelGroup {
            id: 1,
            changes: vec!["queued-a".to_string()],
            depends_on: Vec::new(),
        },
        ParallelGroup {
            id: 2,
            changes: Vec::new(),
            depends_on: vec![1],
        },
    ];
    harness
        .run_gated_pass(
            Some(Reply {
                empty_order: false,
                groups: Some(groups),
                provenance: AnalysisProvenance::HealthyLlm,
            }),
            |_| {},
        )
        .await
        .expect("pass returned");
    assert_one_ordered_pair(&harness, 2);
    harness.assert_no_process_terminal();
}

/// Closing analysis retires only analysis: an unrelated process activity and an unrelated
/// change's active phase keep reporting active work.
#[tokio::test]
async fn analysis_activity_completion_keeps_unrelated_active_work() {
    let mut harness = Harness::new(&["unrelated"]);
    // Fabricated *starts* for the unrelated controls are allowed; analysis completion is not.
    harness
        .dispatcher
        .dispatch(ExecutionEvent::ConflictResolutionStarted)
        .await;
    harness
        .dispatcher
        .dispatch(ExecutionEvent::ApplyStarted {
            change_id: "unrelated".to_string(),
            command: "agent apply".to_string(),
        })
        .await;

    harness
        .run_gated_pass(Some(Reply::usable(AnalysisProvenance::HealthyLlm)), |h| {
            assert!(h.analysis_open())
        })
        .await
        .expect("pass returned");

    assert_one_ordered_pair(&harness, 0);
    let snapshot = harness.facts.snapshot();
    assert_eq!(
        snapshot.activities,
        vec![ProcessActivity::ConflictResolution]
    );
    assert!(snapshot.change("unrelated").current_phase.is_active());
    assert!(snapshot.has_active_work());
    harness.assert_no_process_terminal();
}

/// Dropping the producer before the analyzer returns fabricates no completion.
#[tokio::test]
async fn analysis_activity_completion_interrupted_pending_analysis_emits_nothing() {
    let mut harness = Harness::new(&[]);
    let outcome = harness
        .run_gated_pass(None, |h| assert!(h.analysis_open()))
        .await;
    assert!(outcome.is_none());
    harness.drain().await;

    let analysis = harness.analysis_events();
    assert_eq!(analysis.len(), 1, "only Started: {analysis:?}");
    assert!(matches!(
        analysis[0],
        ExecutionEvent::AnalysisStarted { .. }
    ));
    assert!(
        harness.analysis_open(),
        "retirement of a dropped invocation belongs to process-terminal cleanup"
    );
    harness.assert_no_process_terminal();
}

/// An analyzer that observes cancellation itself and still returns an outcome has returned, so
/// its invocation closes.
#[tokio::test]
async fn analysis_activity_completion_cancellation_handled_inside_analyzer_still_closes() {
    let mut harness = Harness::new(&[]);
    let cancel = CancellationToken::new();
    harness.executor.set_cancel_token(cancel.clone());

    harness
        .run_gated_pass(
            Some(Reply::usable(
                AnalysisProvenance::RecoverableFailureFallback,
            )),
            |h| {
                assert!(h.analysis_open());
                // Cancellation arrives while the analyzer is pending; the analyzer handles it
                // and returns its fallback once released.
                cancel.cancel();
            },
        )
        .await
        .expect("pass returned");

    assert_one_ordered_pair(&harness, 0);
    assert!(!harness.analysis_open());
    harness.assert_no_process_terminal();
}
