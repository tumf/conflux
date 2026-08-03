//! Shared, frontend-independent run lifecycle service.
//!
//! [`crate::orchestration::operator_command::OperatorCommandService`] owns
//! per-change intent (marks, queue membership, cancellation, retry routing).
//! This module owns the commands that act on the *run* itself — start, graceful
//! stop, cancel stop, force stop, and resolve — plus the scheduler dispatch that
//! makes a retry real.
//!
//! Every frontend calls the same instance. A frontend supplies two things and
//! nothing else:
//!
//! - the operator-facing [`OperatorMode`] it observed, so the lifecycle matrix
//!   is evaluated against one vocabulary;
//! - a projection of the returned [`RunControlOutcome`] into its own logs,
//!   events, or response body.
//!
//! What a frontend must never do is decide *whether* a command was successful.
//! An outcome is produced only after the reducer transition and the scheduler
//! side effect the outcome names have actually happened, so no adapter can
//! report success for a message it merely enqueued.
//!
//! All state here is process-local and in-memory. Nothing in this module is
//! durable workflow evidence: a restart drops every reservation and the next
//! action is recomputed from the workspace alone.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::orchestration::operator_command::{
    OperatorCommandError, OperatorCommandService, OperatorMode, RetryPlan,
};
use crate::orchestration::state::{OrchestratorState, ReduceOutcome, ReducerCommand};
use crate::tui::stop_classification::{StopActivitySnapshot, StopClassification};

/// Display status a change must carry before a resolve can be reserved for it.
const MERGE_WAIT_STATUS: &str = "merge wait";

/// Display status a change must carry before it can be started.
const NOT_QUEUED_STATUS: &str = "not queued";

// ============================================================================
// Vocabulary
// ============================================================================

/// Which run-lifecycle command produced an outcome or a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCommandKind {
    /// Start (or resume, or retry) the run.
    Start,
    /// Request a graceful stop.
    Stop,
    /// Withdraw a pending graceful stop.
    CancelStop,
    /// Stop immediately.
    ForceStop,
    /// Retry one or more changes.
    Retry,
    /// Resolve a merge wait.
    Resolve,
}

impl RunCommandKind {
    /// Operator-facing name used in messages.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::CancelStop => "cancel stop",
            Self::ForceStop => "force stop",
            Self::Retry => "retry",
            Self::Resolve => "resolve",
        }
    }
}

/// The scheduler side effect a command actually caused.
///
/// This is the field that makes "the run was dispatched" checkable: it is set
/// from the return of the scheduler port, never from an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerEffect {
    /// A new scheduler run was spawned.
    Started,
    /// A scheduler that was already alive was woken.
    Notified,
    /// No scheduler dispatch was required.
    None,
}

impl SchedulerEffect {
    /// True when the scheduler was really started or woken.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn dispatched(self) -> bool {
        matches!(self, Self::Started | Self::Notified)
    }
}

/// Why a run-lifecycle command changed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunNoOpReason {
    /// The change already held a resolve reservation.
    ResolveAlreadyReserved {
        /// Target change.
        change_id: String,
    },
    /// No marked change carried retryable evidence.
    NoRetryableTarget,
}

/// What a run-lifecycle command actually did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunControlOutcome {
    /// A run was dispatched over `change_ids`.
    RunDispatched {
        /// Targets the run was dispatched for, in request order.
        change_ids: Vec<String>,
        /// True when the run must consume reconciled holds instead of rerunning apply.
        explicit_retry: bool,
        /// What actually happened to the scheduler.
        scheduler: SchedulerEffect,
    },
    /// A graceful stop was requested.
    StopRequested,
    /// A pending graceful stop was withdrawn.
    StopCancelled,
    /// An immediate stop was applied.
    ForceStopped {
        /// Truthful runtime-activity classification for this stop.
        classification: StopClassification,
        /// True when the scheduler owns the terminal stop and must reach its
        /// cancellation-safe boundary before the run is reported stopped.
        awaiting_safe_boundary: bool,
    },
    /// A resolve reservation was taken.
    ResolveReserved {
        /// Target change.
        change_id: String,
        /// Where the reservation landed.
        reservation: ResolveReservation,
        /// What actually happened to the scheduler.
        scheduler: SchedulerEffect,
    },
    /// Nothing changed.
    NoOp {
        /// Why nothing changed.
        reason: RunNoOpReason,
    },
}

/// Why a run-lifecycle command was refused without side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunControlError {
    /// The command is not valid for the current operator mode.
    InvalidMode {
        /// Command that was refused.
        command: RunCommandKind,
        /// Mode the request arrived in.
        mode: OperatorMode,
    },
    /// No target satisfied the command's eligibility rules.
    NoEligibleTarget {
        /// Command that was refused.
        command: RunCommandKind,
        /// Actionable operator-facing detail.
        detail: String,
    },
    /// The named change cannot accept this command right now.
    TargetIneligible {
        /// Command that was refused.
        command: RunCommandKind,
        /// Target change.
        change_id: String,
        /// Display status at refusal time.
        display_status: String,
    },
    /// The runtime refused to start or wake the scheduler.
    DispatchFailed {
        /// Command that was refused.
        command: RunCommandKind,
        /// Failure detail from the runtime.
        message: String,
    },
    /// A per-change operator command refused the request.
    Operator(OperatorCommandError),
}

impl std::fmt::Display for RunControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMode { command, mode } => {
                write!(f, "{} is not available in {mode:?} mode", command.as_str())
            }
            Self::NoEligibleTarget { command, detail } => {
                write!(f, "{} has no eligible target: {detail}", command.as_str())
            }
            Self::TargetIneligible {
                command,
                change_id,
                display_status,
            } => write!(
                f,
                "{} is not available for '{change_id}' with status '{display_status}'",
                command.as_str()
            ),
            Self::DispatchFailed { command, message } => {
                write!(
                    f,
                    "{} could not dispatch the run: {message}",
                    command.as_str()
                )
            }
            Self::Operator(error) => write!(f, "{error}"),
        }
    }
}

impl From<OperatorCommandError> for RunControlError {
    fn from(error: OperatorCommandError) -> Self {
        Self::Operator(error)
    }
}

/// Result alias for run-lifecycle command execution.
pub type RunControlResult<T> = std::result::Result<T, RunControlError>;

// ============================================================================
// Resolve reservations (process-local)
// ============================================================================

/// Where a resolve reservation landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveReservation {
    /// The change became the single active resolver.
    Active,
    /// The change is waiting behind the active resolver.
    Queued {
        /// 1-based distance behind the active resolver.
        position: usize,
    },
}

/// Process-local single-resolver reservation ledger.
///
/// One merge resolution runs at a time. Everything else waits in FIFO order,
/// and a change can hold at most one reservation, so a duplicate request never
/// creates a second queue entry.
///
/// Reservations are in-memory only: a restart begins with no active resolver and
/// an empty queue, and merge-wait routing is recomputed from workspace evidence.
#[derive(Debug, Default)]
pub struct ResolveReservations {
    inner: Mutex<ResolveInner>,
}

#[derive(Debug, Default)]
struct ResolveInner {
    active: Option<String>,
    waiting: VecDeque<String>,
}

impl ResolveReservations {
    /// Create an empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ResolveInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Reserve a resolve slot for `change_id`.
    ///
    /// Returns `None` when the change already holds a reservation, which is what
    /// makes a duplicate submission a no-op instead of a second queue entry.
    pub fn reserve(&self, change_id: &str) -> Option<ResolveReservation> {
        let mut guard = self.lock();
        if guard.active.as_deref() == Some(change_id)
            || guard.waiting.iter().any(|q| q == change_id)
        {
            return None;
        }
        if guard.active.is_none() {
            guard.active = Some(change_id.to_string());
            return Some(ResolveReservation::Active);
        }
        guard.waiting.push_back(change_id.to_string());
        Some(ResolveReservation::Queued {
            position: guard.waiting.len(),
        })
    }

    /// True when any resolve is currently active.
    pub fn is_active(&self) -> bool {
        self.lock().active.is_some()
    }

    /// The change that currently owns the resolver, if any.
    pub fn active(&self) -> Option<String> {
        self.lock().active.clone()
    }

    /// True when the change holds an active or queued reservation.
    pub fn is_reserved(&self, change_id: &str) -> bool {
        let guard = self.lock();
        guard.active.as_deref() == Some(change_id)
            || guard.waiting.iter().any(|queued| queued == change_id)
    }

    /// Queued changes in FIFO order (the active resolver is not included).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn waiting(&self) -> Vec<String> {
        self.lock().waiting.iter().cloned().collect()
    }

    /// True when at least one change is waiting behind the active resolver.
    pub fn has_waiting(&self) -> bool {
        !self.lock().waiting.is_empty()
    }

    /// Record that `change_id` is the change the runtime actually started.
    ///
    /// Used by the event path: a resolve can start from scheduler-owned work that
    /// never went through [`Self::reserve`], and the ledger must still report one
    /// active resolver.
    pub fn mark_active(&self, change_id: &str) {
        let mut guard = self.lock();
        guard.waiting.retain(|queued| queued != change_id);
        guard.active = Some(change_id.to_string());
    }

    /// Release the active resolver and promote the next waiting change.
    ///
    /// Returns the promoted change so the caller can dispatch it. The promoted
    /// change loses its reservation, so re-submitting it reserves cleanly.
    pub fn finish_active(&self) -> Option<String> {
        let mut guard = self.lock();
        guard.active = None;
        guard.waiting.pop_front()
    }

    /// Drop one specific reservation, preserving FIFO order of the rest.
    ///
    /// Returns true when a reservation was really dropped.
    pub fn cancel(&self, change_id: &str) -> bool {
        let mut guard = self.lock();
        let was_active = guard.active.as_deref() == Some(change_id);
        if was_active {
            guard.active = None;
        }
        let before = guard.waiting.len();
        guard.waiting.retain(|queued| queued != change_id);
        was_active || guard.waiting.len() != before
    }

    /// Drop every reservation.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn clear(&self) {
        let mut guard = self.lock();
        guard.active = None;
        guard.waiting.clear();
    }
}

// ============================================================================
// Start eligibility (process-local)
// ============================================================================

/// Process-local publication of start-time target eligibility.
///
/// Parallel-mode eligibility is derived from workspace observation that only the
/// frontend running the refresh loop performs. Publishing it in the shared
/// [`ParallelRuntime`] is what lets the shared service apply one guard to every
/// frontend instead of letting each one re-derive it — and it is the same store
/// the operator command service mutates when a remote client or a keypress
/// toggles parallel mode, so the guard can never read a toggle nobody set.
pub use crate::orchestration::operator_command::ParallelRuntime as StartEligibility;

// ============================================================================
// Ports
// ============================================================================

/// The run-scheduling runtime the service drives.
///
/// Every method reports what really happened. `start_run` returning `Ok` means a
/// scheduler task exists; a runtime refusal is an `Err`, never a silent success.
#[async_trait]
pub trait RunSchedulerPort: Send + Sync {
    /// True when a scheduler task is currently alive.
    fn is_running(&self) -> bool;

    /// Spawn a scheduler run over `targets`.
    ///
    /// An empty `targets` list is a scheduler-owned run that consumes
    /// reducer-owned intent (a manual resolve, for example).
    async fn start_run(&self, targets: Vec<String>, explicit_retry: bool) -> Result<(), String>;

    /// Wake a scheduler that is already alive.
    async fn notify_scheduler(&self);

    /// Request cancellation of the live run.
    async fn cancel_run(&self);

    /// Set or clear the graceful-stop request.
    fn set_graceful_stop(&self, requested: bool);

    /// Take one runtime activity snapshot for an immediate stop decision.
    async fn stop_activity(&self) -> StopActivitySnapshot;
}

// ============================================================================
// Service
// ============================================================================

/// Process-local application service for run-lifecycle commands.
pub struct RunControlService {
    state: Arc<RwLock<OrchestratorState>>,
    operator: Arc<OperatorCommandService>,
    scheduler: Arc<dyn RunSchedulerPort>,
    resolves: Arc<ResolveReservations>,
    eligibility: Arc<StartEligibility>,
}

impl RunControlService {
    /// Build a service over the shared reducer state and the run scheduler.
    pub fn new(
        state: Arc<RwLock<OrchestratorState>>,
        operator: Arc<OperatorCommandService>,
        scheduler: Arc<dyn RunSchedulerPort>,
        resolves: Arc<ResolveReservations>,
        eligibility: Arc<StartEligibility>,
    ) -> Self {
        Self {
            state,
            operator,
            scheduler,
            resolves,
            eligibility,
        }
    }

    /// The per-change operator command service this service composes with.
    pub fn operator(&self) -> Arc<OperatorCommandService> {
        self.operator.clone()
    }

    async fn display_status(&self, change_id: &str) -> String {
        self.state
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }

    /// Dispatch work to the scheduler and report what actually happened.
    async fn dispatch(
        &self,
        command: RunCommandKind,
        targets: Vec<String>,
        explicit_retry: bool,
    ) -> RunControlResult<SchedulerEffect> {
        if self.scheduler.is_running() {
            self.scheduler.notify_scheduler().await;
            return Ok(SchedulerEffect::Notified);
        }
        self.scheduler
            .start_run(targets, explicit_retry)
            .await
            .map(|()| SchedulerEffect::Started)
            .map_err(|message| RunControlError::DispatchFailed { command, message })
    }

    // ------------------------------------------------------------------
    // Start
    // ------------------------------------------------------------------

    /// Start, resume, or retry the run for the authoritative marked target set.
    ///
    /// `Select` and `Stopped` start marked rows that are not queued yet.
    /// `Error` routes the marked rows through retry classification instead, so a
    /// reconciled acceptance hold resumes rather than rerunning apply. A mode
    /// with a live run owns its own queue mutation and refuses start outright.
    pub async fn start(&self, mode: OperatorMode) -> RunControlResult<RunControlOutcome> {
        match mode {
            OperatorMode::Running | OperatorMode::Stopping => Err(RunControlError::InvalidMode {
                command: RunCommandKind::Start,
                mode,
            }),
            OperatorMode::Error => self.start_retry().await,
            OperatorMode::Select | OperatorMode::Stopped => self.start_marked().await,
        }
    }

    /// Marked change IDs that are eligible to enter a new run.
    ///
    /// This is the authoritative target set: it reads the shared execution-mark
    /// store and the reducer's display status, never a frontend's row cache.
    pub async fn start_targets(&self) -> Vec<String> {
        let marked = self.operator.marks().marked_ids();
        let guard = self.state.read().await;
        marked
            .into_iter()
            .filter(|id| guard.display_status(id) == NOT_QUEUED_STATUS)
            .collect()
    }

    async fn start_marked(&self) -> RunControlResult<RunControlOutcome> {
        let marked = self.operator.marks().marked_ids();
        if marked.is_empty() {
            return Err(RunControlError::NoEligibleTarget {
                command: RunCommandKind::Start,
                detail: "no change carries an execution mark".to_string(),
            });
        }

        // The fence is applied to the *complete* marked set before anything is
        // narrowed down, so parallel start is all-or-nothing: one ineligible
        // target refuses the whole operation instead of quietly starting the
        // eligible remainder, which is a target set the operator never asked
        // for.
        let rejected = self.eligibility.rejected(&marked);
        if !rejected.is_empty() {
            return Err(RunControlError::NoEligibleTarget {
                command: RunCommandKind::Start,
                detail: format!(
                    "parallel mode requires committed changes with no uncommitted files; \
                     ineligible marked targets: {}",
                    rejected.join(", ")
                ),
            });
        }

        let targets = self.start_targets().await;
        if targets.is_empty() {
            return Err(RunControlError::NoEligibleTarget {
                command: RunCommandKind::Start,
                detail: format!(
                    "no marked change is startable ({} marked, none with status '{NOT_QUEUED_STATUS}')",
                    marked.len()
                ),
            });
        }

        // Queue intent before dispatch: a scheduler woken by the dispatch below
        // must already see the work it is being woken for.
        {
            let mut guard = self.state.write().await;
            for id in &targets {
                guard.apply_command(ReducerCommand::AddToQueue(id.clone()));
            }
        }

        let scheduler = self
            .dispatch(RunCommandKind::Start, targets.clone(), false)
            .await?;
        Ok(RunControlOutcome::RunDispatched {
            change_ids: targets,
            explicit_retry: false,
            scheduler,
        })
    }

    async fn start_retry(&self) -> RunControlResult<RunControlOutcome> {
        let marked = self.operator.marks().marked_ids();
        if marked.is_empty() {
            return Err(RunControlError::NoEligibleTarget {
                command: RunCommandKind::Start,
                detail: "no change carries an execution mark".to_string(),
            });
        }
        self.retry_errors(&marked).await
    }

    // ------------------------------------------------------------------
    // Retry
    // ------------------------------------------------------------------

    /// Retry one change and prove the scheduler picked the work up.
    pub async fn retry_change(&self, change_id: &str) -> RunControlResult<RunControlOutcome> {
        let plan = self.operator.retry_change(change_id).await?;
        self.dispatch_retry(plan).await
    }

    /// Retry every change in `change_ids` that carries retryable evidence.
    ///
    /// Changes without retryable evidence are skipped, so a bulk retry never
    /// consumes an unsupported hold or destroys its blocker evidence.
    pub async fn retry_errors(&self, change_ids: &[String]) -> RunControlResult<RunControlOutcome> {
        let plan = self.operator.retry_errors(change_ids).await;
        self.dispatch_retry(plan).await
    }

    async fn dispatch_retry(&self, plan: RetryPlan) -> RunControlResult<RunControlOutcome> {
        if plan.is_empty() {
            return Ok(RunControlOutcome::NoOp {
                reason: RunNoOpReason::NoRetryableTarget,
            });
        }
        let scheduler = self
            .dispatch(
                RunCommandKind::Retry,
                plan.change_ids.clone(),
                plan.explicit_retry,
            )
            .await?;
        Ok(RunControlOutcome::RunDispatched {
            change_ids: plan.change_ids,
            explicit_retry: plan.explicit_retry,
            scheduler,
        })
    }

    // ------------------------------------------------------------------
    // Stop family
    // ------------------------------------------------------------------

    /// Request a graceful stop after the current change completes.
    pub async fn stop(&self, mode: OperatorMode) -> RunControlResult<RunControlOutcome> {
        if mode != OperatorMode::Running {
            return Err(RunControlError::InvalidMode {
                command: RunCommandKind::Stop,
                mode,
            });
        }
        self.scheduler.set_graceful_stop(true);
        Ok(RunControlOutcome::StopRequested)
    }

    /// Withdraw a pending graceful stop.
    pub async fn cancel_stop(&self, mode: OperatorMode) -> RunControlResult<RunControlOutcome> {
        if mode != OperatorMode::Stopping {
            return Err(RunControlError::InvalidMode {
                command: RunCommandKind::CancelStop,
                mode,
            });
        }
        self.scheduler.set_graceful_stop(false);
        Ok(RunControlOutcome::StopCancelled)
    }

    /// Stop immediately and report the truthful runtime-activity classification.
    ///
    /// Cancellation is issued for both reporting classes; the classification
    /// controls what may be *claimed* and whether the caller must wait for the
    /// scheduler's cancellation-safe boundary, never whether cleanup runs.
    pub async fn force_stop(&self, mode: OperatorMode) -> RunControlResult<RunControlOutcome> {
        if !matches!(mode, OperatorMode::Running | OperatorMode::Stopping) {
            return Err(RunControlError::InvalidMode {
                command: RunCommandKind::ForceStop,
                mode,
            });
        }

        let snapshot = self.scheduler.stop_activity().await;
        let classification = snapshot.classify();
        let scheduler_running = self.scheduler.is_running();

        self.scheduler.cancel_run().await;

        Ok(RunControlOutcome::ForceStopped {
            classification,
            awaiting_safe_boundary: scheduler_running
                && snapshot.scheduler_owns_cleanup()
                && classification.shutdown_barrier.is_required(),
        })
    }

    // ------------------------------------------------------------------
    // Resolve
    // ------------------------------------------------------------------

    /// Reserve a merge resolution for a change.
    ///
    /// The single-resolver rule is enforced here rather than in a frontend: the
    /// first valid request becomes the active resolver and dispatches the
    /// scheduler, later ones queue in FIFO order, and a duplicate request is a
    /// no-op that never creates a second entry.
    pub async fn resolve_merge(&self, change_id: &str) -> RunControlResult<RunControlOutcome> {
        if self.resolves.is_reserved(change_id) {
            return Ok(RunControlOutcome::NoOp {
                reason: RunNoOpReason::ResolveAlreadyReserved {
                    change_id: change_id.to_string(),
                },
            });
        }

        let display_status = self.display_status(change_id).await;
        if display_status != MERGE_WAIT_STATUS {
            return Err(RunControlError::TargetIneligible {
                command: RunCommandKind::Resolve,
                change_id: change_id.to_string(),
                display_status,
            });
        }

        // Reducer first: it owns whether the wait state really accepts a resolve
        // intent, so a stale target is refused before any reservation exists.
        let reduce_outcome = {
            let mut guard = self.state.write().await;
            guard.apply_command(ReducerCommand::ResolveMerge(change_id.to_string()))
        };
        if matches!(reduce_outcome, ReduceOutcome::NoOp) {
            return Err(RunControlError::TargetIneligible {
                command: RunCommandKind::Resolve,
                change_id: change_id.to_string(),
                display_status,
            });
        }

        let Some(reservation) = self.resolves.reserve(change_id) else {
            // Another caller reserved between the check and here.
            return Ok(RunControlOutcome::NoOp {
                reason: RunNoOpReason::ResolveAlreadyReserved {
                    change_id: change_id.to_string(),
                },
            });
        };

        // Only the active resolver dispatches. A queued reservation is consumed
        // when the active resolve finishes, so waking the scheduler for it now
        // would claim work that cannot start.
        let scheduler = match reservation {
            ResolveReservation::Active => {
                self.dispatch(RunCommandKind::Resolve, Vec::new(), false)
                    .await?
            }
            ResolveReservation::Queued { .. } => SchedulerEffect::None,
        };

        Ok(RunControlOutcome::ResolveReserved {
            change_id: change_id.to_string(),
            reservation,
            scheduler,
        })
    }
}

/// Test doubles shared by the service, TUI adapter, and `/api/v2` adapter tests.
///
/// Both adapters are verified against the *same* recorder, which is what makes a
/// cross-adapter comparison meaningful: an assertion that TUI and v2 produced the
/// same scheduler effect is comparing two runs of one instrumented runtime, not
/// two differently-stubbed ones.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// One scheduler interaction the service performed.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum SchedulerCall {
        /// `start_run` was called with these targets and retry flag.
        Started {
            /// Targets handed to the run.
            targets: Vec<String>,
            /// Explicit-retry flag handed to the run.
            explicit_retry: bool,
        },
        /// A live scheduler was woken.
        Notified,
        /// Run cancellation was requested.
        Cancelled,
        /// The graceful-stop flag was set to this value.
        GracefulStop(bool),
    }

    /// A [`RunSchedulerPort`] that records calls instead of spawning work.
    ///
    /// No process, task, or repository is involved, so tests over it stay
    /// unit-scoped while still proving the side effect really happened.
    #[derive(Debug)]
    pub(crate) struct RecordingScheduler {
        calls: Mutex<Vec<SchedulerCall>>,
        running: std::sync::atomic::AtomicBool,
        activity: Mutex<StopActivitySnapshot>,
        launch_failure: Mutex<Option<String>>,
    }

    impl Default for RecordingScheduler {
        fn default() -> Self {
            Self::new()
        }
    }

    impl RecordingScheduler {
        /// An idle scheduler with a known-empty, nothing-pending activity snapshot.
        pub(crate) fn new() -> Self {
            use crate::tui::stop_classification::{ExecutionEvidence, ShutdownWorkEvidence};
            Self {
                calls: Mutex::new(Vec::new()),
                running: std::sync::atomic::AtomicBool::new(false),
                activity: Mutex::new(StopActivitySnapshot {
                    execution_handles: ExecutionEvidence::Known { registered: 0 },
                    reducer_agent_execution_active: false,
                    shutdown_work: ShutdownWorkEvidence::Known { pending: false },
                }),
                launch_failure: Mutex::new(None),
            }
        }

        /// Report a live scheduler run.
        pub(crate) fn set_running(&self, running: bool) {
            self.running
                .store(running, std::sync::atomic::Ordering::SeqCst);
        }

        /// Replace the runtime activity snapshot force stop will read.
        pub(crate) fn set_activity(&self, activity: StopActivitySnapshot) {
            *self.activity.lock().unwrap() = activity;
        }

        /// Make the next `start_run` fail with `message`.
        pub(crate) fn fail_launch(&self, message: &str) {
            *self.launch_failure.lock().unwrap() = Some(message.to_string());
        }

        /// Every recorded call, in order.
        pub(crate) fn calls(&self) -> Vec<SchedulerCall> {
            self.calls.lock().unwrap().clone()
        }

        /// Targets of every `start_run`, in order.
        pub(crate) fn started_targets(&self) -> Vec<Vec<String>> {
            self.calls()
                .into_iter()
                .filter_map(|call| match call {
                    SchedulerCall::Started { targets, .. } => Some(targets),
                    _ => None,
                })
                .collect()
        }

        fn record(&self, call: SchedulerCall) {
            self.calls.lock().unwrap().push(call);
        }
    }

    #[async_trait]
    impl RunSchedulerPort for RecordingScheduler {
        fn is_running(&self) -> bool {
            self.running.load(std::sync::atomic::Ordering::SeqCst)
        }

        async fn start_run(
            &self,
            targets: Vec<String>,
            explicit_retry: bool,
        ) -> std::result::Result<(), String> {
            if let Some(message) = self.launch_failure.lock().unwrap().take() {
                return Err(message);
            }
            self.record(SchedulerCall::Started {
                targets,
                explicit_retry,
            });
            self.set_running(true);
            Ok(())
        }

        async fn notify_scheduler(&self) {
            self.record(SchedulerCall::Notified);
        }

        async fn cancel_run(&self) {
            self.record(SchedulerCall::Cancelled);
        }

        fn set_graceful_stop(&self, requested: bool) {
            self.record(SchedulerCall::GracefulStop(requested));
        }

        async fn stop_activity(&self) -> StopActivitySnapshot {
            *self.activity.lock().unwrap()
        }
    }
}

#[cfg(test)]
mod tests;
