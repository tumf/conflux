//! Loop-level regression tests for unchanged-input dependency-analysis suppression.
//!
//! The archived `prevent-repeated-resolve-completion-analysis` change stopped explicit
//! scheduler edges from being replayed across timer wakes. A live v0.6.200 run still launched
//! dependency-analysis agents forever, because the queue-coalescing debounce only measures how
//! long ago the queue changed: once ten seconds have elapsed, every later 500 ms wake passes
//! that check regardless of whether the input was already analyzed.
//!
//! These tests drive `evaluate_queued_reanalysis_and_dispatch` — the production loop step that
//! `execute_with_order_based_reanalysis` calls each iteration — with paused Tokio time and an
//! analyzer invocation counter, so the suppression under test is the real scheduler path rather
//! than a test-only copy.
//!
//! Repository-visible signature material is supplied through an injected
//! [`crate::parallel::analysis_signature::AnalysisInputProbe`] double. That keeps the timing
//! assertions deterministic, lets probe counts be observed directly, and lets proposal-read and
//! revision-resolution failures be injected without depending on real VCS or filesystem state.

use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::parallel::analysis_signature::{AnalysisInputProbe, DEGRADED_SUPPRESSION_TTL};
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::dynamic_queue::ReanalysisReason;
use crate::parallel::queue_state::ReanalysisDispatchContext;
use crate::parallel::{ParallelExecutor, WorkspaceResult};
use crate::vcs::VcsBackend;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// The scheduler's ordinary debounce timer branch duration.
const SCHEDULER_TIMER: Duration = Duration::from_millis(500);

/// The existing queue-coalescing debounce window, which also bounds probe cadence.
const QUEUE_DEBOUNCE: Duration = Duration::from_secs(10);

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

/// Deterministic stand-in for the repository probe behind an analysis-input signature.
///
/// It records how often each kind of probe ran, so tests can prove a suppressed 500 ms wake
/// performs no proposal read and spawns no VCS revision lookup.
struct FakeAnalysisInputProbe {
    revision: StdMutex<String>,
    digests: StdMutex<HashMap<String, String>>,
    revision_failure: StdMutex<Option<String>>,
    digest_failure: StdMutex<Option<String>>,
    revision_probes: AtomicUsize,
    digest_probes: AtomicUsize,
}

impl FakeAnalysisInputProbe {
    fn new(revision: &str) -> Arc<Self> {
        Arc::new(Self {
            revision: StdMutex::new(revision.to_string()),
            digests: StdMutex::new(HashMap::new()),
            revision_failure: StdMutex::new(None),
            digest_failure: StdMutex::new(None),
            revision_probes: AtomicUsize::new(0),
            digest_probes: AtomicUsize::new(0),
        })
    }

    fn set_revision(&self, revision: &str) {
        *self.revision.lock().expect("revision lock") = revision.to_string();
    }

    /// Mirror the production encoding, which names the effective dependency-base ref alongside
    /// the revision that ref resolves to.
    fn set_effective_base(&self, base_ref: &str, revision: &str) {
        self.set_revision(&format!("{base_ref}@{revision}"));
    }

    fn clear_revision_failure(&self) {
        *self.revision_failure.lock().expect("revision failure lock") = None;
    }

    fn set_proposal_digest(&self, change_id: &str, digest: &str) {
        self.digests
            .lock()
            .expect("digest lock")
            .insert(change_id.to_string(), digest.to_string());
    }

    fn fail_revision(&self, error: &str) {
        *self.revision_failure.lock().expect("revision failure lock") = Some(error.to_string());
    }

    fn fail_proposal_digest(&self, error: &str) {
        *self.digest_failure.lock().expect("digest failure lock") = Some(error.to_string());
    }

    fn revision_probes(&self) -> usize {
        self.revision_probes.load(Ordering::SeqCst)
    }

    fn digest_probes(&self) -> usize {
        self.digest_probes.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl AnalysisInputProbe for FakeAnalysisInputProbe {
    async fn base_revision(&self) -> Result<String, String> {
        self.revision_probes.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self
            .revision_failure
            .lock()
            .expect("revision failure lock")
            .clone()
        {
            return Err(error);
        }
        Ok(self.revision.lock().expect("revision lock").clone())
    }

    fn proposal_digest(&self, change_id: &str) -> Result<String, String> {
        self.digest_probes.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self
            .digest_failure
            .lock()
            .expect("digest failure lock")
            .clone()
        {
            return Err(error);
        }
        Ok(self
            .digests
            .lock()
            .expect("digest lock")
            .get(change_id)
            .cloned()
            .unwrap_or_else(|| format!("digest-of-{change_id}")))
    }
}

/// The analyzer callback's return type, as required by the scheduler loop.
type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

/// Mutable script for the analyzer test double.
///
/// Every knob mirrors something the real analyzer can do: how long it takes, whether its result
/// is healthy / intentionally metadata-only / a recoverable-failure fallback, what dependencies
/// it reports, whether it returns an unusable empty order, and a side effect that happens
/// *while* the analysis is in flight.
struct AnalyzerScript {
    invocations: AtomicUsize,
    duration: StdMutex<Duration>,
    provenance: StdMutex<AnalysisProvenance>,
    dependencies: StdMutex<HashMap<String, Vec<String>>>,
    empty_order: StdMutex<bool>,
    during_analysis: StdMutex<Option<Box<dyn Fn() + Send + Sync>>>,
    /// In-flight ID lists exactly as the analyzer received them, one entry per invocation.
    observed_in_flight: StdMutex<Vec<Vec<String>>>,
}

impl AnalyzerScript {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            invocations: AtomicUsize::new(0),
            duration: StdMutex::new(Duration::from_secs(1)),
            provenance: StdMutex::new(AnalysisProvenance::HealthyLlm),
            dependencies: StdMutex::new(HashMap::new()),
            empty_order: StdMutex::new(false),
            during_analysis: StdMutex::new(None),
            observed_in_flight: StdMutex::new(Vec::new()),
        })
    }

    fn observed_in_flight(&self) -> Vec<Vec<String>> {
        self.observed_in_flight
            .lock()
            .expect("observed in-flight lock")
            .clone()
    }

    fn invocations(&self) -> usize {
        self.invocations.load(Ordering::SeqCst)
    }

    fn set_duration(&self, duration: Duration) {
        *self.duration.lock().expect("duration lock") = duration;
    }

    fn set_provenance(&self, provenance: AnalysisProvenance) {
        *self.provenance.lock().expect("provenance lock") = provenance;
    }

    fn set_dependencies(&self, dependencies: HashMap<String, Vec<String>>) {
        *self.dependencies.lock().expect("dependencies lock") = dependencies;
    }

    fn set_empty_order(&self, empty_order: bool) {
        *self.empty_order.lock().expect("empty order lock") = empty_order;
    }

    fn run_during_analysis(&self, effect: impl Fn() + Send + Sync + 'static) {
        *self.during_analysis.lock().expect("during analysis lock") = Some(Box::new(effect));
    }
}

/// Analyzer test double that counts started analyses and honours [`AnalyzerScript`].
fn scripted_analyzer(
    script: Arc<AnalyzerScript>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |changes: &[Change], in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        script.invocations.fetch_add(1, Ordering::SeqCst);
        script
            .observed_in_flight
            .lock()
            .expect("observed in-flight lock")
            .push(in_flight.to_vec());
        let order: Vec<String> = if *script.empty_order.lock().expect("empty order lock") {
            Vec::new()
        } else {
            changes.iter().map(|change| change.id.clone()).collect()
        };
        let dependencies = script
            .dependencies
            .lock()
            .expect("dependencies lock")
            .clone();
        let provenance = *script.provenance.lock().expect("provenance lock");
        let duration = *script.duration.lock().expect("duration lock");
        if let Some(effect) = script
            .during_analysis
            .lock()
            .expect("during analysis lock")
            .as_ref()
        {
            effect();
        }

        Box::pin(async move {
            // Mocked analysis duration. Under paused time this also advances the clock, which is
            // how the "analysis took longer than the debounce window" variant is expressed.
            tokio::time::sleep(duration).await;
            AnalysisOutcome::new(
                AnalysisResult {
                    order,
                    dependencies,
                    groups: None,
                },
                provenance,
            )
        })
    }
}

/// Deterministic harness owning exactly the scheduler-loop state that unchanged-input
/// suppression depends on, replayed through the production loop step.
struct SuppressionHarness {
    executor: ParallelExecutor,
    queued: Vec<Change>,
    in_flight: HashSet<String>,
    join_set: JoinSet<WorkspaceResult>,
    cleanup_guard: WorkspaceCleanupGuard,
    semaphore: Arc<Semaphore>,
    reanalysis_reason: ReanalysisReason,
    iteration: u32,
    max_parallelism: usize,
    probe: Arc<FakeAnalysisInputProbe>,
    script: Arc<AnalyzerScript>,
    events: mpsc::Receiver<ExecutionEvent>,
    /// Keeps the isolated acceptance-stall state root alive for the harness's lifetime.
    _stall_state_root: TempDir,
}

impl SuppressionHarness {
    fn new(repo_root: std::path::PathBuf, queued: Vec<Change>) -> Self {
        Self::with_config(repo_root, queued, create_test_config(), 1)
    }

    fn with_config(
        repo_root: std::path::PathBuf,
        queued: Vec<Change>,
        config: OrchestratorConfig,
        max_parallelism: usize,
    ) -> Self {
        let (tx, events) = mpsc::channel(256);
        let mut executor = ParallelExecutor::new(repo_root.clone(), config, Some(tx));
        let probe = FakeAnalysisInputProbe::new("base-rev-1");
        executor.set_analysis_input_probe(probe.clone());
        // Isolate acceptance-stall state so queue classification never touches (or is slowed by)
        // the developer's real Conflux state directory.
        let stall_state_root = TempDir::new().expect("create stall state root");
        executor.set_acceptance_stall_state_root(stall_state_root.path().to_path_buf());
        Self {
            executor,
            queued,
            in_flight: HashSet::new(),
            join_set: JoinSet::new(),
            cleanup_guard: WorkspaceCleanupGuard::new(VcsBackend::Git, repo_root),
            semaphore: Arc::new(Semaphore::new(max_parallelism)),
            reanalysis_reason: ReanalysisReason::Initial,
            // Iteration 1 unconditionally skips debounce, so these tests start where the live
            // scheduler already ran its first analysis.
            iteration: 2,
            max_parallelism,
            probe,
            script: AnalyzerScript::new(),
            events,
            _stall_state_root: stall_state_root,
        }
    }

    /// Reproduce the live state: the queue changed long ago, so the one-way debounce check
    /// passes on every later wake and cannot establish quiescence by itself.
    async fn make_queue_debounce_stale(&self) {
        let mut last_change = self.executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now() - Duration::from_secs(600));
    }

    /// Occupy all dispatch capacity, as the live run's in-flight changes did.
    fn occupy_all_capacity(&mut self) -> Arc<AtomicUsize> {
        let counter = Arc::new(AtomicUsize::new(self.max_parallelism));
        self.executor.set_manual_resolve_counter(counter.clone());
        counter
    }

    /// One scheduler loop iteration's queued re-analysis/dispatch evaluation, including the
    /// loop-owned consumption of one-shot edge triggers.
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
            .expect("scheduler re-analysis evaluation must not fail");

        if let Some((_, new_iteration)) = outcome {
            self.iteration = new_iteration;
        }
        outcome
    }

    /// The scheduler's plain 500 ms timer branch: it wakes the loop and contributes no new
    /// re-analysis reason.
    async fn timer_wake(&self) {
        tokio::time::sleep(SCHEDULER_TIMER).await;
    }

    /// Run `wakes` ordinary timer iterations with no state change at all.
    async fn timer_wakes<F>(&mut self, analyzer: &F, wakes: usize)
    where
        for<'a> F: Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync,
    {
        for _ in 0..wakes {
            self.timer_wake().await;
            self.run_loop_iteration(analyzer).await;
        }
    }

    /// Let the bounded suppressed-state probe deadline expire.
    async fn pass_probe_deadline(&self) {
        tokio::time::sleep(QUEUE_DEBOUNCE + SCHEDULER_TIMER).await;
    }

    fn deliver_edge(&mut self, reason: ReanalysisReason) {
        self.reanalysis_reason = reason;
    }

    fn analyses(&self) -> usize {
        self.script.invocations()
    }

    fn drain_logs(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            if let ExecutionEvent::Log(entry) = event {
                messages.push(entry.message);
            }
        }
        messages
    }

    /// Count both operator-visible analysis attempts and apply starts in one drain, since both
    /// arrive on the same event channel.
    fn drain_attempt_counts(&mut self) -> (usize, usize) {
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
}

/// Wakes spanning more than the bounded probe interval, so the suppressed-state re-probe path is
/// exercised rather than only the first suppression.
///
/// Each scheduler pass costs one `git worktree list` subprocess per queued change through the
/// existing acceptance-stall reconciliation, so wake count and queue size both have to stay modest
/// to respect the repository's one-second unit-test duration policy. The wake count is what this
/// regression turns on; a stable *multi-change* queue is covered separately by
/// [`stable_multi_change_queue_is_analyzed_once`].
const TIMER_WAKES: usize = 24;

/// The live no-progress loop: stale debounce, stable queued work, no capacity, only 500 ms
/// timer wakes.
async fn assert_unchanged_timer_wakes_analyze_once(analysis_duration: Duration) {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    harness.script.set_duration(analysis_duration);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    let occupancy = harness.occupy_all_capacity();

    // The first ordinary timer evaluation analyzes the input once.
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        1,
        "an ordinary timer evaluation of a never-analyzed input must analyze once"
    );

    // Many later wakes change nothing: same queued set, same in-flight set, same capacity, same
    // proposals, same effective base revision.
    harness.timer_wakes(&analyzer, TIMER_WAKES).await;

    assert_eq!(
        occupancy.load(Ordering::SeqCst),
        1,
        "the scenario must hold dispatch capacity at zero throughout"
    );
    assert_eq!(
        harness.analyses(),
        1,
        "unchanged timer wakes must not invoke the analyzer again; saw {} analyses",
        harness.analyses()
    );
    assert_eq!(
        harness.queued.len(),
        1,
        "queued work must be retained while capacity is zero"
    );
    let (analysis_started, apply_started) = harness.drain_attempt_counts();
    assert_eq!(
        apply_started, 0,
        "a suppressed pass must not dispatch apply work"
    );
    assert_eq!(
        analysis_started, 1,
        "a pass suppressed before analyzer invocation must not surface as a distinct analysis \
         attempt in operator-visible output"
    );
}

#[tokio::test(start_paused = true)]
async fn unchanged_timer_wakes_analyze_once_when_analysis_is_shorter_than_debounce() {
    assert_unchanged_timer_wakes_analyze_once(Duration::from_secs(2)).await;
}

#[tokio::test(start_paused = true)]
async fn unchanged_timer_wakes_analyze_once_when_analysis_is_longer_than_debounce() {
    // The live evidence showed 20-42 second analyses. Suppression must not depend on the
    // analyzer finishing inside the ten-second debounce window.
    assert_unchanged_timer_wakes_analyze_once(Duration::from_secs(25)).await;
}

#[tokio::test(start_paused = true)]
async fn stable_multi_change_queue_is_analyzed_once() {
    // The live run had many queued changes competing for capacity held by in-flight work.
    let temp_dir = TempDir::new().unwrap();
    let queued: Vec<Change> = (0..4)
        .map(|index| test_change(&format!("queued-{index:02}")))
        .collect();
    let mut harness = SuppressionHarness::new(temp_dir.path().to_path_buf(), queued);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    harness.timer_wakes(&analyzer, 6).await;

    assert_eq!(
        harness.analyses(),
        1,
        "a stable multi-change queue must be analyzed once, not once per wake"
    );
    assert_eq!(
        harness.queued.len(),
        4,
        "every queued change must be retained while capacity is zero"
    );
}

#[tokio::test(start_paused = true)]
async fn suppressed_wakes_do_not_probe_before_the_bounded_deadline() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);

    // Recording the completed input also arms its probe deadline, so the immediate 500 ms wake
    // performs no repository I/O at all.
    let probes_after_analysis = harness.probe.revision_probes();
    let digest_probes_after_analysis = harness.probe.digest_probes();
    harness.timer_wake().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.probe.revision_probes(),
        probes_after_analysis,
        "the wake immediately after a completed analysis must not re-probe the VCS revision"
    );
    assert_eq!(
        harness.probe.digest_probes(),
        digest_probes_after_analysis,
        "the wake immediately after a completed analysis must not re-read proposal files"
    );

    // Ten seconds of 500 ms wakes must add no probes at all.
    harness.timer_wakes(&analyzer, 18).await;

    assert_eq!(
        harness.probe.revision_probes(),
        probes_after_analysis,
        "a suppressed 500 ms wake must not spawn a VCS revision probe"
    );
    assert_eq!(
        harness.probe.digest_probes(),
        digest_probes_after_analysis,
        "a suppressed 500 ms wake must not re-read proposal files"
    );

    // After the bounded deadline, repository-visible input is confirmed again exactly once.
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.probe.revision_probes(),
        probes_after_analysis + 1,
        "the first evaluation after the probe deadline must re-confirm repository-visible input"
    );
    assert_eq!(
        harness.analyses(),
        1,
        "re-probing an unchanged input must not invoke the analyzer"
    );
    let logs = harness.drain_logs();
    assert!(
        logs.iter()
            .any(|message| message.contains("unchanged_analysis_input")),
        "suppression must be observable as a deduplicated operator-visible reason; saw {logs:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn queue_addition_bypasses_a_matching_signature_once() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    harness.timer_wakes(&analyzer, 4).await;
    assert_eq!(
        harness.analyses(),
        1,
        "the unchanged input must be suppressed before the queue event"
    );

    // A real queue notification must analyze immediately even though the queued set, capacity,
    // and repository state are all unchanged.
    harness.deliver_edge(ReanalysisReason::QueueNotification);
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        2,
        "a queue notification must not be suppressed by a prior matching signature"
    );

    // Once the event is consumed, timer-only wakes return to unchanged-input suppression.
    harness.deliver_edge(ReanalysisReason::Initial);
    harness.timer_wakes(&analyzer, 6).await;
    assert_eq!(
        harness.analyses(),
        2,
        "timer wakes after the consumed event must stay quiescent"
    );
}

#[tokio::test(start_paused = true)]
async fn each_explicit_edge_bypasses_a_matching_signature_once() {
    for edge in [
        ReanalysisReason::ResolveCompletion,
        ReanalysisReason::RepairCandidate,
        ReanalysisReason::SlotRecovery,
    ] {
        let temp_dir = TempDir::new().unwrap();
        let mut harness =
            SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
        let analyzer = scripted_analyzer(harness.script.clone());
        harness.make_queue_debounce_stale().await;
        harness.occupy_all_capacity();

        harness.run_loop_iteration(&analyzer).await;
        harness.timer_wakes(&analyzer, 4).await;
        assert_eq!(
            harness.analyses(),
            1,
            "{edge}: the unchanged input must be suppressed before the edge"
        );

        harness.deliver_edge(edge);
        harness.run_loop_iteration(&analyzer).await;
        assert_eq!(
            harness.analyses(),
            2,
            "{edge}: an explicit edge must analyze once even with a matching signature"
        );
        assert_eq!(
            harness.reanalysis_reason,
            ReanalysisReason::Initial,
            "{edge}: the one-shot edge must still be consumed after evaluation"
        );

        harness.timer_wakes(&analyzer, 6).await;
        assert_eq!(
            harness.analyses(),
            2,
            "{edge}: timer wakes must not replay the consumed edge"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn capacity_recovery_rearms_analysis_without_a_slot_recovery_reason() {
    let repo_dir = TempDir::new().unwrap();
    let workspace_base = TempDir::new().unwrap();
    init_minimal_git_repo(repo_dir.path());

    let config = OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..create_test_config()
    };
    let mut harness = SuppressionHarness::with_config(
        repo_dir.path().to_path_buf(),
        vec![test_change("queued-a")],
        config,
        1,
    );
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    let occupancy = harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    harness.timer_wakes(&analyzer, 4).await;
    assert_eq!(harness.analyses(), 1, "zero-capacity input analyzed once");
    assert!(
        harness.in_flight.is_empty(),
        "dispatch stays suppressed while capacity is zero"
    );

    // Capacity recovers, but no slot-recovery reason is delivered: the timer fallback alone must
    // notice the changed input.
    occupancy.store(0, Ordering::SeqCst);
    harness.pass_probe_deadline().await;
    let (should_break, _iteration) = harness
        .run_loop_iteration(&analyzer)
        .await
        .expect("queued work must be evaluated");

    assert!(!should_break, "capacity recovery must resume the scheduler");
    assert_eq!(
        harness.reanalysis_reason,
        ReanalysisReason::Initial,
        "this variant must not rely on a retained SlotRecovery reason"
    );
    assert_eq!(
        harness.analyses(),
        2,
        "changed capacity must re-arm dependency analysis"
    );
    assert!(
        harness.queued.is_empty(),
        "eligible queued work must reach dispatch evaluation after capacity recovery"
    );
    assert_eq!(
        harness.in_flight.len(),
        1,
        "the recovered slot must be used by the queued change"
    );

    harness.join_set.abort_all();
    while harness.join_set.join_next().await.is_some() {}
}

#[tokio::test(start_paused = true)]
async fn same_id_queued_proposal_change_rearms_analysis() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    harness.timer_wakes(&analyzer, 4).await;
    assert_eq!(harness.analyses(), 1);

    // The queued ID set is unchanged; only the proposal content the analyzer reads changed.
    harness.probe.set_proposal_digest("queued-a", "edited");
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        2,
        "an edited proposal with the same change ID must re-arm analysis"
    );
}

#[tokio::test(start_paused = true)]
async fn same_id_in_flight_proposal_change_rearms_analysis() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness = SuppressionHarness::with_config(
        temp_dir.path().to_path_buf(),
        vec![test_change("queued-a")],
        create_test_config(),
        2,
    );
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();
    harness.in_flight.insert("inflight-a".to_string());

    harness.run_loop_iteration(&analyzer).await;
    harness.timer_wakes(&analyzer, 4).await;
    assert_eq!(harness.analyses(), 1);

    // In-flight proposals are emitted in the analyzer prompt as dependency context, so their
    // content is part of the analysis input too.
    harness.probe.set_proposal_digest("inflight-a", "edited");
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        2,
        "an edited in-flight proposal must re-arm analysis"
    );
}

#[tokio::test(start_paused = true)]
async fn effective_base_revision_change_rearms_analysis() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    harness.timer_wakes(&analyzer, 4).await;
    assert_eq!(harness.analyses(), 1);

    // A dependency was integrated into the base: same IDs, new repository evidence.
    harness.probe.set_revision("base-rev-2");
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        2,
        "a changed effective dependency-base revision must re-arm analysis"
    );
}

#[tokio::test(start_paused = true)]
async fn input_change_during_analysis_is_visible_at_the_next_probe() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let probe = harness.probe.clone();
    // The proposal is edited while the analysis is still running, so the post-analysis state
    // differs from the input that was actually analyzed.
    harness.script.run_during_analysis(move || {
        probe.set_proposal_digest("queued-a", "edited-during-analysis");
    });
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);

    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        2,
        "recording the pre-analysis snapshot must keep a change made during analysis visible"
    );
}

#[tokio::test(start_paused = true)]
async fn fresh_executor_starts_without_a_prior_analysis_signature() {
    let temp_dir = TempDir::new().unwrap();
    let queued = vec![test_change("queued-a")];

    // A first process analyzes the input and then suppresses unchanged wakes.
    let mut first = SuppressionHarness::new(temp_dir.path().to_path_buf(), queued.clone());
    let first_analyzer = scripted_analyzer(first.script.clone());
    first.make_queue_debounce_stale().await;
    first.occupy_all_capacity();
    first.run_loop_iteration(&first_analyzer).await;
    first.timer_wakes(&first_analyzer, 4).await;
    assert_eq!(first.analyses(), 1);

    // A restarted scheduler evaluating the same workspace state must analyze again: suppression
    // state is process-local and is never read back from logs or caches.
    let mut restarted = SuppressionHarness::new(temp_dir.path().to_path_buf(), queued);
    let restarted_analyzer = scripted_analyzer(restarted.script.clone());
    restarted.make_queue_debounce_stale().await;
    restarted.occupy_all_capacity();
    restarted.run_loop_iteration(&restarted_analyzer).await;

    assert_eq!(
        restarted.analyses(),
        1,
        "a fresh executor must perform its own initial analysis"
    );
}

#[tokio::test(start_paused = true)]
async fn degraded_fallback_suppresses_rapid_retries_and_permits_one_retry_after_five_minutes() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    harness
        .script
        .set_provenance(AnalysisProvenance::RecoverableFailureFallback);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        1,
        "a recoverable-failure fallback is still a usable degraded result"
    );

    // Just under five minutes of wakes must not relaunch the failing analyzer command.
    for _ in 0..8 {
        tokio::time::sleep(Duration::from_secs(30)).await;
        harness.run_loop_iteration(&analyzer).await;
    }
    assert_eq!(
        harness.analyses(),
        1,
        "a broken analyzer command must not be relaunched on every timer wake; saw {} analyses",
        harness.analyses()
    );

    // Once the fixed degraded interval expires, exactly one retry becomes eligible.
    tokio::time::sleep(DEGRADED_SUPPRESSION_TTL).await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        2,
        "the degraded record must expire and permit one retry"
    );

    harness.timer_wakes(&analyzer, 6).await;
    assert_eq!(
        harness.analyses(),
        2,
        "the retry must re-arm a new bounded window rather than a rapid loop"
    );
}

#[tokio::test(start_paused = true)]
async fn healthy_result_after_a_degraded_record_becomes_non_expiring() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    harness
        .script
        .set_provenance(AnalysisProvenance::RecoverableFailureFallback);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);

    // The analyzer recovers on the retry that the degraded interval permits.
    harness
        .script
        .set_provenance(AnalysisProvenance::HealthyLlm);
    tokio::time::sleep(DEGRADED_SUPPRESSION_TTL).await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 2, "one retry must be permitted");

    // A healthy unchanged input stays quiescent instead of retrying every five minutes.
    for _ in 0..3 {
        tokio::time::sleep(DEGRADED_SUPPRESSION_TTL * 2).await;
        harness.run_loop_iteration(&analyzer).await;
    }
    assert_eq!(
        harness.analyses(),
        2,
        "a healthy result must record a non-expiring signature; saw {} analyses",
        harness.analyses()
    );
}

#[tokio::test(start_paused = true)]
async fn intentional_metadata_only_result_is_not_treated_as_degraded() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    harness
        .script
        .set_provenance(AnalysisProvenance::IntentionalMetadataOnly);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);

    for _ in 0..3 {
        tokio::time::sleep(DEGRADED_SUPPRESSION_TTL * 2).await;
        harness.run_loop_iteration(&analyzer).await;
    }

    assert_eq!(
        harness.analyses(),
        1,
        "configured metadata-only analysis is the intended result and must not expire"
    );
}

#[tokio::test(start_paused = true)]
async fn unusable_empty_result_establishes_no_completed_signature() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    harness.script.set_empty_order(true);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);

    // An analyzer path that terminates before producing a usable decision must not suppress the
    // next eligible attempt.
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        2,
        "an unusable empty result must not falsely establish a completed input"
    );
}

#[tokio::test(start_paused = true)]
async fn zero_dispatch_with_positive_capacity_and_idle_scheduler_stays_eligible() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    // An erroneous dependency result that the change itself never declared: classification still
    // sees a dispatchable candidate, but dispatch selection finds nothing to start.
    harness.script.set_dependencies(HashMap::from([(
        "queued-a".to_string(),
        vec!["hallucinated-dependency".to_string()],
    )]));
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);
    assert!(
        harness.in_flight.is_empty(),
        "the erroneous dependency must block dispatch"
    );
    assert_eq!(
        harness.queued.len(),
        1,
        "the undispatched change stays queued"
    );

    // Positive capacity, nothing in flight, nothing dispatched: a non-deterministic bad result
    // must not be able to freeze the scheduler permanently.
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        2,
        "a zero-dispatch idle pass must stay eligible for the next debounced analysis"
    );
}

#[tokio::test(start_paused = true)]
async fn revision_probe_failure_fails_open_without_recording_suppression() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();
    harness.probe.fail_revision("revision resolution failed");

    // Every eligible evaluation must reach the analyzer, because a signature that cannot be built
    // must never suppress work.
    for _ in 0..3 {
        harness.pass_probe_deadline().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    assert_eq!(
        harness.analyses(),
        3,
        "signature construction failure must fail open instead of suppressing analysis"
    );
    let logs = harness.drain_logs();
    assert!(
        logs.iter()
            .any(|message| message.contains("signature unavailable")),
        "the fail-open path must stay operator-visible; saw {logs:?}"
    );

    // Recovery re-establishes ordinary suppression without any user action.
    harness.probe.set_revision("base-rev-recovered");
    harness.probe.clear_revision_failure();
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 4);
    harness.timer_wakes(&analyzer, 6).await;
    assert_eq!(
        harness.analyses(),
        4,
        "once a signature can be built again, unchanged wakes must be suppressed"
    );
}

#[tokio::test(start_paused = true)]
async fn effective_base_ref_change_rearms_analysis() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();
    harness.probe.set_effective_base("main", "commit-1");

    harness.run_loop_iteration(&analyzer).await;
    harness.timer_wakes(&analyzer, 4).await;
    assert_eq!(harness.analyses(), 1);

    // A stacked run switched the effective dependency base to an integration branch. The commit
    // it currently resolves to is unchanged, so only the *named* base differs; merge evidence is
    // now read from a different ref and must be re-evaluated.
    harness
        .probe
        .set_effective_base("integration-1", "commit-1");
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        2,
        "a changed effective dependency-base ref must re-arm analysis"
    );

    // And the ordinary case: the same named ref advances on its own.
    harness.timer_wakes(&analyzer, 4).await;
    assert_eq!(
        harness.analyses(),
        2,
        "the new base must then be suppressed"
    );
    harness
        .probe
        .set_effective_base("integration-1", "commit-2");
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;

    assert_eq!(
        harness.analyses(),
        3,
        "an advancing effective dependency-base ref must re-arm analysis"
    );
}

#[tokio::test(start_paused = true)]
async fn persistent_signature_failure_is_rate_limited_across_timer_wakes() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();
    harness.probe.fail_revision("revision resolution failed");

    // The first eligible evaluation fails open: it analyzes despite having no signature.
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        1,
        "an unavailable signature must permit analysis"
    );
    let probes_after_first_attempt = harness.probe.revision_probes();

    // Eight seconds of 500 ms wakes while the failure persists must add no probe and no
    // analysis: fail-open must not become fail-hot.
    harness.timer_wakes(&analyzer, 16).await;
    assert_eq!(
        harness.analyses(),
        1,
        "a persistent signature failure must not relaunch the analyzer on every 500 ms wake; saw \
         {} analyses",
        harness.analyses()
    );
    assert_eq!(
        harness.probe.revision_probes(),
        probes_after_first_attempt,
        "a throttled wake must not re-probe repository-visible signature material"
    );
    let logs = harness.drain_logs();
    assert!(
        logs.iter()
            .any(|message| message.contains("analysis_signature_unavailable_retry_pending")),
        "the bounded fail-open retry must stay operator-visible; saw {logs:?}"
    );

    // After the ten-second cadence, exactly one probe and one analysis are retried.
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        2,
        "the deadline must permit one retry of both signature construction and analysis"
    );
    assert_eq!(
        harness.probe.revision_probes(),
        probes_after_first_attempt + 1,
        "the retry must probe exactly once"
    );

    harness.timer_wakes(&analyzer, 6).await;
    assert_eq!(
        harness.analyses(),
        2,
        "the retry must re-arm the bounded window rather than a rapid loop"
    );
    assert!(
        !harness.queued.is_empty(),
        "a fail-open pass must keep queued work alive"
    );
}

#[tokio::test(start_paused = true)]
async fn explicit_edge_bypasses_the_signature_failure_retry_deadline_once() {
    for edge in [
        ReanalysisReason::QueueNotification,
        ReanalysisReason::ResolveCompletion,
        ReanalysisReason::RepairCandidate,
        ReanalysisReason::SlotRecovery,
    ] {
        let temp_dir = TempDir::new().unwrap();
        let mut harness =
            SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
        let analyzer = scripted_analyzer(harness.script.clone());
        harness.make_queue_debounce_stale().await;
        harness.occupy_all_capacity();
        harness.probe.fail_revision("revision resolution failed");

        harness.run_loop_iteration(&analyzer).await;
        harness.timer_wakes(&analyzer, 3).await;
        assert_eq!(
            harness.analyses(),
            1,
            "{edge}: the bounded retry deadline must hold before the edge"
        );

        // A real state-transition event is worth one immediate attempt even though the retry
        // deadline has not elapsed.
        harness.deliver_edge(edge);
        harness.run_loop_iteration(&analyzer).await;
        assert_eq!(
            harness.analyses(),
            2,
            "{edge}: an explicit edge must bypass the bounded retry deadline once"
        );

        // Once consumed, ordinary wakes return to the bounded cadence.
        harness.deliver_edge(ReanalysisReason::Initial);
        harness.timer_wakes(&analyzer, 4).await;
        assert_eq!(
            harness.analyses(),
            2,
            "{edge}: timer wakes after the event must return to bounded retry"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn signature_probe_recovery_reestablishes_suppression_without_a_queue_change() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();
    harness.probe.fail_revision("revision resolution failed");

    harness.run_loop_iteration(&analyzer).await;
    harness.timer_wakes(&analyzer, 4).await;
    assert_eq!(harness.analyses(), 1, "the failing probe fails open once");

    // The probe recovers on its own. Queue membership, capacity, and proposal content never
    // changed, so recovery must not depend on any new operator or scheduler event.
    harness.probe.clear_revision_failure();
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        2,
        "the first eligible evaluation after recovery must analyze once"
    );
    assert_eq!(
        harness.reanalysis_reason,
        ReanalysisReason::Initial,
        "recovery must not rely on an explicit edge"
    );

    // The recovered signature is a real completed input, so unchanged wakes go quiet again.
    harness.timer_wakes(&analyzer, 6).await;
    assert_eq!(
        harness.analyses(),
        2,
        "a recovered signature must re-establish ordinary suppression"
    );
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        2,
        "re-probing the recovered unchanged input must not invoke the analyzer"
    );
}

#[tokio::test(start_paused = true)]
async fn unusable_empty_result_retries_at_the_bounded_cadence() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    harness.script.set_empty_order(true);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);

    // An unusable result records nothing, but it must not turn into a 500 ms analyzer loop
    // either.
    harness.timer_wakes(&analyzer, 18).await;
    assert_eq!(
        harness.analyses(),
        1,
        "a persistently unusable analyzer must not be relaunched on every wake; saw {} analyses",
        harness.analyses()
    );

    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        2,
        "the bounded deadline must permit one retry of the unusable input"
    );
}

#[tokio::test(start_paused = true)]
async fn degraded_expiry_is_not_delayed_by_the_repository_probe_cadence() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    harness
        .script
        .set_provenance(AnalysisProvenance::RecoverableFailureFallback);
    // Keep the analyzer instantaneous so the degraded record's expiry is exactly five paused
    // minutes after the wake that records it.
    harness.script.set_duration(Duration::ZERO);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);

    // A repository probe lands one second before expiry. Arming a fresh ten-second probe
    // deadline there would push the promised retry out to 5m09s.
    tokio::time::sleep(DEGRADED_SUPPRESSION_TTL - Duration::from_secs(1)).await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        1,
        "the degraded record must still suppress one second before expiry"
    );

    // The first eligible wake at exactly five minutes must perform the promised retry.
    tokio::time::sleep(Duration::from_secs(1)).await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        2,
        "a probe taken just before expiry must not delay the degraded retry"
    );
}

#[tokio::test(start_paused = true)]
async fn in_flight_order_permutations_produce_one_deterministic_analyzer_input() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness = SuppressionHarness::with_config(
        temp_dir.path().to_path_buf(),
        vec![test_change("queued-a")],
        create_test_config(),
        4,
    );
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();
    for id in ["inflight-c", "inflight-a", "inflight-b"] {
        harness.in_flight.insert(id.to_string());
    }

    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(harness.analyses(), 1);

    // Rebuild the same in-flight set through a different insertion order. `HashSet` iteration is
    // not a repository-visible change, so it must neither invalidate the signature nor reorder
    // the analyzer prompt.
    harness.in_flight.clear();
    for id in ["inflight-b", "inflight-c", "inflight-a"] {
        harness.in_flight.insert(id.to_string());
    }
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        1,
        "an order-only permutation of the same in-flight set is one semantic input"
    );

    // A real membership change still re-arms analysis.
    harness.in_flight.insert("inflight-d".to_string());
    harness.pass_probe_deadline().await;
    harness.run_loop_iteration(&analyzer).await;
    assert_eq!(
        harness.analyses(),
        2,
        "a real in-flight membership change must re-arm analysis"
    );

    for observed in harness.script.observed_in_flight() {
        let mut sorted = observed.clone();
        sorted.sort();
        assert_eq!(
            observed, sorted,
            "the analyzer must receive one deterministic ordering of the in-flight set"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn proposal_read_failure_fails_open_without_recording_suppression() {
    let temp_dir = TempDir::new().unwrap();
    let mut harness =
        SuppressionHarness::new(temp_dir.path().to_path_buf(), vec![test_change("queued-a")]);
    let analyzer = scripted_analyzer(harness.script.clone());
    harness.make_queue_debounce_stale().await;
    harness.occupy_all_capacity();
    harness
        .probe
        .fail_proposal_digest("proposal.md could not be read");

    for _ in 0..3 {
        harness.pass_probe_deadline().await;
        harness.run_loop_iteration(&analyzer).await;
    }

    assert_eq!(
        harness.analyses(),
        3,
        "a proposal read failure must fail open instead of suppressing analysis"
    );
    assert_eq!(
        harness.queued.len(),
        1,
        "a fail-open pass must not lose queued work"
    );
}
