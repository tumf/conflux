//! The local run scheduler behind [`RunSchedulerPort`].
//!
//! The TUI used to own the orchestrator task handle in its event loop, which
//! meant only a keypress could start or cancel a run. The supervisor owns it
//! instead, so the TUI adapter and the `/api/v2` adapter drive one object: a
//! remote start really spawns the run, and a remote force stop really cancels
//! the task a keypress would have cancelled.
//!
//! Everything here is process-local and dropped on exit. The supervisor holds no
//! workflow decision state — it starts what it is told to start and reports what
//! actually happened.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::orchestration::run_control::RunSchedulerPort;
use crate::orchestration::state::OrchestratorState;
use crate::parallel::PostArchiveAction;
use crate::tui::events::OrchestratorEvent;
use crate::tui::orchestrator::{run_orchestrator, run_orchestrator_parallel};
use crate::tui::queue::DynamicQueue;
use crate::tui::stop_classification::{collect_stop_activity_snapshot, StopActivitySnapshot};

/// Immutable launch context for a local orchestrator run.
struct LaunchContext {
    repo_root: PathBuf,
    config: OrchestratorConfig,
    /// The frontend *delivery* channel a spawned run publishes to.
    ///
    /// A run owns its own dispatch: it applies each event to the reducer and
    /// projects it to `/api/v2` before handing it to this sink. Wiring the TUI's
    /// producer channel here instead would route every one of those events back
    /// through the TUI's producer boundary and apply it to the reducer a second
    /// time — a doubled apply count, a doubled event sequence.
    tx: mpsc::Sender<OrchestratorEvent>,
    dynamic_queue: DynamicQueue,
    shared_state: Arc<tokio::sync::RwLock<OrchestratorState>>,
    manual_resolve_counter: Arc<std::sync::atomic::AtomicUsize>,
    post_archive_action: PostArchiveAction,
    upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
    #[cfg(feature = "web-monitoring")]
    web_state: Option<Arc<crate::web::WebState>>,
}

/// Owns the local orchestrator task, its cancellation token, and the flags that
/// steer it.
pub struct TuiRunSupervisor {
    launch: LaunchContext,
    handle: Mutex<Option<tokio::task::JoinHandle<Result<()>>>>,
    cancel: Mutex<Option<CancellationToken>>,
    graceful_stop: Arc<AtomicBool>,
    parallel_mode: Arc<AtomicBool>,
}

impl TuiRunSupervisor {
    /// Build a supervisor for the current TUI invocation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        tx: mpsc::Sender<OrchestratorEvent>,
        dynamic_queue: DynamicQueue,
        shared_state: Arc<tokio::sync::RwLock<OrchestratorState>>,
        manual_resolve_counter: Arc<std::sync::atomic::AtomicUsize>,
        post_archive_action: PostArchiveAction,
        upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
        graceful_stop: Arc<AtomicBool>,
        parallel_mode: Arc<AtomicBool>,
        #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
    ) -> Self {
        Self {
            launch: LaunchContext {
                repo_root,
                config,
                tx,
                dynamic_queue,
                shared_state,
                manual_resolve_counter,
                post_archive_action,
                upstream_runtime,
                #[cfg(feature = "web-monitoring")]
                web_state,
            },
            handle: Mutex::new(None),
            cancel: Mutex::new(None),
            graceful_stop,
            parallel_mode,
        }
    }

    /// Take the current run task and cancellation token for shutdown.
    pub fn take_run(
        &self,
    ) -> (
        Option<tokio::task::JoinHandle<Result<()>>>,
        Option<CancellationToken>,
    ) {
        let handle = lock(&self.handle).take();
        let cancel = lock(&self.cancel).take();
        (handle, cancel)
    }

    /// The cancellation token of the live run, if any.
    pub fn cancel_token(&self) -> Option<CancellationToken> {
        lock(&self.cancel).clone()
    }

    /// Which orchestrator this launch must use.
    fn use_parallel_for(&self, targets: &[String]) -> bool {
        requires_parallel_dispatch(targets, self.parallel_mode.load(Ordering::SeqCst))
    }

    fn spawn(
        &self,
        targets: Vec<String>,
        explicit_retry: bool,
        use_parallel: bool,
    ) -> CancellationToken {
        let cancel = CancellationToken::new();

        let repo_root = self.launch.repo_root.clone();
        let config = self.launch.config.clone();
        let tx = self.launch.tx.clone();
        let dynamic_queue = self.launch.dynamic_queue.clone();
        let shared_state = self.launch.shared_state.clone();
        let manual_resolve_counter = self.launch.manual_resolve_counter.clone();
        let post_archive_action = self.launch.post_archive_action.clone();
        let upstream_runtime = self.launch.upstream_runtime.clone();
        let graceful_stop = self.graceful_stop.clone();
        let run_cancel = cancel.clone();
        #[cfg(feature = "web-monitoring")]
        let web_state = self.launch.web_state.clone();

        let handle = tokio::spawn(async move {
            if use_parallel {
                run_orchestrator_parallel(
                    targets,
                    explicit_retry,
                    repo_root,
                    config,
                    tx,
                    run_cancel,
                    dynamic_queue,
                    graceful_stop,
                    shared_state,
                    manual_resolve_counter,
                    post_archive_action,
                    upstream_runtime,
                    #[cfg(feature = "web-monitoring")]
                    web_state,
                )
                .await
            } else {
                run_orchestrator(
                    targets,
                    explicit_retry,
                    config,
                    tx,
                    run_cancel,
                    dynamic_queue,
                    graceful_stop,
                    shared_state,
                    #[cfg(feature = "web-monitoring")]
                    web_state,
                )
                .await
            }
        });

        *lock(&self.handle) = Some(handle);
        *lock(&self.cancel) = Some(cancel.clone());
        cancel
    }
}

/// Whether a launch must use the parallel orchestrator, whatever the toggle says.
///
/// A launch with no targets is scheduler-owned: the only caller that produces one
/// is the resolve dispatch, which exists solely to consume reducer-recorded
/// `ResolveWait`/`RejectWait` intent. Only the parallel machinery models that
/// intent — it syncs `resolve_wait_changes` from the shared reducer and preserves
/// it during empty-target startup. The serial orchestrator has no such notion: it
/// replaces the shared `OrchestratorState` over the (empty) target list, so it
/// would erase the very intent the launch was created to serve while still
/// reporting a started run. An empty-target launch is therefore always parallel.
fn requires_parallel_dispatch(targets: &[String], parallel_toggle: bool) -> bool {
    targets.is_empty() || parallel_toggle
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[async_trait]
impl RunSchedulerPort for TuiRunSupervisor {
    fn is_running(&self) -> bool {
        lock(&self.handle)
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    async fn start_run(
        &self,
        targets: Vec<String>,
        explicit_retry: bool,
    ) -> std::result::Result<(), String> {
        // An opted-in upstream session's terminal success is remote
        // confirmation, and the serial dispatch path carries no upstream
        // runtime: work dispatched there would finalize as terminal `merged` and
        // publish nothing. Startup already rejects `-u` with serial effective
        // mode, and this keeps the runtime toggle from walking around it. The
        // refusal follows the dispatch this launch really gets, not the toggle,
        // so a scheduler-owned resolve is not refused for a serial dispatch it
        // was never going to take.
        let use_parallel = self.use_parallel_for(&targets);
        if self.launch.upstream_runtime.is_some() && !use_parallel {
            return Err(
                "serial mode cannot run while -u/--integrate-upstream is active: upstream \
                 publication is defined on the cumulative parallel base"
                    .to_string(),
            );
        }

        self.graceful_stop.store(false, Ordering::SeqCst);
        self.spawn(targets, explicit_retry, use_parallel);
        Ok(())
    }

    async fn notify_scheduler(&self) {
        self.launch.dynamic_queue.notify_scheduler();
    }

    async fn cancel_run(&self) {
        if let Some(cancel) = self.cancel_token() {
            cancel.cancel();
        }
    }

    fn set_graceful_stop(&self, requested: bool) {
        self.graceful_stop.store(requested, Ordering::SeqCst);
    }

    async fn stop_activity(&self) -> StopActivitySnapshot {
        collect_stop_activity_snapshot(&self.launch.dynamic_queue, &self.launch.shared_state).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::ExecutionEvent;
    use crate::orchestration::state::{ExecutionMode, ReducerCommand};

    /// Marker the parallel orchestrator emits before it touches anything else,
    /// which is what makes the dispatched path observable from outside.
    const PARALLEL_START_MARKER: &str = "Starting parallel processing";

    fn shared_state(change_ids: &[&str]) -> Arc<tokio::sync::RwLock<OrchestratorState>> {
        Arc::new(tokio::sync::RwLock::new(OrchestratorState::with_mode(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
            ExecutionMode::Parallel,
        )))
    }

    fn supervisor(
        repo_root: PathBuf,
        tx: mpsc::Sender<OrchestratorEvent>,
        state: Arc<tokio::sync::RwLock<OrchestratorState>>,
        parallel_mode: Arc<AtomicBool>,
        upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
    ) -> TuiRunSupervisor {
        TuiRunSupervisor::new(
            repo_root,
            OrchestratorConfig::default(),
            tx,
            DynamicQueue::new(),
            state,
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            PostArchiveAction::MergeToBase,
            upstream_runtime,
            Arc::new(AtomicBool::new(false)),
            parallel_mode,
            #[cfg(feature = "web-monitoring")]
            None,
        )
    }

    /// Record reducer-owned `ResolveWait` for `change_id`, the way the shared
    /// run-control service does before it dispatches a manual resolve.
    async fn reserve_resolve_wait(
        state: &Arc<tokio::sync::RwLock<OrchestratorState>>,
        change_id: &str,
    ) {
        let mut guard = state.write().await;
        guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: change_id.to_string(),
            reason: "manual resolution required".to_string(),
            auto_resumable: false,
        });
        guard.apply_command(ReducerCommand::ResolveMerge(change_id.to_string()));
        assert_eq!(
            guard.resolve_wait_change_ids(),
            vec![change_id.to_string()],
            "arrangement must record the intent the dispatch exists to serve"
        );
    }

    #[test]
    fn only_an_empty_target_launch_overrides_the_serial_toggle() {
        let targets = vec!["alpha".to_string()];

        // A scheduler-owned launch carries no targets and is always parallel.
        assert!(requires_parallel_dispatch(&[], false));
        assert!(requires_parallel_dispatch(&[], true));
        // Operator-selected work still honours the toggle.
        assert!(!requires_parallel_dispatch(&targets, false));
        assert!(requires_parallel_dispatch(&targets, true));
    }

    /// `p` is accepted in Select/Stopped, so an operator can be in serial mode
    /// with a merge-wait row on screen. Consuming that row dispatches a run with
    /// no targets; serial startup would replace the shared reducer state over the
    /// empty target list, erasing the `ResolveWait` intent while `start_run`
    /// still reported a started run to the TUI and to `/api/v2`.
    #[tokio::test]
    async fn empty_target_launch_dispatches_parallel_despite_serial_toggle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (tx, mut rx) = mpsc::channel(64);
        let state = shared_state(&["alpha"]);
        reserve_resolve_wait(&state, "alpha").await;

        let supervisor = supervisor(
            dir.path().to_path_buf(),
            tx,
            state.clone(),
            Arc::new(AtomicBool::new(false)),
            None,
        );

        supervisor
            .start_run(Vec::new(), false)
            .await
            .expect("a scheduler-owned resolve must dispatch");

        let started = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("the dispatched run must announce itself")
            .expect("the dispatched run must emit its first event");
        match started {
            ExecutionEvent::Log(entry) => assert!(
                entry.message.contains(PARALLEL_START_MARKER),
                "an empty-target launch must reach the parallel orchestrator, got: {}",
                entry.message
            ),
            other => panic!("expected the parallel startup log, got: {other:?}"),
        }

        let (handle, cancel) = supervisor.take_run();
        if let Some(cancel) = cancel {
            cancel.cancel();
        }
        if let Some(handle) = handle {
            handle.abort();
            let _ = handle.await;
        }

        assert_eq!(
            state.read().await.resolve_wait_change_ids(),
            vec!["alpha".to_string()],
            "the dispatched run must not wipe the intent it was started to consume"
        );
    }

    /// The upstream refusal exists because the *serial* dispatch path carries no
    /// upstream runtime. A scheduler-owned resolve never takes that path, so it
    /// must not inherit the refusal from a toggle it does not follow.
    #[tokio::test]
    async fn empty_target_launch_is_not_refused_for_an_opted_in_serial_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (tx, _rx) = mpsc::channel(64);
        let state = shared_state(&["alpha"]);
        reserve_resolve_wait(&state, "alpha").await;

        let supervisor = supervisor(
            dir.path().to_path_buf(),
            tx,
            state,
            Arc::new(AtomicBool::new(false)),
            Some(crate::upstream::UpstreamRuntime {
                config: crate::upstream::UpstreamIntegrationConfig::new("origin", "exit 0"),
                branch: "main".to_string(),
            }),
        );

        supervisor
            .start_run(Vec::new(), false)
            .await
            .expect("a parallel-dispatched resolve keeps the publication contract");

        let (handle, cancel) = supervisor.take_run();
        if let Some(cancel) = cancel {
            cancel.cancel();
        }
        if let Some(handle) = handle {
            handle.abort();
            let _ = handle.await;
        }
    }
}
