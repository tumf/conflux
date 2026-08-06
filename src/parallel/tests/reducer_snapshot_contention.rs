//! Scheduler-liveness regressions for transient reducer lock contention.
//!
//! One scheduler pass used to read reducer-owned work through several
//! independent `try_read` calls. Each failed closed on its own, but together a
//! single short reducer write could refuse a dynamic hint, return an empty
//! reconciliation, and report every candidate unavailable — erasing all
//! scheduler-local evidence of still-queued reducer intent. The scheduler then
//! treated that momentary result as a *stable* state: a finite run reported
//! `DrainedSuccessfully` or `BlockedOrStalled`, and a persistent run entered an
//! event-driven idle wait with no timer, so queued work could sit untouched
//! until some unrelated event happened to arrive.
//!
//! These tests pin the replacement contract: one awaited coherent snapshot per
//! evaluation, contention that only *delays*, and automatic continuation of the
//! same evaluation when the writer releases.
//!
//! # Why every contended call runs in a spawned task
//!
//! Awaiting the reducer read from the same task that holds the write guard is a
//! self-deadlock, not a contention test. Every scenario here therefore holds the
//! writer in the test task and drives the scheduler work from a separate task,
//! which is also what production does: the writer is another task.
//!
//! # Why dispatch is proven at the selection gate
//!
//! Letting the full loop dispatch would create real worktrees and spawn real
//! agent commands, which does not belong on the sub-second default test path.
//! Reaching the analyzer already proves classification produced a dispatchable
//! candidate, and
//! [`reducer_snapshot_contention_release_admits_the_candidate_to_dispatch`]
//! asserts the reducer-governed dispatch gate directly on the resumed snapshot.
//!
//! Tokio time is deliberately not paused: the scheduler loop shells out to git,
//! and paused-time auto-advance would fire the must-arrive timeouts while a
//! child process is still running. The only fixed wait is the short window in
//! which progress must *not* appear; every other wait is event-driven and
//! returns as soon as its condition holds.

use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::{ExecutionEvent, StalledBlocker};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::state::{OrchestratorState, ReducerCommand};
use crate::parallel::work_snapshot::ReducerWorkSnapshot;
use crate::parallel::{ParallelEvent, ParallelExecutor, SchedulerLifetime, SchedulerRunReport};
use crate::tui::queue::DynamicQueue;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio_util::sync::CancellationToken;

/// Bounded wait for something that must happen. Only reached once the assertion
/// has already failed, so it costs nothing on the passing path.
const MUST_HAPPEN: Duration = Duration::from_secs(5);

/// Window in which the scheduler must make no progress at all because reducer
/// evidence is unavailable.
const NO_PROGRESS_WINDOW: Duration = Duration::from_millis(120);

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
    assert!(output.status.success(), "git {:?} failed", args);
}

fn write_change(repo_root: &std::path::Path, change_id: &str) {
    let change_dir = repo_root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).expect("create change directory");
    std::fs::write(
        change_dir.join("proposal.md"),
        format!("---\ndependencies:\n---\n# {change_id}\n"),
    )
    .expect("write proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [ ] apply\n",
    )
    .expect("write tasks");
}

/// A committed repository holding one independent change.
fn init_repo(change_ids: &[&str]) -> TempDir {
    let repo = TempDir::new().expect("create temp repo");
    let root = repo.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.email", "test@example.com"]);
    git(root, &["config", "user.name", "Test User"]);
    std::fs::write(root.join("README.md"), "base\n").expect("write base file");
    for change_id in change_ids {
        write_change(root, change_id);
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "Base"]);
    repo
}

async fn reducer_state(known: &[&str], queued_ids: &[&str]) -> Arc<RwLock<OrchestratorState>> {
    let state = Arc::new(RwLock::new(OrchestratorState::new(
        known.iter().map(|id| id.to_string()).collect(),
        10,
    )));
    {
        let mut guard = state.write().await;
        for id in queued_ids {
            guard.apply_command(ReducerCommand::AddToQueue(id.to_string()));
        }
    }
    state
}

/// A validated-facts-free acceptance blocker, which the classifier keeps on the
/// Acceptance-owned `stalled` path rather than promoting to external `blocked`.
fn acceptance_hold(change_id: &str) -> ExecutionEvent {
    ExecutionEvent::AcceptanceGated {
        change_id: change_id.to_string(),
        blocker: StalledBlocker {
            category: "external_service".to_string(),
            phase: "acceptance".to_string(),
            gate: "acceptance".to_string(),
            error_summary: "waiting on an operator-owned prerequisite".to_string(),
            evidence: vec!["tasks.md: Implementation Blocker #1".to_string()],
            unblock_condition: None,
            prerequisite_owner: None,
            next_action: "operator clears the prerequisite".to_string(),
            resumable: true,
            worktree_preserved: true,
        },
    }
}

/// What the scheduler actually did, recorded from inside the analyzer.
///
/// The analyzer is the first thing a pass reaches that can only run on complete
/// reducer evidence, so its invocation count is the scheduler-path proof that
/// classification authorized work — and its zero value is the proof that
/// contention authorized none.
#[derive(Default)]
struct AnalyzerProbe {
    invocations: AtomicUsize,
    analyzed_ids: Mutex<Vec<Vec<String>>>,
    /// Set when the analyzer was able to take the reducer *write* lock while it
    /// ran, which is only possible if the scheduler released its read guard
    /// before starting analysis.
    reducer_writable_during_analysis: AtomicUsize,
    started: Notify,
}

impl AnalyzerProbe {
    fn invocations(&self) -> usize {
        self.invocations.load(Ordering::SeqCst)
    }

    fn analyzed_ids(&self) -> Vec<Vec<String>> {
        self.analyzed_ids.lock().expect("probe mutex").clone()
    }
}

/// Analyzer double that records what it was given, proves the reducer lock is
/// free while it runs, and returns an empty order so nothing is dispatched.
fn probing_analyzer(
    probe: Arc<AnalyzerProbe>,
    shared: Arc<RwLock<OrchestratorState>>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        let probe = probe.clone();
        let shared = shared.clone();
        let ids: Vec<String> = changes.iter().map(|change| change.id.clone()).collect();
        Box::pin(async move {
            probe.invocations.fetch_add(1, Ordering::SeqCst);
            probe
                .analyzed_ids
                .lock()
                .expect("probe mutex")
                .push(ids.clone());
            if shared.try_write().is_ok() {
                probe
                    .reducer_writable_during_analysis
                    .fetch_add(1, Ordering::SeqCst);
            }
            probe.started.notify_waiters();
            AnalysisOutcome::new(
                AnalysisResult {
                    order: Vec::new(),
                    dependencies: HashMap::new(),
                    groups: None,
                },
                AnalysisProvenance::HealthyLlm,
            )
        })
    }
}

/// Drain the scheduler event stream, recording the terminal events a false
/// drain or a false blocked exit would produce.
#[derive(Default)]
struct TerminalEvents {
    all_completed: AtomicUsize,
    stopped: AtomicUsize,
}

fn spawn_event_recorder(
    mut events: mpsc::Receiver<ParallelEvent>,
    recorded: Arc<TerminalEvents>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            match event {
                ParallelEvent::AllCompleted => {
                    recorded.all_completed.fetch_add(1, Ordering::SeqCst);
                }
                ParallelEvent::Stopped => {
                    recorded.stopped.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
        }
    })
}

/// Everything one contended scheduler run needs, wired but not yet started.
struct ContendedRun {
    executor: ParallelExecutor,
    shared: Arc<RwLock<OrchestratorState>>,
    queue: Arc<DynamicQueue>,
    cancel: CancellationToken,
    probe: Arc<AnalyzerProbe>,
    terminal: Arc<TerminalEvents>,
    _recorder: tokio::task::JoinHandle<()>,
    _workspace_base: TempDir,
}

fn wire_run(
    repo_root: &std::path::Path,
    shared: Arc<RwLock<OrchestratorState>>,
    lifetime: SchedulerLifetime,
) -> ContendedRun {
    let workspace_base = TempDir::new().expect("create workspace base");
    let (event_tx, events) = mpsc::channel(512);
    let mut executor = ParallelExecutor::new(
        repo_root.to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    let queue = Arc::new(DynamicQueue::new());
    let cancel = CancellationToken::new();
    executor.set_shared_orchestrator_state(shared.clone());
    executor.set_dynamic_queue(queue.clone());
    executor.set_scheduler_lifetime(lifetime);
    executor.set_cancel_token(cancel.clone());

    let terminal = Arc::new(TerminalEvents::default());
    let recorder = spawn_event_recorder(events, terminal.clone());

    ContendedRun {
        executor,
        probe: Arc::new(AnalyzerProbe::default()),
        shared,
        queue,
        cancel,
        terminal,
        _recorder: recorder,
        _workspace_base: workspace_base,
    }
}

/// A finite run whose scheduler-local queue starts empty while the reducer holds
/// real queued intent: contention must delay it, never drain it.
///
/// This is the reported stall in miniature. Before the awaited snapshot, an
/// unreadable reducer made the reducer look like it had no queued work at all,
/// and a finite run announced completion on the spot.
#[tokio::test]
async fn reducer_snapshot_contention_does_not_drain_or_strand_a_finite_scheduler() {
    let repo = init_repo(&["queued-a"]);
    let shared = reducer_state(&["queued-a"], &["queued-a"]).await;
    let run = wire_run(repo.path(), shared.clone(), SchedulerLifetime::Finite);
    let ContendedRun {
        mut executor,
        queue,
        cancel,
        probe,
        terminal,
        ..
    } = run;

    // The one wake edge this queued change has.
    assert!(queue.push("queued-a".to_string()).await);

    let contended = shared.write().await;
    let analyzer = probing_analyzer(probe.clone(), shared.clone());
    let mut scheduler = tokio::spawn(async move {
        // An empty initial candidate list: every candidate must come from
        // reducer-visible queue intent.
        executor
            .execute_with_order_based_reanalysis(Vec::new(), analyzer)
            .await
    });

    // Nothing may happen on incomplete reducer evidence: no analysis, no
    // dispatch, and above all no terminal report.
    assert!(
        tokio::time::timeout(NO_PROGRESS_WINDOW, &mut scheduler)
            .await
            .is_err(),
        "a finite scheduler must not return while reducer evidence is unavailable"
    );
    assert_eq!(
        probe.invocations(),
        0,
        "no dependency analysis may start from incomplete reducer evidence"
    );
    assert_eq!(
        terminal.all_completed.load(Ordering::SeqCst),
        0,
        "contention must never be announced as a completed run"
    );
    assert!(
        queue.contains("queued-a").await,
        "a hint the pass cannot yet judge keeps its wake edge instead of being consumed"
    );

    // Releasing the writer is the only thing that happens next: no queue
    // mutation, no notification, no cancellation.
    drop(contended);

    tokio::time::timeout(MUST_HAPPEN, probe.started.notified())
        .await
        .expect("the same evaluation must continue once the writer releases");

    assert_eq!(
        probe.analyzed_ids().first().cloned().unwrap_or_default(),
        vec!["queued-a".to_string()],
        "the reducer-queued candidate is reconciled into the scheduler-local queue and analyzed"
    );
    assert_eq!(
        probe
            .reducer_writable_during_analysis
            .load(Ordering::SeqCst),
        1,
        "the reducer read guard must be released before dependency analysis begins"
    );

    cancel.cancel();
    let report = tokio::time::timeout(MUST_HAPPEN, scheduler)
        .await
        .expect("cancelled scheduler must return")
        .expect("scheduler task must not panic")
        .expect("scheduler loop must not fail");
    assert_eq!(
        report,
        SchedulerRunReport::Stopped,
        "the run ended by cancellation, not by a contention-derived drain"
    );
    assert_eq!(
        terminal.all_completed.load(Ordering::SeqCst),
        0,
        "queued work was never drained, so completion must never be announced"
    );
}

/// The complementary case: once the same contention clears, a genuinely held
/// candidate stays held.
///
/// Resuming must not overcorrect into dispatching everything it was withholding.
/// A real Acceptance hold is stable blocked-only work, so the finite run reports
/// `BlockedOrStalled` without ever starting analysis, dispatch, or a repeat
/// acceptance.
#[tokio::test]
async fn reducer_snapshot_contention_release_preserves_a_real_acceptance_hold() {
    let repo = init_repo(&["held-a"]);
    let shared = reducer_state(&["held-a"], &["held-a"]).await;
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&acceptance_hold("held-a"));
        assert!(
            guard.acceptance_stalled_change_ids().contains("held-a"),
            "fixture must install a reducer-owned Acceptance hold"
        );
        assert!(
            guard.queued_change_ids().contains(&"held-a".to_string()),
            "the held change must still carry queue intent, or the test proves nothing"
        );
    }

    let run = wire_run(repo.path(), shared.clone(), SchedulerLifetime::Finite);
    let ContendedRun {
        mut executor,
        queue,
        probe,
        terminal,
        ..
    } = run;
    assert!(queue.push("held-a".to_string()).await);

    let contended = shared.write().await;
    let analyzer = probing_analyzer(probe.clone(), shared.clone());
    let mut scheduler = tokio::spawn(async move {
        executor
            .execute_with_order_based_reanalysis(Vec::new(), analyzer)
            .await
    });

    assert!(
        tokio::time::timeout(NO_PROGRESS_WINDOW, &mut scheduler)
            .await
            .is_err(),
        "even a run that will end blocked must not report that while evidence is unavailable"
    );

    drop(contended);

    let report = tokio::time::timeout(MUST_HAPPEN, scheduler)
        .await
        .expect("the released writer must let the blocked-only decision complete")
        .expect("scheduler task must not panic")
        .expect("scheduler loop must not fail");

    assert_eq!(
        report,
        SchedulerRunReport::BlockedOrStalled,
        "a real hold is stable blocked-only work once evidence is coherent"
    );
    assert_eq!(
        probe.invocations(),
        0,
        "a held candidate must not reach dependency analysis or ordinary dispatch"
    );
    assert_eq!(
        terminal.all_completed.load(Ordering::SeqCst),
        0,
        "blocked-only work is not completion"
    );
    assert!(
        shared
            .read()
            .await
            .acceptance_stalled_change_ids()
            .contains("held-a"),
        "the hold is still reducer-owned; the resumed pass did not repeat acceptance"
    );
}

/// The resumed evaluation admits an ordinary candidate all the way to the
/// reducer-governed dispatch gate.
///
/// `select_changes_for_dispatch` is where reducer intent decides dispatch, so
/// this asserts the post-release snapshot authorizes real work rather than
/// merely failing to terminate.
#[tokio::test]
async fn reducer_snapshot_contention_release_admits_the_candidate_to_dispatch() {
    let repo = init_repo(&["queued-a"]);
    let shared = reducer_state(&["queued-a"], &["queued-a"]).await;
    let run = wire_run(repo.path(), shared.clone(), SchedulerLifetime::Finite);
    let ContendedRun { mut executor, .. } = run;

    let contended = shared.write().await;
    let mut selection = tokio::spawn(async move {
        let mut queued: Vec<Change> = Vec::new();
        let in_flight = HashSet::new();
        let reconciled = executor
            .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
            .await;
        let analysis = AnalysisResult {
            order: queued.iter().map(|change| change.id.clone()).collect(),
            dependencies: HashMap::new(),
            groups: None,
        };
        let selected = executor
            .select_changes_for_dispatch(&analysis, 1, &in_flight)
            .await;
        (reconciled.queued_added, selected)
    });

    assert!(
        tokio::time::timeout(NO_PROGRESS_WINDOW, &mut selection)
            .await
            .is_err(),
        "reconciliation and dispatch selection must both wait for coherent evidence"
    );

    drop(contended);

    let (queued_added, selected) = tokio::time::timeout(MUST_HAPPEN, selection)
        .await
        .expect("the released writer must let reconciliation and selection finish")
        .expect("selection task must not panic");

    assert_eq!(
        queued_added, 1,
        "reducer-visible queue intent is reconciled into the scheduler-local queue"
    );
    assert_eq!(
        selected,
        vec!["queued-a".to_string()],
        "the resumed snapshot admits the ordinary candidate to dispatch"
    );
}

/// Incomplete reducer evidence may never authorize termination or idle.
///
/// This pins the gate directly, independently of how the scheduler happened to
/// arrive at it: the same empty scheduler state that legitimately drains and
/// idles on complete evidence must do neither on an incomplete view.
#[tokio::test]
async fn reducer_snapshot_contention_never_authorizes_termination_or_idle() {
    let repo = init_repo(&[]);
    let shared = reducer_state(&[], &[]).await;
    let run = wire_run(repo.path(), shared.clone(), SchedulerLifetime::Finite);
    let ContendedRun { mut executor, .. } = run;

    let incomplete = ReducerWorkSnapshot::incomplete();
    let complete = executor.capture_reducer_work_snapshot().await;
    let empty_queue: Vec<Change> = Vec::new();
    let in_flight = HashSet::new();

    assert!(
        !executor
            .should_exit_when_idle(true, &empty_queue, &in_flight, Some(&incomplete))
            .await,
        "a finite run must not report DrainedSuccessfully or BlockedOrStalled on incomplete evidence"
    );
    assert!(
        executor
            .should_exit_when_idle(true, &empty_queue, &in_flight, Some(&complete))
            .await,
        "the same state does drain once the reducer view is coherent"
    );

    executor.set_persistent_lifetime();
    assert!(
        !executor
            .should_enter_persistent_idle_wait(true, &empty_queue, &in_flight, Some(&incomplete))
            .await,
        "a persistent run must not park in the timer-free idle wait on incomplete evidence"
    );
    assert!(
        executor
            .should_enter_persistent_idle_wait(true, &empty_queue, &in_flight, Some(&complete))
            .await,
        "stable, genuinely drained state still enters event-driven persistent idle"
    );
}

/// An unjudgeable dynamic hint keeps its wake edge.
///
/// A popped hint may be the only thing that will ever wake the scheduler for
/// that change, so a pass that cannot judge it must put it back rather than
/// consume it. Nothing scheduler-visible is derived from the gap either.
#[tokio::test]
async fn reducer_snapshot_contention_retains_an_unjudged_dynamic_hint() {
    let repo = init_repo(&["queued-a"]);
    let shared = reducer_state(&["queued-a"], &["queued-a"]).await;
    let run = wire_run(repo.path(), shared.clone(), SchedulerLifetime::Finite);
    let ContendedRun {
        mut executor,
        queue,
        ..
    } = run;

    assert!(queue.push("queued-a".to_string()).await);

    let mut queued: Vec<Change> = Vec::new();
    let in_flight = HashSet::new();
    let mut reason = crate::parallel::dynamic_queue::ReanalysisReason::Initial;
    let ingested = executor
        .check_dynamic_queue_and_add_changes_with_snapshot(
            &mut queued,
            &in_flight,
            &mut reason,
            &ReducerWorkSnapshot::incomplete(),
        )
        .await;

    assert!(!ingested, "incomplete evidence admits nothing");
    assert!(queued.is_empty(), "and adds no scheduler-local candidate");
    assert!(
        queue.contains("queued-a").await,
        "the hint must survive so its wake edge is not lost"
    );

    // With coherent evidence the very same hint is admitted, proving retention
    // deferred the decision rather than refusing it.
    let snapshot = executor.capture_reducer_work_snapshot().await;
    assert!(
        executor
            .check_dynamic_queue_and_add_changes_with_snapshot(
                &mut queued,
                &in_flight,
                &mut reason,
                &snapshot,
            )
            .await,
        "the retained hint is ingested once the reducer view is coherent"
    );
    assert_eq!(
        queued
            .iter()
            .map(|change| change.id.as_str())
            .collect::<Vec<_>>(),
        vec!["queued-a"]
    );
}

/// Cancellation terminates scheduler work even while snapshot acquisition is
/// still pending.
///
/// The writer is never released here: if the awaited read were not raced against
/// the cancellation token, the run could only end when the reducer write did.
#[tokio::test]
async fn reducer_snapshot_contention_stays_cancellable_while_acquisition_is_pending() {
    let repo = init_repo(&["queued-a"]);
    let shared = reducer_state(&["queued-a"], &["queued-a"]).await;
    let run = wire_run(repo.path(), shared.clone(), SchedulerLifetime::Persistent);
    let ContendedRun {
        mut executor,
        queue,
        cancel,
        probe,
        ..
    } = run;
    assert!(queue.push("queued-a".to_string()).await);

    let contended = shared.write().await;
    let analyzer = probing_analyzer(probe.clone(), shared.clone());
    let mut scheduler = tokio::spawn(async move {
        executor
            .execute_with_order_based_reanalysis(Vec::new(), analyzer)
            .await
    });

    assert!(
        tokio::time::timeout(NO_PROGRESS_WINDOW, &mut scheduler)
            .await
            .is_err(),
        "the scheduler must still be suspended on snapshot acquisition"
    );

    cancel.cancel();

    let report = tokio::time::timeout(MUST_HAPPEN, scheduler)
        .await
        .expect("cancellation must terminate a scheduler that is waiting for a reducer snapshot")
        .expect("scheduler task must not panic")
        .expect("scheduler loop must not fail");

    // The writer is still held at this point on purpose.
    assert_eq!(report, SchedulerRunReport::Stopped);
    assert_eq!(
        probe.invocations(),
        0,
        "a cancelled acquisition must not fall through into analysis"
    );
    drop(contended);
}
