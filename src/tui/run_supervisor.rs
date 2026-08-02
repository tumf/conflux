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

    /// Shared parallel-mode toggle the TUI keeps in sync with its own state.
    pub fn parallel_mode(&self) -> Arc<AtomicBool> {
        self.parallel_mode.clone()
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

    fn spawn(&self, targets: Vec<String>, explicit_retry: bool) -> CancellationToken {
        let cancel = CancellationToken::new();
        let use_parallel = self.parallel_mode.load(Ordering::SeqCst);

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
        // mode, and this keeps the runtime toggle from walking around it.
        if self.launch.upstream_runtime.is_some() && !self.parallel_mode.load(Ordering::SeqCst) {
            return Err(
                "serial mode cannot run while -u/--integrate-upstream is active: upstream \
                 publication is defined on the cumulative parallel base"
                    .to_string(),
            );
        }

        self.graceful_stop.store(false, Ordering::SeqCst);
        self.spawn(targets, explicit_retry);
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
