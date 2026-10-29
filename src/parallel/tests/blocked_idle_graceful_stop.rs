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
//! The classifier-barrier cases suspend the real blocked-only classification
//! inside an archived dependency's base lookup and accept cancel-stop through
//! shared run control while it is pending, so the settlement's post-
//! classification flag read is what decides — not the stale entry read.
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

// ============================================================================
// Cancel-stop across the awaited blocked-only classification
// ============================================================================

const ARCHIVED_UPSTREAM: &str = "archived-upstream";

/// Workspace-manager double that suspends the first dependency-base lookup.
///
/// Blocked-only classification awaits `ensure_original_branch_initialized`
/// only for an *archived* dependency, before the real Git merge-evidence probe.
/// The first call acknowledges entry and then waits for the test to release
/// it; every later call passes straight through. Base evidence itself stays
/// real (`is_merged_to_base` is not routed through the manager), and nothing
/// this fixture never expects the scheduler to do is answered quietly: every
/// workspace-mutating method panics.
struct BlockingDependencyWorkspaceManager {
    repo_root: std::path::PathBuf,
    entered: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

#[async_trait::async_trait]
impl crate::vcs::WorkspaceManager for BlockingDependencyWorkspaceManager {
    fn backend_type(&self) -> crate::vcs::VcsBackend {
        crate::vcs::VcsBackend::Git
    }

    async fn check_available(&self) -> crate::vcs::VcsResult<bool> {
        Ok(true)
    }

    async fn prepare_for_parallel(&self) -> crate::vcs::VcsResult<Option<crate::vcs::VcsWarning>> {
        panic!("blocked-idle settlement must not prepare parallel workspaces")
    }

    async fn get_current_revision(&self) -> crate::vcs::VcsResult<String> {
        panic!("blocked-idle settlement must not read the checkout revision")
    }

    async fn current_branch(&self) -> crate::vcs::VcsResult<Option<String>> {
        Ok(Some("main".to_string()))
    }

    async fn create_workspace(
        &mut self,
        _change_id: &str,
        _base_revision: Option<&str>,
    ) -> crate::vcs::VcsResult<crate::vcs::Workspace> {
        panic!("blocked-idle settlement must not create a workspace")
    }

    fn update_workspace_status(
        &mut self,
        _workspace_name: &str,
        _status: crate::vcs::WorkspaceStatus,
    ) {
        panic!("blocked-idle settlement must not update a workspace")
    }

    async fn merge_workspaces(&self, _revisions: &[String]) -> crate::vcs::VcsResult<String> {
        panic!("blocked-idle settlement must not merge")
    }

    async fn cleanup_workspace(&mut self, _workspace_name: &str) -> crate::vcs::VcsResult<()> {
        panic!("blocked-idle settlement must not clean up a workspace")
    }

    async fn cleanup_all(&mut self) -> crate::vcs::VcsResult<()> {
        panic!("blocked-idle settlement must not clean up workspaces")
    }

    fn max_concurrent(&self) -> usize {
        1
    }

    fn workspaces(&self) -> Vec<crate::vcs::Workspace> {
        Vec::new()
    }

    async fn list_worktree_change_ids(&self) -> crate::vcs::VcsResult<HashSet<String>> {
        Ok(HashSet::new())
    }

    fn conflict_resolution_prompt(&self) -> &'static str {
        "unused"
    }

    async fn snapshot_working_copy(
        &self,
        _workspace_path: &std::path::Path,
    ) -> crate::vcs::VcsResult<()> {
        panic!("blocked-idle settlement must not snapshot a workspace")
    }

    async fn create_verified_commit(
        &self,
        _workspace_path: &std::path::Path,
        _message: &str,
    ) -> crate::vcs::VcsResult<crate::vcs::VerifiedCommitOutcome> {
        panic!("blocked-idle settlement must not commit")
    }

    async fn create_iteration_snapshot(
        &self,
        _workspace_path: &std::path::Path,
        _change_id: &str,
        _iteration: u32,
        _completed: u32,
        _total: u32,
    ) -> crate::vcs::VcsResult<()> {
        panic!("blocked-idle settlement must not snapshot an iteration")
    }

    async fn squash_wip_commits(
        &self,
        _workspace_path: &std::path::Path,
        _change_id: &str,
        _final_iteration: u32,
    ) -> crate::vcs::VcsResult<()> {
        panic!("blocked-idle settlement must not squash commits")
    }

    async fn get_revision_in_workspace(
        &self,
        _workspace_path: &std::path::Path,
    ) -> crate::vcs::VcsResult<String> {
        panic!("blocked-idle settlement must not read a workspace revision")
    }

    async fn get_status(&self) -> crate::vcs::VcsResult<String> {
        Ok(String::new())
    }

    async fn get_log_for_revisions(&self, _revisions: &[String]) -> crate::vcs::VcsResult<String> {
        Ok(String::new())
    }

    async fn detect_conflicts(&self) -> crate::vcs::VcsResult<Vec<String>> {
        Ok(Vec::new())
    }

    fn forget_workspace_sync(&self, _workspace_name: &str) {}

    fn repo_root(&self) -> &std::path::Path {
        &self.repo_root
    }

    async fn ensure_original_branch_initialized(&self) -> crate::vcs::VcsResult<String> {
        let entered = self
            .entered
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(entered) = entered {
            entered
                .send(())
                .expect("the test awaits the classifier entry acknowledgement");
            let release = self.release.lock().await.take();
            if let Some(release) = release {
                release
                    .await
                    .expect("the test releases the suspended classifier");
            }
        }
        Ok("main".to_string())
    }

    fn original_branch(&self) -> Option<String> {
        Some("main".to_string())
    }

    async fn find_existing_workspace(
        &mut self,
        _change_id: &str,
    ) -> crate::vcs::VcsResult<Option<crate::vcs::WorkspaceInfo>> {
        panic!("blocked-idle settlement must not look up a workspace")
    }

    async fn reuse_workspace(
        &mut self,
        _workspace_info: &crate::vcs::WorkspaceInfo,
    ) -> crate::vcs::VcsResult<crate::vcs::Workspace> {
        panic!("blocked-idle settlement must not reuse a workspace")
    }
}

/// One evaluation boundary over genuinely blocked archived-dependency
/// evidence, a suspended classifier, and the shared run-control service.
struct ClassifierBarrierFixture {
    executor: ParallelExecutor,
    events: mpsc::Receiver<ParallelEvent>,
    service: crate::orchestration::run_control::RunControlService,
    scheduler: Arc<crate::orchestration::run_control::testing::RecordingScheduler>,
    graceful_stop: Arc<AtomicBool>,
    queued: Vec<Change>,
    snapshot: ReducerWorkSnapshot,
    entered: Option<tokio::sync::oneshot::Receiver<()>>,
    release: Option<tokio::sync::oneshot::Sender<()>>,
    _repo: TempDir,
}

impl ClassifierBarrierFixture {
    async fn new() -> Self {
        use crate::orchestration::operator_command::{
            ExecutionMarkStore, NoopQueueHooks, OperatorCommandService,
        };
        use crate::orchestration::run_control::testing::RecordingScheduler;
        use crate::orchestration::run_control::{
            ResolveReservations, RunControlService, StartEligibility,
        };

        // A committed, valid queued proposal whose only dependency is archived
        // on disk but not integrated into `main`.
        let repo = TempDir::new().expect("create temp repo");
        let root = repo.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.email", "test@example.com"]);
        git(root, &["config", "user.name", "Test User"]);
        std::fs::write(root.join("README.md"), "base\n").expect("write base file");
        let change_dir = root.join("openspec/changes").join(BLOCKED);
        std::fs::create_dir_all(&change_dir).expect("create change directory");
        let proposal = change_dir.join("proposal.md");
        std::fs::write(
            &proposal,
            format!("---\ndependencies:\n  - {ARCHIVED_UPSTREAM}\n---\n# {BLOCKED}\n"),
        )
        .expect("write proposal");
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [ ] apply\n",
        )
        .expect("write tasks");
        git(root, &["add", "-A"]);
        git(root, &["commit", "-m", "Base"]);
        let archive_entry = root
            .join("openspec/changes/archive")
            .join(format!("2026-01-01-{ARCHIVED_UPSTREAM}"));
        std::fs::create_dir_all(&archive_entry).expect("create archive entry");
        std::fs::write(
            archive_entry.join("proposal.md"),
            format!("# {ARCHIVED_UPSTREAM}\n"),
        )
        .expect("write archived proposal");

        assert_eq!(
            crate::openspec::parse_proposal_dependencies_strict_from_file(&proposal)
                .expect("strict metadata parses"),
            vec![ARCHIVED_UPSTREAM.to_string()]
        );
        assert!(
            crate::dependency_targets::collect_archived_change_ids(root)
                .contains(ARCHIVED_UPSTREAM),
            "archive discovery sees the dependency"
        );
        assert!(
            !root
                .join("openspec/changes")
                .join(ARCHIVED_UPSTREAM)
                .exists(),
            "there is no active proposal for the dependency"
        );
        assert!(
            !crate::execution::state::is_merged_to_base(ARCHIVED_UPSTREAM, root, "main")
                .await
                .expect("base evidence is readable"),
            "the archived dependency is not integrated into main"
        );

        let shared = Arc::new(RwLock::new(OrchestratorState::new(
            vec![BLOCKED.to_string()],
            10,
        )));
        shared
            .write()
            .await
            .apply_command(ReducerCommand::AddToQueue(BLOCKED.to_string()));

        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        // Known spare capacity: this isolates classifier suspension from send
        // backpressure.
        let (event_tx, events) = mpsc::channel(16);
        let mut executor = ParallelExecutor::new(root.to_path_buf(), config(root), Some(event_tx));
        executor.set_persistent_lifetime();
        executor.set_shared_orchestrator_state(shared.clone());
        executor.set_workspace_manager(Box::new(BlockingDependencyWorkspaceManager {
            repo_root: root.to_path_buf(),
            entered: std::sync::Mutex::new(Some(entered_tx)),
            release: tokio::sync::Mutex::new(Some(release_rx)),
        }));

        let queue = Arc::new(DynamicQueue::new());
        let scheduler = Arc::new(RecordingScheduler::new());
        let graceful_stop = scheduler.graceful_stop_flag();
        executor.set_graceful_stop_flag(graceful_stop.clone());
        let operator = Arc::new(OperatorCommandService::new(
            shared.clone(),
            queue,
            Arc::new(NoopQueueHooks),
            Arc::new(ExecutionMarkStore::new()),
        ));
        let service = RunControlService::new(
            shared,
            operator,
            scheduler.clone(),
            Arc::new(ResolveReservations::new()),
            Arc::new(StartEligibility::new()),
        );

        let snapshot = executor.capture_reducer_work_snapshot().await;
        assert!(snapshot.is_complete());
        let queued = vec![Change {
            id: BLOCKED.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: String::new(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }];

        Self {
            executor,
            events,
            service,
            scheduler,
            graceful_stop,
            queued,
            snapshot,
            entered: Some(entered_rx),
            release: Some(release_tx),
            _repo: repo,
        }
    }

    async fn request_stop(&self) {
        use crate::orchestration::operator_command::OperatorMode;
        use crate::orchestration::run_control::RunControlOutcome;
        assert_eq!(
            self.service
                .stop(OperatorMode::Running)
                .await
                .expect("stop is accepted while running"),
            RunControlOutcome::StopRequested
        );
        assert!(self.graceful_stop.load(Ordering::SeqCst));
    }

    /// One settlement evaluation from the actual production helper.
    async fn settle(&self) -> bool {
        self.executor
            .settle_graceful_stop_when_no_work(true, &self.queued, &HashSet::new(), &self.snapshot)
            .await
    }

    fn drain(&mut self) -> Counts {
        let mut counts = Counts::default();
        while let Ok(event) = self.events.try_recv() {
            match event {
                ParallelEvent::PersistentSchedulerIdle => counts.idle_edges += 1,
                ParallelEvent::Stopped => counts.stopped += 1,
                ParallelEvent::AllCompleted => counts.all_completed += 1,
                ParallelEvent::Error { .. } => counts.errors += 1,
                ParallelEvent::AnalysisStarted { .. }
                | ParallelEvent::WorkspacePreparationStarted { .. }
                | ParallelEvent::ApplyStarted { .. }
                | ParallelEvent::AcceptanceStarted { .. }
                | ParallelEvent::ArchiveStarted { .. }
                | ParallelEvent::MergeStarted { .. }
                | ParallelEvent::ResolveStarted { .. } => counts.work_starts += 1,
                _ => {}
            }
        }
        counts
    }
}

/// Verification `idle-stop-cancel-race`: cancel-stop accepted while the real
/// blocked-only classifier is suspended in its archived-dependency base lookup
/// wins that evaluation — no `Stopped` — and a later accepted Stop on the same
/// executor still settles exactly once.
#[tokio::test]
async fn blocked_idle_graceful_stop_cancel_during_classifier_never_stops() {
    use crate::orchestration::operator_command::OperatorMode;
    use crate::orchestration::run_control::RunControlOutcome;

    let mut fixture = ClassifierBarrierFixture::new().await;
    fixture.request_stop().await;

    let entered = fixture.entered.take().expect("entry barrier");
    let release = fixture.release.take().expect("release barrier");
    let control = async {
        entered
            .await
            .expect("the classifier reaches its dependency-base lookup");
        assert!(
            fixture.graceful_stop.load(Ordering::SeqCst),
            "the request was pending when classification suspended"
        );
        assert_eq!(
            fixture
                .service
                .cancel_stop(OperatorMode::Stopping)
                .await
                .expect("cancel-stop is accepted while stopping"),
            RunControlOutcome::StopCancelled
        );
        assert!(
            !fixture.graceful_stop.load(Ordering::SeqCst),
            "cancel-stop withdrew the shared request"
        );
        release
            .send(())
            .expect("the suspended classifier is still waiting");
    };
    let (settled, ()) = tokio::time::timeout(MUST_HAPPEN, async {
        tokio::join!(fixture.settle(), control)
    })
    .await
    .expect("the boundary evaluation must finish once released");

    assert!(
        !settled,
        "a request withdrawn during classification must not settle"
    );
    assert_eq!(
        fixture.drain(),
        Counts::default(),
        "the withdrawn evaluation publishes nothing"
    );

    // A fresh Stop is a new pending request, not one suppressed forever. The
    // barrier is spent, so this evaluation classifies without suspending.
    fixture.request_stop().await;
    assert!(
        tokio::time::timeout(MUST_HAPPEN, fixture.settle())
            .await
            .expect("the later evaluation must finish"),
        "a later accepted Stop settles at its own boundary"
    );
    assert_eq!(
        fixture.drain(),
        Counts {
            stopped: 1,
            ..Counts::default()
        },
        "exactly one Stopped in total, and nothing executed or completed"
    );
    assert!(
        fixture.scheduler.started_targets().is_empty(),
        "no run was started"
    );
}

/// Verification `idle-stop-cancel-race`: with the same suspended classifier and
/// no cancellation, the pending request settles exactly once after release —
/// the fence does not disable Stop.
#[tokio::test]
async fn blocked_idle_graceful_stop_pending_after_classifier_stops_once() {
    let mut fixture = ClassifierBarrierFixture::new().await;
    fixture.request_stop().await;

    let entered = fixture.entered.take().expect("entry barrier");
    let release = fixture.release.take().expect("release barrier");
    let mut events = std::mem::replace(&mut fixture.events, mpsc::channel(1).1);
    let control = async {
        entered
            .await
            .expect("the classifier reaches its dependency-base lookup");
        assert!(
            matches!(events.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
            "nothing is published while classification is suspended"
        );
        release
            .send(())
            .expect("the suspended classifier is still waiting");
    };
    let (settled, ()) = tokio::time::timeout(MUST_HAPPEN, async {
        tokio::join!(fixture.settle(), control)
    })
    .await
    .expect("the boundary evaluation must finish once released");
    fixture.events = events;

    assert!(settled, "a still-pending request settles at this boundary");
    assert!(fixture.graceful_stop.load(Ordering::SeqCst));
    assert_eq!(
        fixture.drain(),
        Counts {
            stopped: 1,
            ..Counts::default()
        },
        "exactly one Stopped, and nothing executed or completed"
    );
    assert!(fixture.scheduler.started_targets().is_empty());
}
