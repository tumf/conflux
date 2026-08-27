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
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

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

/// Bounded budget for proving quiescence after the run task has been drained.
///
/// It runs *after* the orchestrator shutdown grace, so it covers only the
/// escalation of identities that grace could not prove — not the run's own
/// cancellation window.
const EXTERNAL_SHUTDOWN_QUIESCENCE_BUDGET: Duration = Duration::from_secs(15);

/// What a bounded TUI shutdown proved about the run it was asked to stop.
///
/// The distinction that matters is the last variant: a TUI that cannot prove
/// its owned process groups are empty must not report a clean stop, because
/// detached agent, shell, or test descendants would keep mutating the managed
/// worktree after the operator's terminal came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunShutdownOutcome {
    /// No run was live, so there was nothing to cancel and nothing to clean.
    NoActiveRun,
    /// The run was drained and every owned process group is proven empty.
    Quiescent {
        /// Owned identities that only managed escalation could prove quiescent.
        escalated: usize,
    },
    /// Bounded cleanup expired or left members behind.
    CleanupUnconfirmed {
        /// One bounded, actionable operator-facing summary.
        diagnostics: String,
    },
}

impl RunShutdownOutcome {
    /// Whether the process may exit reporting a clean stop.
    #[allow(dead_code)] // Read by shutdown-outcome coverage; the binary uses `into_exit_result`.
    pub fn is_clean(&self) -> bool {
        !matches!(self, Self::CleanupUnconfirmed { .. })
    }

    /// Classify one bounded cleanup barrier result.
    ///
    /// Kept as its own function because it is the step that decides what the
    /// operator is told, and it must be exercisable over every verdict —
    /// including the ones a real process only reaches by outliving SIGKILL.
    pub(crate) fn from_cleanup(cleanup: &crate::ai_command_runner::RunCommandScopeCleanup) -> Self {
        if cleanup.is_quiescent() {
            Self::Quiescent {
                escalated: cleanup.escalated,
            }
        } else {
            Self::CleanupUnconfirmed {
                diagnostics: cleanup.diagnostics(),
            }
        }
    }

    /// The process-level result a TUI exit reports for this outcome.
    ///
    /// Cleanup that could not be proven means owned descendants may still be
    /// mutating the managed worktree, so the process must exit non-zero with
    /// the diagnostics rather than let the operator believe the run stopped.
    pub(crate) fn into_exit_result(self) -> crate::error::Result<()> {
        match self {
            Self::CleanupUnconfirmed { diagnostics } => {
                Err(crate::error::OrchestratorError::AgentCommand(format!(
                    "TUI shutdown could not prove that run-owned processes stopped: {diagnostics}. \
                     Workspace contents were left in place; terminate the surviving processes \
                     before starting another run."
                )))
            }
            Self::NoActiveRun | Self::Quiescent { .. } => Ok(()),
        }
    }
}

/// An external SIGINT/SIGTERM asking this TUI process to stop.
///
/// The TUI owns the terminal in raw mode, so a keyboard Ctrl+C arrives as a key
/// event and never as a signal; this exists for the external `kill -INT` /
/// `kill -TERM` case, which nothing in the event loop would otherwise observe.
///
/// The watcher only *records* the request. It never exits the process, because
/// exiting from a signal handler is exactly what leaves detached agent, shell,
/// and test descendants behind: the request is drained by the event loop, which
/// then leaves through [`TuiRunSupervisor::shutdown_run`] like every other exit.
#[derive(Clone, Default)]
pub struct ExternalShutdownRequest {
    requested: Arc<AtomicBool>,
}

impl ExternalShutdownRequest {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether an external signal has asked this process to stop.
    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }

    /// Record the request. Idempotent: a second signal adds nothing to observe.
    pub fn request(&self) {
        self.requested.store(true, Ordering::SeqCst);
    }

    /// Install the process-wide SIGINT and SIGTERM watchers.
    ///
    /// Failing to install a handler is not fatal: the TUI still runs, and the
    /// default disposition stays in effect for that one signal. Refusing to
    /// start over it would be a worse trade than losing the graceful path.
    pub fn install(&self) {
        #[cfg(unix)]
        {
            let request = self.clone();
            tokio::spawn(async move {
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(mut sigterm) => {
                        sigterm.recv().await;
                        info!("Received SIGTERM; routing the TUI through run shutdown");
                        request.request();
                    }
                    Err(err) => warn!(error = %err, "Could not install the TUI SIGTERM handler"),
                }
            });
        }

        let request = self.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                info!("Received SIGINT; routing the TUI through run shutdown");
                request.request();
            }
        });
    }
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
    ///
    /// The managed ownership graph a targeted force-stop signals through, read
    /// per call rather than captured: a scope belongs to one run, so a caller
    /// holding an old one would be addressing PGIDs that were reaped runs ago.
    /// `None` between runs means this process owns no managed process at all.
    pub fn run_command_scope(&self) -> Option<RunCommandScope> {
        lock(&self.slots.scope).clone()
    }

    /// Install run slots directly.
    ///
    /// Shutdown coverage needs a run whose scope it controls; spawning the real
    /// orchestrator would give it a scope with nothing registered in it, which
    /// is the one case shutdown has nothing to prove about.
    #[cfg(test)]
    pub(crate) fn install_run_for_test(
        &self,
        handle: Option<tokio::task::JoinHandle<Result<()>>>,
        cancel: Option<CancellationToken>,
        scope: Option<RunCommandScope>,
    ) {
        *lock(&self.slots.handle) = handle;
        *lock(&self.slots.cancel) = cancel;
        *lock(&self.slots.scope) = scope;
    }

    /// The cancellation token of the live run, if any.
    pub fn cancel_token(&self) -> Option<CancellationToken> {
        lock(&self.slots.cancel).clone()
    }

    /// The one bounded shutdown boundary every TUI exit path goes through.
    ///
    /// Keyboard quit, an operator force stop, and an external SIGINT/SIGTERM all
    /// call this rather than exiting on their own. A signal that terminated the
    /// process directly would skip three things that only this sequence does:
    /// closing command admission, running the graceful-then-forceful cleanup of
    /// owned process groups, and — through the run cancellation it propagates —
    /// letting an in-flight Apply preserve its dirty worktree before the run ends.
    ///
    /// The sequence is ordered: admission closes first so no command can start
    /// while the process is already leaving, then the run is cancelled and
    /// drained, and only then is quiescence proven. A failure is returned, never
    /// swallowed: the caller decides the exit status, but it cannot be told the
    /// stop was clean when it was not.
    pub async fn shutdown_run(&self, grace_period: Duration) -> RunShutdownOutcome {
        self.shutdown_run_with_budget(grace_period, EXTERNAL_SHUTDOWN_QUIESCENCE_BUDGET)
            .await
    }

    /// [`Self::shutdown_run`] with an explicit quiescence budget.
    ///
    /// The budget is a parameter only so coverage can drive the unprovable-cleanup
    /// branch without waiting out the production window; production always uses
    /// [`EXTERNAL_SHUTDOWN_QUIESCENCE_BUDGET`].
    pub(crate) async fn shutdown_run_with_budget(
        &self,
        grace_period: Duration,
        quiescence_budget: Duration,
    ) -> RunShutdownOutcome {
        let (handle, cancel, scope) = self.take_run();
        if handle.is_none() && cancel.is_none() && scope.is_none() {
            return RunShutdownOutcome::NoActiveRun;
        }

        // Admission closes before cancellation rather than after the grace
        // period, so a command cannot be admitted into a run that is already
        // being torn down.
        if let Some(scope) = &scope {
            scope.close();
        }

        crate::tui::runner::shutdown_local_orchestrator_task(
            handle,
            cancel,
            scope.clone(),
            grace_period,
        )
        .await;

        // A run with no scope owns no commands, so there is no process identity
        // left that could still hold the managed worktree.
        let Some(scope) = scope else {
            return RunShutdownOutcome::Quiescent { escalated: 0 };
        };

        let cleanup = scope.wait_quiescent(quiescence_budget).await;
        let outcome = RunShutdownOutcome::from_cleanup(&cleanup);
        match &outcome {
            RunShutdownOutcome::CleanupUnconfirmed { diagnostics } => warn!(
                "TUI shutdown could not prove run-owned process cleanup: {}",
                diagnostics
            ),
            _ => info!(
                escalated = cleanup.escalated,
                "TUI shutdown proved every owned process group quiescent"
            ),
        }
        outcome
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
            let run_graceful_stop = graceful_stop.clone();
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
                    run_graceful_stop,
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
    use crate::orchestration::state::ReducerCommand;
    use crate::tui::events::OrchestratorEvent;
    use tokio::sync::mpsc;

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

    /// Where an externally-signalled TUI exit has to end up.
    ///
    /// A signal handler that exits the process directly is the failure this
    /// boundary exists to prevent: the terminal comes back, the operator
    /// believes the run stopped, and detached agent, shell, and test
    /// descendants keep mutating the managed worktree. So the contract is not
    /// "we sent a signal" but "admission is closed, the run is cancelled, and
    /// the owned groups are *proven* empty" — and when that proof cannot be
    /// produced, the process must say so instead of reporting a clean stop.
    mod external_shutdown {
        use super::*;
        use crate::ai_command_runner::{AiCommandRunner, RunCommandScope, RunCommandScopeCleanup};

        /// Long enough that nothing but shutdown can end these commands.
        const NEVER_ENDS: &str = "sleep 300";

        fn test_supervisor() -> TuiRunSupervisor {
            let dir = tempfile::tempdir().expect("tempdir");
            let (tx, _rx) = mpsc::channel(64);
            let state = shared_state(&["alpha"]);
            let supervisor = supervisor(dir.path().to_path_buf(), tx, state, None);
            // The temp dir only has to outlive construction: nothing in these
            // tests dispatches a run that would read from it.
            std::mem::forget(dir);
            supervisor
        }

        fn scoped_runner(scope: &RunCommandScope) -> AiCommandRunner {
            let config = OrchestratorConfig {
                command_queue_stagger_delay_ms: Some(0),
                command_queue_max_retries: Some(1),
                command_queue_retry_delay_ms: Some(0),
                // Only the shutdown path may end these commands.
                command_inactivity_timeout_secs: Some(0),
                command_inactivity_timeout_max_retries: Some(0),
                command_max_runtime_secs: Some(0),
                ..OrchestratorConfig::default()
            };
            AiCommandRunner::for_run(
                &config,
                Arc::new(tokio::sync::Mutex::new(None)),
                scope.clone(),
            )
        }

        /// A task that never finishes on its own, standing in for an
        /// orchestrator that has stopped cooperating.
        fn uncooperative_run() -> tokio::task::JoinHandle<Result<()>> {
            tokio::spawn(async {
                std::future::pending::<()>().await;
                Ok(())
            })
        }

        /// A signal is a *request*, never an exit. Recording it is all the
        /// handler may do, because the event loop is what carries it into the
        /// bounded shutdown boundary.
        #[test]
        fn an_external_signal_is_recorded_as_a_request() {
            let request = ExternalShutdownRequest::new();
            assert!(
                !request.is_requested(),
                "a fresh process has not been asked to stop"
            );

            request.request();
            assert!(request.is_requested());

            // A second signal adds nothing new to observe, so escalation cannot
            // be built on "we saw it twice" by accident.
            request.request();
            assert!(request.is_requested());

            // The watcher shares one flag with the loop that drains it.
            let observer = request.clone();
            assert!(observer.is_requested());
        }

        /// A remote TUI owns no local run, so closing it must send no stop.
        #[tokio::test]
        async fn a_supervisor_with_no_run_reports_no_active_run() {
            let supervisor = test_supervisor();
            assert_eq!(
                supervisor.shutdown_run(Duration::from_millis(50)).await,
                RunShutdownOutcome::NoActiveRun
            );
        }

        /// The whole ordered sequence, against a real owned command: admission
        /// closes, the run is cancelled, the registration drains, and no owned
        /// process identity is left behind.
        #[tokio::test]
        async fn external_shutdown_closes_admission_drains_and_proves_quiescence() {
            let supervisor = test_supervisor();
            let scope = RunCommandScope::new();
            let cancel = CancellationToken::new();
            scope.link_cancellation(cancel.clone());
            let runner = scoped_runner(&scope);

            let (handle, mut rx) = runner
                .execute_streaming_with_retry(NEVER_ENDS, None, Some("apply"), Some("alpha"))
                .await
                .expect("an open scope admits the command");
            // Drain so the runner task is never blocked on its output channel.
            let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });

            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while scope.active_executions() == 0 {
                assert!(
                    std::time::Instant::now() < deadline,
                    "arrangement failed: the command never registered"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            // The workspace future that held the handle is gone, exactly as
            // `JoinSet::abort_all` leaves it.
            drop(handle);

            supervisor.install_run_for_test(
                Some(uncooperative_run()),
                Some(cancel.clone()),
                Some(scope.clone()),
            );

            let outcome = supervisor
                .shutdown_run_with_budget(Duration::from_millis(50), Duration::from_secs(30))
                .await;
            drain.await.expect("the drain task joins");

            assert!(
                matches!(outcome, RunShutdownOutcome::Quiescent { .. }),
                "the owned group must be proven empty: {outcome:?}"
            );
            assert!(outcome.is_clean());
            assert!(
                scope.is_closed(),
                "command admission must be closed by shutdown"
            );
            assert!(
                cancel.is_cancelled(),
                "the run must be cancelled, which is what lets an in-flight \
                 Apply preserve its worktree before the run ends"
            );
            assert_eq!(
                scope.active_executions(),
                0,
                "every registered execution must have drained"
            );
            assert!(
                scope.retained_process_ids().is_empty(),
                "no owned process identity may remain after a clean shutdown"
            );

            // Retry and spawn admission stay closed: nothing may start into a
            // run that is already gone.
            let (mut refused, mut refused_rx) = runner
                .execute_streaming_with_retry("echo never", None, Some("apply"), Some("beta"))
                .await
                .expect("the call returns a refusal rather than launching");
            while refused_rx.recv().await.is_some() {}
            assert!(
                !refused.wait().await.expect("status").success(),
                "a command admitted after shutdown would outlive the process"
            );

            // The slots are consumed, so a second shutdown has nothing to stop.
            assert_eq!(
                supervisor.shutdown_run(Duration::from_millis(10)).await,
                RunShutdownOutcome::NoActiveRun
            );
        }

        /// Cleanup that cannot be proven is never rounded up to a clean stop.
        ///
        /// This is the branch that decides what the operator is told, and both
        /// of its failure shapes matter: a barrier that expired with a runner
        /// still active, and a barrier that finished but left an owned identity
        /// it could not verify. Neither can be arranged with a real process —
        /// force-kill escalation eventually wins against anything a test can
        /// spawn — so the verdicts are driven directly.
        #[test]
        fn an_unproven_barrier_is_never_reported_as_a_clean_stop() {
            let expired = RunCommandScopeCleanup {
                escalated: 0,
                unconfirmed: Vec::new(),
                timed_out: true,
            };
            let outcome = RunShutdownOutcome::from_cleanup(&expired);
            let RunShutdownOutcome::CleanupUnconfirmed { diagnostics } = &outcome else {
                panic!("an expired barrier is not proof of quiescence: {outcome:?}");
            };
            assert!(!outcome.is_clean());
            assert!(
                diagnostics.contains("unconfirmed") && diagnostics.contains("timed_out=true"),
                "the operator needs actionable diagnostics: {diagnostics}"
            );

            let survivors = RunCommandScopeCleanup {
                escalated: 1,
                unconfirmed: vec!["apply/alpha pgid=4242 members remain".to_string()],
                timed_out: false,
            };
            let outcome = RunShutdownOutcome::from_cleanup(&survivors);
            let RunShutdownOutcome::CleanupUnconfirmed { diagnostics } = &outcome else {
                panic!("a surviving owned identity is not a clean stop: {outcome:?}");
            };
            assert!(!outcome.is_clean());
            assert!(
                diagnostics.contains("apply/alpha"),
                "the surviving identity must be named: {diagnostics}"
            );
        }

        #[test]
        fn a_proven_barrier_is_a_clean_stop_even_after_escalation() {
            let escalated = RunCommandScopeCleanup {
                escalated: 2,
                unconfirmed: Vec::new(),
                timed_out: false,
            };
            assert_eq!(
                RunShutdownOutcome::from_cleanup(&escalated),
                RunShutdownOutcome::Quiescent { escalated: 2 },
                "forced-but-verified cleanup is still verified cleanup"
            );
            assert!(RunShutdownOutcome::from_cleanup(&escalated).is_clean());
        }

        /// The exit status is the operator-visible half of the contract: a
        /// shutdown that could not prove cleanup must fail the process, and a
        /// proven one must not.
        #[test]
        fn unproven_cleanup_exits_non_zero_with_its_diagnostics() {
            let error = RunShutdownOutcome::CleanupUnconfirmed {
                diagnostics: "run command cleanup unconfirmed (timed_out=true): apply/alpha"
                    .to_string(),
            }
            .into_exit_result()
            .expect_err("an unproven shutdown must not report success");
            let rendered = error.to_string();
            assert!(
                rendered.contains("could not prove that run-owned processes stopped"),
                "the failure must say what could not be proven: {rendered}"
            );
            assert!(
                rendered.contains("apply/alpha"),
                "the barrier diagnostics must reach the operator: {rendered}"
            );
            assert!(
                rendered.contains("Workspace contents were left in place"),
                "recovery must stay possible and be stated: {rendered}"
            );

            assert!(RunShutdownOutcome::NoActiveRun.into_exit_result().is_ok());
            assert!(RunShutdownOutcome::Quiescent { escalated: 0 }
                .into_exit_result()
                .is_ok());
            assert!(RunShutdownOutcome::Quiescent { escalated: 3 }
                .into_exit_result()
                .is_ok());
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
