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
use tokio_util::sync::CancellationToken;

use crate::ai_command_runner::RunCommandScope;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::events::EventDispatcher;
use crate::orchestration::run_control::{RunPermit, RunSchedulerPort};
use crate::orchestration::state::OrchestratorState;
use crate::parallel::PostArchiveAction;
use crate::tui::orchestrator::run_orchestrator_parallel;
use crate::tui::queue::DynamicQueue;
use crate::tui::stop_classification::{collect_stop_activity_snapshot, StopActivitySnapshot};

/// Immutable launch context for a local orchestrator run.
#[derive(Clone)]
struct LaunchContext {
    repo_root: PathBuf,
    config: OrchestratorConfig,
    /// The process-lifetime authoritative dispatch owner every run publishes to.
    ///
    /// A spawned run does not build a dispatcher of its own: runner-local
    /// producers, accepted command outcomes, and orchestration runs share one
    /// owner, so one internal event is one reducer transition, one core mode
    /// transition, and one delivery per frontend regardless of who raised it.
    dispatcher: Arc<EventDispatcher>,
    dynamic_queue: DynamicQueue,
    shared_state: Arc<tokio::sync::RwLock<OrchestratorState>>,
    manual_resolve_counter: Arc<std::sync::atomic::AtomicUsize>,
    post_archive_action: PostArchiveAction,
    upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
}

/// The live run's task, cancellation token, and command scope.
///
/// Shared behind an `Arc` so a prepared launch can install them at *activation*
/// time without borrowing the supervisor: preparation must leave no observable
/// trace, which includes not claiming the run slots for a launch that may still
/// be rolled back.
#[derive(Default)]
struct RunSlots {
    handle: Mutex<Option<tokio::task::JoinHandle<Result<()>>>>,
    cancel: Mutex<Option<CancellationToken>>,
    /// The live run's command scope, retained *outside* the spawned
    /// orchestrator task.
    ///
    /// Aborting that task destroys everything it owns, so a scope reachable
    /// only from inside it would take the run's process identities with it.
    /// This clone is what still lets local shutdown force-clean them.
    scope: Mutex<Option<RunCommandScope>>,
}

/// Owns the local orchestrator task, its cancellation token, and the flags that
/// steer it.
pub struct TuiRunSupervisor {
    launch: LaunchContext,
    slots: Arc<RunSlots>,
    graceful_stop: Arc<AtomicBool>,
}

impl TuiRunSupervisor {
    /// Build a supervisor for the current TUI invocation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        dispatcher: Arc<EventDispatcher>,
        dynamic_queue: DynamicQueue,
        shared_state: Arc<tokio::sync::RwLock<OrchestratorState>>,
        manual_resolve_counter: Arc<std::sync::atomic::AtomicUsize>,
        post_archive_action: PostArchiveAction,
        upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
        graceful_stop: Arc<AtomicBool>,
    ) -> Self {
        Self {
            launch: LaunchContext {
                repo_root,
                config,
                dispatcher,
                dynamic_queue,
                shared_state,
                manual_resolve_counter,
                post_archive_action,
                upstream_runtime,
            },
            slots: Arc::new(RunSlots::default()),
            graceful_stop,
        }
    }

    /// Take the current run task, cancellation token, and command scope for shutdown.
    pub fn take_run(
        &self,
    ) -> (
        Option<tokio::task::JoinHandle<Result<()>>>,
        Option<CancellationToken>,
        Option<RunCommandScope>,
    ) {
        let handle = lock(&self.slots.handle).take();
        let cancel = lock(&self.slots.cancel).take();
        let scope = lock(&self.slots.scope).take();
        (handle, cancel, scope)
    }

    /// The command scope of the live run, if any.
    #[cfg(test)]
    pub fn run_command_scope(&self) -> Option<RunCommandScope> {
        lock(&self.slots.scope).clone()
    }

    /// The cancellation token of the live run, if any.
    pub fn cancel_token(&self) -> Option<CancellationToken> {
        lock(&self.slots.cancel).clone()
    }

    /// Build the closure that actually launches the run.
    ///
    /// Everything it needs is cloned here; running it is a pure `spawn` with no
    /// fallible step left, which is what makes activation infallible.
    fn launcher(&self, targets: Vec<String>, explicit_retry: bool) -> impl FnOnce() + Send {
        let launch = self.launch.clone();
        let slots = self.slots.clone();
        let graceful_stop = self.graceful_stop.clone();

        move || {
            let cancel = CancellationToken::new();
            // One fresh scope per run start. A closed scope is never reused, so
            // a restarted run always begins with open admission.
            let scope = RunCommandScope::new();
            scope.link_cancellation(cancel.clone());

            // Every launch — selected targets and scheduler-owned resolve alike
            // — dispatches the cumulative worktree scheduler, so an opted-in
            // upstream session always reaches the path its publication contract
            // is defined on.
            graceful_stop.store(false, Ordering::SeqCst);

            let run_cancel = cancel.clone();
            let run_scope = scope.clone();
            let handle = tokio::spawn(async move {
                run_orchestrator_parallel(
                    targets,
                    explicit_retry,
                    launch.repo_root,
                    launch.config,
                    launch.dispatcher,
                    run_cancel,
                    run_scope,
                    launch.dynamic_queue,
                    launch.shared_state,
                    launch.manual_resolve_counter,
                    launch.post_archive_action,
                    launch.upstream_runtime,
                )
                .await
            });

            *lock(&slots.handle) = Some(handle);
            *lock(&slots.cancel) = Some(cancel);
            *lock(&slots.scope) = Some(scope);
        }
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
        lock(&self.slots.handle)
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    /// Reserve a launch without spawning it.
    ///
    /// There is no fallible step: the orchestrator task is spawned by the
    /// returned permit, so nothing exists to emit an event, claim the run slots,
    /// or be observed as running until the accepted command outcome has already
    /// been published.
    async fn prepare_run(
        &self,
        targets: Vec<String>,
        explicit_retry: bool,
    ) -> std::result::Result<RunPermit, String> {
        Ok(RunPermit::new(self.launcher(targets, explicit_retry)))
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
    use crate::tui::events::OrchestratorEvent;
    use tokio::sync::mpsc;
    use crate::orchestration::state::ReducerCommand;

    /// Marker the worktree orchestrator emits before it touches anything else,
    /// which is what makes the dispatched path observable from outside.
    const PARALLEL_START_MARKER: &str = "Starting parallel processing";

    fn shared_state(change_ids: &[&str]) -> Arc<tokio::sync::RwLock<OrchestratorState>> {
        Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )))
    }

    /// The process-lifetime dispatch owner a spawned run publishes through.
    ///
    /// Built here rather than inside the supervisor because production builds
    /// exactly one for the whole process and hands it in; a supervisor that made
    /// its own would be the per-run dispatcher this change removed.
    fn dispatcher(
        tx: mpsc::Sender<OrchestratorEvent>,
        state: Arc<tokio::sync::RwLock<OrchestratorState>>,
    ) -> Arc<EventDispatcher> {
        Arc::new(EventDispatcher::new(
            state,
            vec![Arc::new(crate::tui::events::TuiEventSink::new(tx))],
        ))
    }

    fn supervisor(
        repo_root: PathBuf,
        tx: mpsc::Sender<OrchestratorEvent>,
        state: Arc<tokio::sync::RwLock<OrchestratorState>>,
        upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
    ) -> TuiRunSupervisor {
        TuiRunSupervisor::new(
            repo_root,
            OrchestratorConfig::default(),
            dispatcher(tx, state.clone()),
            DynamicQueue::new(),
            state,
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            PostArchiveAction::MergeToBase,
            upstream_runtime,
            Arc::new(AtomicBool::new(false)),
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

    /// Every launch reaches the cumulative worktree scheduler: a selected target
    /// and a scheduler-owned resolve alike, with no mode flag anywhere.
    #[tokio::test]
    async fn every_launch_dispatches_the_worktree_scheduler() {
        for targets in [Vec::new(), vec!["alpha".to_string()]] {
            let dir = tempfile::tempdir().expect("tempdir");
            let (tx, mut rx) = mpsc::channel(64);
            let state = shared_state(&["alpha"]);
            if targets.is_empty() {
                reserve_resolve_wait(&state, "alpha").await;
            }

            let supervisor = supervisor(dir.path().to_path_buf(), tx, state.clone(), None);

            supervisor
                .start_run(targets.clone(), false)
                .await
                .expect("the launch must dispatch");

            let started = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
                .await
                .expect("the dispatched run must announce itself")
                .expect("the dispatched run must emit its first event");
            match started {
                ExecutionEvent::Log(entry) => assert!(
                    entry.message.contains(PARALLEL_START_MARKER),
                    "every launch must reach the worktree orchestrator, got: {}",
                    entry.message
                ),
                other => panic!("expected the worktree startup log, got: {other:?}"),
            }

            let (handle, cancel, _scope) = supervisor.take_run();
            if let Some(cancel) = cancel {
                cancel.cancel();
            }
            if let Some(handle) = handle {
                handle.abort();
                let _ = handle.await;
            }

            if targets.is_empty() {
                assert_eq!(
                    state.read().await.resolve_wait_change_ids(),
                    vec!["alpha".to_string()],
                    "the dispatched run must not wipe the intent it was started to consume"
                );
            }
        }
    }

    /// An opted-in upstream session is never refused: there is no dispatch path
    /// left that carries no upstream runtime.
    #[tokio::test]
    async fn an_opted_in_upstream_session_is_never_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (tx, _rx) = mpsc::channel(64);
        let state = shared_state(&["alpha"]);
        reserve_resolve_wait(&state, "alpha").await;

        let supervisor = supervisor(
            dir.path().to_path_buf(),
            tx,
            state,
            Some(crate::upstream::UpstreamRuntime {
                config: crate::upstream::UpstreamIntegrationConfig::new("origin", "exit 0"),
                branch: "main".to_string(),
            }),
        );

        supervisor
            .start_run(Vec::new(), false)
            .await
            .expect("the launch keeps the publication contract");

        let (handle, cancel, _scope) = supervisor.take_run();
        if let Some(cancel) = cancel {
            cancel.cancel();
        }
        if let Some(handle) = handle {
            handle.abort();
            let _ = handle.await;
        }
    }
}
