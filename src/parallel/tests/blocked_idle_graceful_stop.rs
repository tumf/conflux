//! Graceful stop at an inactive blocked-only scheduler boundary.
//!
//! A persistent scheduler whose only queued work is held — a valid proposal
//! queued behind a dependency that does not exist — parks in its timer-free
//! event-driven wait. An accepted graceful stop wakes that park, but the
//! no-work settlement used to require an empty scheduler-local queue, so the
//! scheduler re-parked with the request still pending and every frontend sat in
//! `Stopping` waiting for a phase no process was executing.
//!
//! The positive and cancellation cases drive the *real* loop
//! (`execute_with_order_based_reanalysis`) over a committed repository, a real
//! reducer, and the same graceful-stop flag plus scheduler notification that
//! shared run control uses. Every wait is an event barrier: the typed
//! `PersistentSchedulerIdle` edge proves a pass parked, and the scheduler
//! task's own completion proves it settled. Generous timeouts only guard
//! against hangs; nothing passes because a clock ran out.
//!
//! The negative controls pin the settlement decision itself over a real
//! executor and coherent reducer views, because runtime ownership that would
//! keep a stop owed (an in-flight task, a lane waiter, a manual resolver, a
//! pending merge) cannot be produced without starting real agent work.
//!
//! Integration-scoped: git, the filesystem, and the reducer lock are real.

use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::state::{OrchestratorState, ReducerCommand};
use crate::parallel::work_snapshot::ReducerWorkSnapshot;
use crate::parallel::{ParallelEvent, ParallelExecutor, SchedulerLifetime, SchedulerRunReport};
use crate::tui::queue::DynamicQueue;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{mpsc, RwLock};

/// Hang guard for something that must happen. Only reached once the
/// assertion has already failed.
const MUST_HAPPEN: Duration = Duration::from_secs(10);

const BLOCKED: &str = "blocked-alpha";
const BLOCKED_TOO: &str = "blocked-beta";
const MISSING_DEPENDENCY: &str = "never-proposed";

type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

fn git(repo_root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .expect("run git command");
    assert!(output.status.success(), "git {:?} failed", args);
}

/// A valid proposal whose only dependency was never proposed. CRLF line
/// endings match the original real-owner evidence.
fn write_blocked_change(repo_root: &std::path::Path, change_id: &str) {
    let change_dir = repo_root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).expect("create change directory");
    std::fs::write(
        change_dir.join("proposal.md"),
        format!("---\r\ndependencies:\r\n  - {MISSING_DEPENDENCY}\r\n---\r\n# {change_id}\r\n"),
    )
    .expect("write proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [ ] apply\n",
    )
    .expect("write tasks");
}

fn init_repo(change_ids: &[&str]) -> TempDir {
    let repo = TempDir::new().expect("create temp repo");
    let root = repo.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.email", "test@example.com"]);
    git(root, &["config", "user.name", "Test User"]);
    std::fs::write(root.join("README.md"), "base\n").expect("write base file");
    for change_id in change_ids {
        write_blocked_change(root, change_id);
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "Base"]);
    repo
}

fn config(workspace_base: &std::path::Path) -> OrchestratorConfig {
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

/// Every event the scheduler published that this contract cares about.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    idle_edges: usize,
    stopped: usize,
    all_completed: usize,
    errors: usize,
    /// Analysis, workspace, Apply, Acceptance, archive, merge, or resolve
    /// starts: any of them means blocked work was executed.
    work_starts: usize,
}

/// Scheduler-side analyzer double. Its invocation count is the proof that no
/// pass treated blocked work as analyzable.
fn counting_analyzer(
    invocations: Arc<AtomicUsize>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |_changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        invocations.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
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

/// A real persistent scheduler loop, running.
struct RunningScheduler {
    shared: Arc<RwLock<OrchestratorState>>,
    queue: Arc<DynamicQueue>,
    graceful_stop: Arc<AtomicBool>,
    analyzer_invocations: Arc<AtomicUsize>,
    events: mpsc::Receiver<ParallelEvent>,
    counts: Counts,
    task: tokio::task::JoinHandle<crate::error::Result<SchedulerRunReport>>,
    _repo: TempDir,
    _workspace_base: TempDir,
}

impl RunningScheduler {
    async fn start(change_ids: &[&str], queued: &[&str]) -> Self {
        let repo = init_repo(change_ids);
        let workspace_base = TempDir::new().expect("create workspace base");
        let shared = Arc::new(RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )));
        {
            let mut guard = shared.write().await;
            for id in queued {
                guard.apply_command(ReducerCommand::AddToQueue(id.to_string()));
            }
        }
        let (event_tx, events) = mpsc::channel(512);
        let mut executor = ParallelExecutor::new(
            repo.path().to_path_buf(),
            config(workspace_base.path()),
            Some(event_tx),
        );
        let queue = Arc::new(DynamicQueue::new());
        let graceful_stop = Arc::new(AtomicBool::new(false));
        executor.set_shared_orchestrator_state(shared.clone());
        executor.set_dynamic_queue(queue.clone());
        executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);
        executor.set_graceful_stop_flag(graceful_stop.clone());
        let analyzer_invocations = Arc::new(AtomicUsize::new(0));
        let analyzer = counting_analyzer(analyzer_invocations.clone());
        let task = tokio::spawn(async move {
            executor
                .execute_with_order_based_reanalysis(Vec::new(), analyzer)
                .await
        });
        Self {
            shared,
            queue,
            graceful_stop,
            analyzer_invocations,
            events,
            counts: Counts::default(),
            task,
            _repo: repo,
            _workspace_base: workspace_base,
        }
    }

    fn record(&mut self, event: &ParallelEvent) {
        match event {
            ParallelEvent::PersistentSchedulerIdle => self.counts.idle_edges += 1,
            ParallelEvent::Stopped => self.counts.stopped += 1,
            ParallelEvent::AllCompleted => self.counts.all_completed += 1,
            ParallelEvent::Error { .. } => self.counts.errors += 1,
            ParallelEvent::AnalysisStarted { .. }
            | ParallelEvent::WorkspacePreparationStarted { .. }
            | ParallelEvent::ApplyStarted { .. }
            | ParallelEvent::AcceptanceStarted { .. }
            | ParallelEvent::ArchiveStarted { .. }
            | ParallelEvent::MergeStarted { .. }
            | ParallelEvent::ResolveStarted { .. } => self.counts.work_starts += 1,
            _ => {}
        }
    }

    /// Event barrier: return once the scheduler has published `n` idle edges
    /// in total, i.e. a pass decided to park.
    async fn until_idle_edges(&mut self, n: usize) {
        tokio::time::timeout(MUST_HAPPEN, async {
            while self.counts.idle_edges < n {
                let event = self
                    .events
                    .recv()
                    .await
                    .expect("scheduler event stream closed before parking");
                self.record(&event);
            }
        })
        .await
        .expect("the scheduler must park on blocked-only work");
    }

    /// What shared run control does for an accepted graceful stop.
    fn request_graceful_stop(&self) {
        self.graceful_stop.store(true, Ordering::SeqCst);
        self.queue.notify_scheduler();
    }

    /// What shared run control does for an accepted cancel-stop.
    fn cancel_graceful_stop(&self) {
        self.graceful_stop.store(false, Ordering::SeqCst);
    }

    /// Await scheduler termination, then drain the events it published.
    async fn finish(
        mut self,
    ) -> (
        SchedulerRunReport,
        Counts,
        usize,
        Arc<RwLock<OrchestratorState>>,
    ) {
        let report = tokio::time::timeout(MUST_HAPPEN, &mut self.task)
            .await
            .expect("the woken scheduler must settle the stop without another wake")
            .expect("scheduler task must not panic")
            .expect("scheduler loop must not fail");
        while let Ok(event) = self.events.try_recv() {
            self.record(&event);
        }
        (
            report,
            self.counts,
            self.analyzer_invocations.load(Ordering::SeqCst),
            self.shared,
        )
    }
}

/// Verification `blocked-idle-graceful-stop`: an accepted graceful stop wakes a
/// blocked-only park and settles to exactly one `Stopped`.
#[tokio::test]
async fn blocked_idle_graceful_stop_settles_parked_dependency_blocked_scheduler() {
    let mut run = RunningScheduler::start(&[BLOCKED], &[BLOCKED]).await;

    run.until_idle_edges(1).await;
    assert!(
        !run.task.is_finished(),
        "with no stop requested the persistent scheduler stays parked"
    );

    run.request_graceful_stop();
    let (report, counts, analyzer_invocations, shared) = run.finish().await;

    assert_eq!(report, SchedulerRunReport::Stopped);
    assert_eq!(
        counts,
        Counts {
            idle_edges: 1,
            stopped: 1,
            all_completed: 0,
            errors: 0,
            work_starts: 0,
        },
        "one park, one terminal stop, and nothing executed or completed"
    );
    assert_eq!(analyzer_invocations, 0, "blocked work is never analyzed");

    // Process-wide `Stopped` reconciliation, as the frontends reduce it.
    let mut state = shared.write().await;
    state.apply_execution_event(&ParallelEvent::Stopped);
    assert_eq!(
        state.display_status(BLOCKED),
        "not queued",
        "ordinary queued intent returns to not queued, never completed or stopped"
    );
}

/// Verification `blocked-idle-graceful-stop`: cancel-stop withdraws the request
/// before the boundary, so the woken pass parks again instead of stopping, and
/// a later stop still settles exactly once.
#[tokio::test]
async fn blocked_idle_graceful_stop_cancel_before_boundary_never_stops() {
    let mut run = RunningScheduler::start(&[BLOCKED, BLOCKED_TOO], &[BLOCKED]).await;
    run.until_idle_edges(1).await;

    // Stop accepted, then withdrawn before any pass could observe it.
    run.graceful_stop.store(true, Ordering::SeqCst);
    run.cancel_graceful_stop();

    // New queued intent is the wake that also rearms the idle edge, so the
    // second edge is a deterministic barrier proving a pass evaluated the
    // withdrawn state and parked rather than stopping.
    run.shared
        .write()
        .await
        .apply_command(ReducerCommand::AddToQueue(BLOCKED_TOO.to_string()));
    assert!(run.queue.push(BLOCKED_TOO.to_string()).await);
    run.until_idle_edges(2).await;
    assert_eq!(
        run.counts.stopped, 0,
        "a withdrawn request produces no Stopped"
    );
    assert!(
        !run.task.is_finished(),
        "the scheduler is parked, not stopped"
    );

    run.request_graceful_stop();
    let (report, counts, analyzer_invocations, _) = run.finish().await;
    assert_eq!(report, SchedulerRunReport::Stopped);
    assert_eq!(
        (counts.stopped, counts.all_completed, counts.work_starts),
        (1, 0, 0)
    );
    assert_eq!(analyzer_invocations, 0);
}

// ============================================================================
// Settlement decision controls
// ============================================================================

/// A real executor over a reducer holding one queued, dependency-blocked row,
/// with a pending graceful stop.
async fn blocked_executor() -> (
    TempDir,
    ParallelExecutor,
    mpsc::Receiver<ParallelEvent>,
    Arc<AtomicBool>,
    Vec<Change>,
) {
    let repo = init_repo(&[BLOCKED]);
    let shared = Arc::new(RwLock::new(OrchestratorState::new(
        vec![BLOCKED.to_string()],
        10,
    )));
    shared
        .write()
        .await
        .apply_command(ReducerCommand::AddToQueue(BLOCKED.to_string()));
    let (tx, rx) = mpsc::channel(16);
    let mut executor =
        ParallelExecutor::new(repo.path().to_path_buf(), config(repo.path()), Some(tx));
    executor.set_persistent_lifetime();
    executor.set_shared_orchestrator_state(shared);
    let graceful_stop = Arc::new(AtomicBool::new(true));
    executor.set_graceful_stop_flag(graceful_stop.clone());
    let queued = vec![Change {
        id: BLOCKED.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: String::new(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }];
    (repo, executor, rx, graceful_stop, queued)
}

fn stopped_events(rx: &mut mpsc::Receiver<ParallelEvent>) -> usize {
    let mut count = 0;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, ParallelEvent::Stopped) {
            count += 1;
        }
    }
    count
}

/// Verification `blocked-idle-graceful-stop`: runtime work the stop is still
/// owed, or evidence that does not exist, never settles early — and once that
/// work reaches its boundary the stop settles exactly once.
#[tokio::test]
async fn blocked_idle_graceful_stop_owed_work_and_incomplete_evidence_do_not_settle() {
    let (_repo, mut executor, mut rx, graceful_stop, queued) = blocked_executor().await;
    let none = HashSet::new();
    let snapshot = executor.capture_reducer_work_snapshot().await;
    assert!(snapshot.is_complete());

    // Incomplete reducer evidence has observed nothing.
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(
                true,
                &queued,
                &none,
                &ReducerWorkSnapshot::incomplete()
            )
            .await,
        "an incomplete view must not settle"
    );
    // A live JoinSet task.
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(false, &queued, &none, &snapshot)
            .await,
        "an in-flight task owes its completion boundary"
    );
    // An admitted in-flight change.
    let in_flight = HashSet::from(["active-change".to_string()]);
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(true, &queued, &in_flight, &snapshot)
            .await,
        "admitted in-flight work owes its completion boundary"
    );
    // Resolve and reject lane waiters.
    executor
        .resolve_wait_changes
        .insert("needs-resolve".to_string());
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(true, &queued, &none, &snapshot)
            .await,
        "a resolve waiter is owed its boundary"
    );
    executor.resolve_wait_changes.clear();
    executor
        .reject_wait_changes
        .insert("needs-reject".to_string());
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(true, &queued, &none, &snapshot)
            .await,
        "a reject waiter is owed its boundary"
    );
    executor.reject_wait_changes.clear();
    // A running manual resolver.
    let manual = Arc::new(AtomicUsize::new(1));
    executor.set_manual_resolve_counter(manual.clone());
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(true, &queued, &none, &snapshot)
            .await,
        "a manual resolver is owed its boundary"
    );
    manual.store(0, Ordering::SeqCst);
    // A pending background merge.
    executor.pending_merge_count.store(1, Ordering::SeqCst);
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(true, &queued, &none, &snapshot)
            .await,
        "a pending merge is owed its boundary"
    );
    assert_eq!(stopped_events(&mut rx), 0, "no control published a stop");

    // The merge reaches its boundary; nothing but blocked work remains.
    executor.pending_merge_count.store(0, Ordering::SeqCst);

    // Withdrawn by cancel-stop: no settlement either.
    graceful_stop.store(false, Ordering::SeqCst);
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(true, &queued, &none, &snapshot)
            .await,
        "a withdrawn request settles nothing"
    );
    assert_eq!(stopped_events(&mut rx), 0);

    graceful_stop.store(true, Ordering::SeqCst);
    assert!(
        executor
            .settle_graceful_stop_when_no_work(true, &queued, &none, &snapshot)
            .await,
        "with only inactive blocked work left the stop settles"
    );
    assert_eq!(stopped_events(&mut rx), 1, "exactly one terminal stop");
}

/// Verification `blocked-idle-graceful-stop`: a dispatchable candidate is a
/// later boundary, so it never settles here; the settlement is bounded by the
/// existing blocked-only classification.
#[tokio::test]
async fn blocked_idle_graceful_stop_dispatchable_candidate_does_not_settle() {
    let repo = TempDir::new().expect("workspace root");
    let shared = Arc::new(RwLock::new(OrchestratorState::new(
        vec!["ready".to_string()],
        10,
    )));
    shared
        .write()
        .await
        .apply_command(ReducerCommand::AddToQueue("ready".to_string()));
    let (tx, mut rx) = mpsc::channel(16);
    let mut executor =
        ParallelExecutor::new(repo.path().to_path_buf(), config(repo.path()), Some(tx));
    executor.set_persistent_lifetime();
    executor.set_shared_orchestrator_state(shared);
    executor.set_graceful_stop_flag(Arc::new(AtomicBool::new(true)));
    let queued = vec![Change {
        id: "ready".to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: String::new(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }];
    let snapshot = executor.capture_reducer_work_snapshot().await;

    assert!(
        !executor
            .classify_queued_work_with_snapshot(&queued, &HashSet::new(), &snapshot)
            .await
            .is_blocked_only(),
        "an independent queued change is dispatchable"
    );
    assert!(
        !executor
            .settle_graceful_stop_when_no_work(true, &queued, &HashSet::new(), &snapshot)
            .await,
        "dispatchable work owes the stop a later boundary"
    );
    assert_eq!(stopped_events(&mut rx), 0);
}
