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
    OperatorCommandError, OperatorCommandService, OperatorMode, RetryPlan, RetryRoute,
    RunBoundaryLiveness,
};
use crate::orchestration::state::{OrchestratorState, ReduceOutcome, ReducerCommand};
use crate::tui::stop_classification::{StopActivitySnapshot, StopClassification};

/// Display status a change must carry before a resolve can be reserved for it.
const MERGE_WAIT_STATUS: &str = "merge wait";

/// Display status a change must carry before it can be started.
const NOT_QUEUED_STATUS: &str = "not queued";

/// Display status of a change settled into the operator-`Stopped` outcome.
const STOPPED_STATUS: &str = "stopped";

/// Why a retry-eligible mark was not routed while ordinary Start work existed.
///
/// Start admits one class per request, so a retry-eligible row an operator can
/// see left behind has to be told what would make it selectable — otherwise the
/// exclusion reads as "your retryable change is not retryable".
const RETRY_DEFERRED_TO_ORDINARY_START: &str =
    "retry-class Start selects it only when no ordinary marked change is startable; \
     remove the ordinary marks first";

/// Why an ordinary mark was not routed while a run owned the lifecycle.
const ORDINARY_DEFERRED_TO_MARK_SETTLEMENT: &str =
    "a live run admits ordinary marks through mark settlement rather than Start";

/// A marked target that final admission did not route, and why.
///
/// A mark carries no eligibility of its own, so an operator may mark a row that
/// the run cannot currently take. The exclusion is therefore an ordinary
/// reportable outcome rather than an error, and it names the target so the
/// operator can tell *which* of their marks did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedTarget {
    /// Marked change that was not routed.
    pub change_id: String,
    /// Reducer display status that excluded it, read at admission.
    pub status: String,
    /// Why that status excluded it, when the status alone does not say so.
    ///
    /// A status like `merge wait` explains itself. A retry-eligible `error`
    /// excluded because ordinary Start work took priority does not: the same
    /// row would be routed by the very next request, so the reason has to
    /// travel with the exclusion.
    pub detail: Option<String>,
}

impl ExcludedTarget {
    /// Name a target whose status is the whole reason it was not routed.
    pub fn new(change_id: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            change_id: change_id.into(),
            status: status.into(),
            detail: None,
        }
    }

    /// Add the reason a status that admission *could* have routed was not.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The one operator-facing spelling of this exclusion.
    ///
    /// Both frontends render exclusions through it, so a keypress and an
    /// `/api/v2` response cannot describe the same excluded target differently.
    pub fn describe(&self) -> String {
        match &self.detail {
            Some(detail) => format!("{} ({}): {detail}", self.change_id, self.status),
            None => format!("{} ({})", self.change_id, self.status),
        }
    }
}

/// Name every excluded target with the status that excluded it.
///
/// Target-specific by construction: a marked row an operator can see excluded
/// has to be identifiable, or the diagnostic only says that *something* was
/// dropped.
fn describe_exclusions(excluded: &[ExcludedTarget]) -> String {
    excluded
        .iter()
        .map(ExcludedTarget::describe)
        .collect::<Vec<_>>()
        .join(", ")
}

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
        /// Marked targets the admission did not route, with their statuses.
        ///
        /// An admitted run is still reported truthfully: a marked row the run
        /// could not take is named here rather than silently dropped.
        excluded: Vec<ExcludedTarget>,
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
/// Worktree eligibility is derived from workspace observation that only the
/// frontend running the refresh loop performs. Publishing it in the shared
/// [`ParallelRuntime`] is what lets the shared service apply one guard to every
/// frontend instead of letting each one re-derive it.
pub use crate::orchestration::operator_command::ParallelRuntime as StartEligibility;

// ============================================================================
// Ports
// ============================================================================

/// A scheduler launch that has been validated but is not yet allowed to run.
///
/// Everything that can fail about a launch happens while the permit is being
/// *made*; [`RunPermit::activate`] cannot fail and cannot be interrupted. That
/// split is what lets the coordinator commit an accepted command's decision
/// state and dispatch its outcome *before* any scheduler activity exists to
/// overtake it.
///
/// Dropping a permit without activating it rolls the launch back. Nothing has
/// been spawned, no event has been emitted, and no scheduler call has been
/// recorded, so a rollback is the absence of an effect rather than the undoing
/// of one.
pub struct RunPermit {
    activate: Box<dyn FnOnce() + Send>,
}

impl std::fmt::Debug for RunPermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RunPermit")
    }
}

impl RunPermit {
    /// Build a permit whose activation runs `activate`.
    ///
    /// `activate` must be infallible: a launch step that can still fail belongs
    /// in preparation, before the command's decision state was committed.
    pub fn new(activate: impl FnOnce() + Send + 'static) -> Self {
        Self {
            activate: Box::new(activate),
        }
    }

    /// Release the launch. Infallible by construction.
    pub fn activate(self) {
        (self.activate)();
    }
}

/// The run-scheduling runtime the service drives.
///
/// Every method reports what really happened. `prepare_run` returning `Ok` means
/// a launch is guaranteed to be possible; a runtime refusal is an `Err`, never a
/// silent success.
#[async_trait]
pub trait RunSchedulerPort: Send + Sync {
    /// True when a scheduler task is currently alive.
    fn is_running(&self) -> bool;

    /// Validate and reserve a scheduler launch over `targets` without starting it.
    ///
    /// An empty `targets` list is a scheduler-owned run that consumes
    /// reducer-owned intent (a manual resolve, for example).
    ///
    /// No event may be emitted and no run may become observable until the
    /// returned permit is activated.
    async fn prepare_run(
        &self,
        targets: Vec<String>,
        explicit_retry: bool,
    ) -> Result<RunPermit, String>;

    /// Prepare and immediately activate a launch.
    ///
    /// The unordered shorthand, for a caller with no decision state to commit
    /// between the two halves. Production never uses it: the whole point of the
    /// split is that an accepted command's outcome is published in the gap.
    #[cfg_attr(not(test), allow(dead_code))]
    async fn start_run(&self, targets: Vec<String>, explicit_retry: bool) -> Result<(), String> {
        self.prepare_run(targets, explicit_retry)
            .await
            .map(RunPermit::activate)
    }

    /// Wake a scheduler that is already alive.
    async fn notify_scheduler(&self);

    /// Request cancellation of the live run.
    async fn cancel_run(&self);

    /// Set or clear the graceful-stop request.
    fn set_graceful_stop(&self, requested: bool);

    /// Take one runtime activity snapshot for an immediate stop decision.
    async fn stop_activity(&self) -> StopActivitySnapshot;
}

/// Every scheduler port is the liveness authority for its own boundary.
///
/// The active Apply-limit gate is retired by scheduler-task exit and by nothing
/// else, so the handle that decides whether a dispatch notifies or starts is the
/// same handle that decides whether a retry is admitted. Deriving the trait here
/// rather than binding a second flag is what makes those two decisions incapable
/// of disagreeing.
impl<T: RunSchedulerPort + ?Sized> RunBoundaryLiveness for T {
    fn boundary_running(&self) -> bool {
        RunSchedulerPort::is_running(self)
    }
}

// ============================================================================
// Service
// ============================================================================

/// A scheduler dispatch that has been reserved but not yet issued.
///
/// Holding one is the promise that activation cannot fail; issuing it is what
/// establishes the causal order between an accepted command's outcome and the
/// scheduler activity that command enabled.
#[derive(Debug)]
pub enum PreparedDispatch {
    /// A new scheduler run is reserved and waiting for its permit.
    Start(RunPermit),
    /// A scheduler that is already alive will be woken.
    Wake,
    /// Nothing will be dispatched.
    None,
}

impl PreparedDispatch {
    /// The scheduler effect this dispatch will report once issued.
    pub fn effect(&self) -> SchedulerEffect {
        match self {
            Self::Start(_) => SchedulerEffect::Started,
            Self::Wake => SchedulerEffect::Notified,
            Self::None => SchedulerEffect::None,
        }
    }

    /// Issue the prepared dispatch. Infallible by construction.
    pub async fn activate(self, scheduler: &dyn RunSchedulerPort) {
        match self {
            Self::Start(permit) => permit.activate(),
            Self::Wake => scheduler.notify_scheduler().await,
            Self::None => {}
        }
    }
}

/// How one admitted ordinary Start target reaches queue intent.
///
/// Both spellings produce the same ordinary launch — `explicit_retry` stays
/// false and the run carries no retry semantics — so this is not a third class
/// of Start. It is the single reducer command each admitted target needs, and
/// it is decided at classification time from that target's own evidence rather
/// than re-derived at commit, so the transaction commits exactly the transition
/// admission approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrdinaryAdmission {
    /// An idle `not queued` row enters through the ordinary queue addition.
    Queue,
    /// A preserved operator-`Stopped` row is resumed.
    ///
    /// The same transaction clears that stop's terminal classification and the
    /// dequeue residue holding the row out of ordinary admission first, which
    /// `AddToQueue` cannot do: it treats every terminal row as a no-op.
    ResumeStopped,
}

/// One ordinary Start target and the transition that admits it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OrdinaryTarget {
    /// Marked change this Start admits.
    change_id: String,
    /// The reducer transition classification chose for it.
    admission: OrdinaryAdmission,
}

impl OrdinaryTarget {
    /// The reducer command this target's admission commits.
    fn command(&self) -> ReducerCommand {
        match self.admission {
            OrdinaryAdmission::Queue => ReducerCommand::AddToQueue(self.change_id.clone()),
            OrdinaryAdmission::ResumeStopped => {
                ReducerCommand::ResumeStopped(self.change_id.clone())
            }
        }
    }
}

/// The class of work one Start request admits, chosen from marked evidence.
///
/// Deliberately not a mixed variant. `explicit_retry` is a run-level launch
/// property, so a launch carrying both classes would apply retry-specific
/// startup behaviour to ordinary work and hide which targets the operator
/// actually asked to retry.
#[derive(Debug)]
enum StartAdmission {
    /// Ordinary `not queued` marks enter the run.
    Ordinary {
        /// Admitted targets, in request order.
        targets: Vec<OrdinaryTarget>,
        /// Marked targets the classification left out.
        excluded: Vec<ExcludedTarget>,
    },
    /// Marked retry-eligible routes are consumed through explicit retry.
    Retry {
        /// Routes the retry classification accepted.
        routes: Vec<(String, RetryRoute)>,
        /// Marked targets the classification left out.
        excluded: Vec<ExcludedTarget>,
    },
}

/// The refusal detail for a Start whose marks contained nothing it could route.
///
/// Worded per mode because the operator's next action differs: in `Select` and
/// `Stopped` either class could have run, while a live run and process-wide
/// `Error` only ever admit retry routes.
fn exhausted_start_detail(
    mode: OperatorMode,
    marked: usize,
    excluded: &[ExcludedTarget],
) -> String {
    let cause = match mode {
        OperatorMode::Select => format!(
            "no marked change is startable ({marked} marked, none with status \
             '{NOT_QUEUED_STATUS}' and none carrying retryable evidence)"
        ),
        // Stopped names the resume route too, because it is the one mode in
        // which a preserved `stopped` mark *would* have been startable: an
        // operator who sees this refusal has to be able to tell that the route
        // exists and that their marks did not qualify for it.
        OperatorMode::Stopped => format!(
            "no marked change is startable ({marked} marked, none with status \
             '{NOT_QUEUED_STATUS}', none resumable from an operator '{STOPPED_STATUS}' \
             outcome, and none carrying retryable evidence)"
        ),
        OperatorMode::Running => format!(
            "a live run admits only marked retry-eligible changes \
             ({marked} marked, none carries retryable evidence)"
        ),
        _ => format!("no marked change carries retryable evidence ({marked} marked)"),
    };
    format!("{cause}; excluded: {}", describe_exclusions(excluded))
}

/// What a prepared run-lifecycle command will commit once its gate allows it.
#[derive(Debug)]
enum PreparedIntent {
    /// Start or resume the run over exactly these targets.
    Start {
        /// Admitted targets, in request order.
        targets: Vec<OrdinaryTarget>,
        /// Marked targets the classification left out, carried so the committed
        /// outcome can report them alongside the admitted subset.
        excluded: Vec<ExcludedTarget>,
    },
    /// Consume these already-classified retry routes.
    Retry {
        /// Routes the retry classification accepted.
        routes: Vec<(String, RetryRoute)>,
        /// Marked targets the classification left out.
        excluded: Vec<ExcludedTarget>,
    },
    /// Reserve the single resolver for this change.
    Resolve { change_id: String },
}

/// A validated run-lifecycle command holding every fallible runtime capability
/// it needs, with nothing committed yet.
///
/// Preparation reads state; it never writes it. A preparation failure therefore
/// leaves no reducer, mark, queue, retry-edge, resolve-reservation, mode, hook,
/// scheduler, or event effect to undo — the absence of an effect, rather than a
/// rollback that has to be trusted.
#[derive(Debug)]
pub struct PreparedRunCommand {
    intent: PreparedIntent,
    dispatch: PreparedDispatch,
}

/// A committed run-lifecycle command whose scheduler dispatch is still pending.
///
/// The caller must dispatch the accepted outcome first and only then call
/// [`CommittedRunCommand::activate`], which is the ordering the whole
/// transaction exists to establish.
#[derive(Debug)]
#[must_use = "a committed command holds a scheduler dispatch that must be activated"]
pub struct CommittedRunCommand {
    /// The typed outcome to dispatch.
    pub outcome: RunControlOutcome,
    dispatch: PreparedDispatch,
}

impl CommittedRunCommand {
    /// Issue the prepared scheduler dispatch.
    pub async fn activate(self, scheduler: &dyn RunSchedulerPort) {
        self.dispatch.activate(scheduler).await;
    }
}

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

    /// The scheduler port this service drives.
    ///
    /// The coordinator needs it to activate a prepared dispatch *after* the
    /// accepted outcome has been published, which is deliberately outside this
    /// service's own critical section.
    pub fn scheduler(&self) -> Arc<dyn RunSchedulerPort> {
        self.scheduler.clone()
    }

    /// Reserve a scheduler dispatch without issuing it.
    ///
    /// This is the only fallible step of a run-dispatching command, and it runs
    /// before anything is mutated.
    async fn prepare_dispatch(
        &self,
        command: RunCommandKind,
        targets: Vec<String>,
        explicit_retry: bool,
    ) -> RunControlResult<PreparedDispatch> {
        if self.scheduler.is_running() {
            return Ok(PreparedDispatch::Wake);
        }
        self.scheduler
            .prepare_run(targets, explicit_retry)
            .await
            .map(PreparedDispatch::Start)
            .map_err(|message| RunControlError::DispatchFailed { command, message })
    }

    /// Dispatch work to the scheduler and report what actually happened.
    #[cfg_attr(not(test), allow(dead_code))]
    async fn dispatch(
        &self,
        command: RunCommandKind,
        targets: Vec<String>,
        explicit_retry: bool,
    ) -> RunControlResult<SchedulerEffect> {
        let prepared = self
            .prepare_dispatch(command, targets, explicit_retry)
            .await?;
        let effect = prepared.effect();
        prepared.activate(self.scheduler.as_ref()).await;
        Ok(effect)
    }

    // ------------------------------------------------------------------
    // Start
    // ------------------------------------------------------------------

    /// Start, resume, or retry the run for the authoritative marked target set.
    ///
    /// The unordered shorthand over [`Self::prepare_start`], [`Self::commit`],
    /// and activation. It routes through exactly those halves rather than
    /// repeating their admission rules, so the composed form and the
    /// coordinator-driven form cannot disagree about which class a mark set
    /// selects.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn start(&self, mode: OperatorMode) -> RunControlResult<RunControlOutcome> {
        let prepared = self.prepare_start(mode).await?;
        let committed = self.commit(prepared).await?;
        let outcome = committed.outcome.clone();
        committed.activate(self.scheduler.as_ref()).await;
        Ok(outcome)
    }

    /// Marked change IDs that are eligible to enter a new run.
    ///
    /// This is the authoritative target set: it reads the shared execution-mark
    /// store and the reducer's display status, never a frontend's row cache.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn start_targets(&self) -> Vec<String> {
        self.classify_start_targets(false)
            .await
            .0
            .into_iter()
            .map(|target| target.change_id)
            .collect()
    }

    /// Split the coherent mark snapshot into startable rows and exclusions.
    ///
    /// Marks carry no eligibility of their own — an operator may mark any
    /// non-terminal row at any time — so the whole run-target decision is made
    /// here, from current reducer facts, and every excluded target is named with
    /// the status that excluded it.
    ///
    /// `resume_stopped` widens the startable set by exactly one row shape: a
    /// preserved mark whose only terminal evidence is an operator stop. The
    /// caller passes it only for an explicit Start in process mode `Stopped`,
    /// which is the one request that means "run these again". Reading the
    /// reducer's own [`OrchestratorState::is_resumable_stopped`] rather than the
    /// `stopped` display string is what keeps a row carrying a wait, a hold, or
    /// re-established queue intent underneath the stop out of the resumed set.
    async fn classify_start_targets(
        &self,
        resume_stopped: bool,
    ) -> (Vec<OrdinaryTarget>, Vec<ExcludedTarget>) {
        let marked = self.operator.marks().marked_ids();
        let guard = self.state.read().await;
        let mut startable = Vec::new();
        let mut excluded = Vec::new();
        for id in marked {
            let status = guard.display_status(&id);
            let admission = if status == NOT_QUEUED_STATUS {
                Some(OrdinaryAdmission::Queue)
            } else if resume_stopped
                && status == STOPPED_STATUS
                && guard.is_resumable_stopped(&id)
            {
                Some(OrdinaryAdmission::ResumeStopped)
            } else {
                None
            };
            match admission {
                Some(admission) => startable.push(OrdinaryTarget {
                    change_id: id,
                    admission,
                }),
                None => {
                    let status = status.to_string();
                    excluded.push(ExcludedTarget::new(id, status));
                }
            }
        }
        (startable, excluded)
    }

    // ------------------------------------------------------------------
    // Retry
    // ------------------------------------------------------------------

    /// Retry one change and prove the scheduler picked the work up.
    ///
    /// Admission happens inside the per-change service and therefore *before*
    /// this method can reach [`Self::dispatch_retry`]: a target whose exhausted
    /// Apply ceiling is still owned by a live boundary returns the typed refusal
    /// here, so no reducer transition, no explicit-retry edge, and no scheduler
    /// notify or start ever happens for it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn retry_change(&self, change_id: &str) -> RunControlResult<RunControlOutcome> {
        let plan = self.operator.retry_change(change_id).await?;
        self.dispatch_retry(plan, Vec::new()).await
    }

    /// Retry every change in `change_ids` that carries retryable evidence.
    ///
    /// Changes without retryable evidence are skipped, so a bulk retry never
    /// consumes an unsupported hold or destroys its blocker evidence. A target
    /// whose active run owns its exhausted Apply ceiling is skipped the same way,
    /// which keeps the request partial rather than global: when every candidate
    /// is skipped the plan is empty and the scheduler is neither woken nor
    /// started.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn retry_errors(&self, change_ids: &[String]) -> RunControlResult<RunControlOutcome> {
        let plan = self.operator.retry_errors(change_ids).await;
        self.dispatch_retry(plan, Vec::new()).await
    }

    #[cfg_attr(not(test), allow(dead_code))]
    async fn dispatch_retry(
        &self,
        plan: RetryPlan,
        excluded: Vec<ExcludedTarget>,
    ) -> RunControlResult<RunControlOutcome> {
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
            excluded,
        })
    }

    // ------------------------------------------------------------------
    // Stop family
    // ------------------------------------------------------------------

    /// Request a graceful stop after the current change completes.
    ///
    /// `Select` is admitted only when the scheduler is really alive. That is the
    /// persistent-idle case: the run parked with nothing to execute and the
    /// frontend presents Ready, but the task still exists and still owes the
    /// operator a stop. Scheduler liveness — never the frontend's idle-episode
    /// fact — is the authority, so a stale client cannot stop a run that ended.
    pub async fn stop(&self, mode: OperatorMode) -> RunControlResult<RunControlOutcome> {
        if mode != OperatorMode::Running && !self.is_persistent_idle_ready(mode) {
            return Err(RunControlError::InvalidMode {
                command: RunCommandKind::Stop,
                mode,
            });
        }
        self.scheduler.set_graceful_stop(true);
        // The request is recorded first, then the wait is woken: a scheduler
        // parked in its event-driven idle wait has no timer to notice the flag
        // on its own, so without this it would sit there until an unrelated
        // queue, merge, or cancellation event happened to arrive.
        self.scheduler.notify_scheduler().await;
        Ok(RunControlOutcome::StopRequested)
    }

    /// Whether `Select` here is persistent-idle Ready over a live scheduler.
    ///
    /// This is the one place the stop family widens beyond its existing modes,
    /// and it widens on scheduler liveness — never on a presentation fact a
    /// frontend sent along — so a stale client cannot stop a run that ended, and
    /// pre-run Select stays refused exactly as before.
    fn is_persistent_idle_ready(&self, mode: OperatorMode) -> bool {
        mode == OperatorMode::Select && self.scheduler.is_running()
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
        if !matches!(mode, OperatorMode::Running | OperatorMode::Stopping)
            && !self.is_persistent_idle_ready(mode)
        {
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
    #[cfg_attr(not(test), allow(dead_code))]
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

    // ------------------------------------------------------------------
    // Prepare / commit
    // ------------------------------------------------------------------
    //
    // The composed methods above stay the unordered shorthand. These are the
    // halves the application coordinator drives, so that a command's accepted
    // decision state and its outcome event are published before any scheduler
    // activity the command enabled can exist.

    /// Validate a Start and reserve its dispatch.
    ///
    /// Read-only: a refusal here has changed nothing.
    ///
    /// Which class the request selects comes from
    /// [`Self::classify_start_admission`], so ordinary and retry Start share one
    /// mark read, one worktree fence, and one set of refusals.
    pub async fn prepare_start(&self, mode: OperatorMode) -> RunControlResult<PreparedRunCommand> {
        match self.classify_start_admission(mode).await? {
            StartAdmission::Ordinary { targets, excluded } => {
                let change_ids: Vec<String> = targets
                    .iter()
                    .map(|target| target.change_id.clone())
                    .collect();
                let dispatch = self
                    .prepare_dispatch(RunCommandKind::Start, change_ids, false)
                    .await?;
                Ok(PreparedRunCommand {
                    intent: PreparedIntent::Start { targets, excluded },
                    dispatch,
                })
            }
            StartAdmission::Retry { routes, excluded } => {
                self.prepare_routes(RunCommandKind::Start, routes, excluded)
                    .await
            }
        }
    }

    /// Choose the class of work one Start request admits, without mutating.
    ///
    /// Process mode is a lifecycle guard here, not a proxy for retry
    /// eligibility. `ProcessingError` is change-scoped, so a retryable failure
    /// leaves the process in `Running` or persistent-idle `Select`; deriving the
    /// class from marked target evidence instead is what keeps the configured
    /// Start control reachable for it.
    ///
    /// The order below is the contract: mode guard, then the complete-request
    /// worktree fence over the *whole* marked set, then class selection. A fence
    /// applied after class selection would let a worktree-ineligible mark decide
    /// which class ran before refusing the request.
    async fn classify_start_admission(
        &self,
        mode: OperatorMode,
    ) -> RunControlResult<StartAdmission> {
        // A run that is already stopping owns its own termination; admitting
        // work into it would race the boundary it is walking to.
        if mode == OperatorMode::Stopping {
            return Err(RunControlError::InvalidMode {
                command: RunCommandKind::Start,
                mode,
            });
        }

        let marked = self.operator.marks().marked_ids();
        if marked.is_empty() {
            return Err(RunControlError::NoEligibleTarget {
                command: RunCommandKind::Start,
                detail: "no change carries an execution mark".to_string(),
            });
        }

        // The fence is applied to the *complete* marked set before anything is
        // narrowed down, so start is all-or-nothing: one ineligible target
        // refuses the whole operation instead of quietly starting the eligible
        // remainder, which is a target set the operator never asked for.
        let rejected = self.eligibility.rejected(&marked);
        if !rejected.is_empty() {
            return Err(RunControlError::NoEligibleTarget {
                command: RunCommandKind::Start,
                detail: format!(
                    "worktree execution requires committed changes with no uncommitted \
                     files; ineligible marked targets: {}",
                    rejected.join(", ")
                ),
            });
        }

        // Status classification happens here, at final admission, from current
        // reducer facts — never from an eligibility result recorded at mark
        // time. Excluded rows are named but do not block the runnable subset.
        //
        // Resuming a preserved stop is scoped to `Stopped` alone, because that
        // is the mode in which an explicit Start means "run these again". In
        // every other mode a `stopped` row keeps its terminal evidence and is
        // reported as an exclusion exactly as before.
        let admits_ordinary = matches!(mode, OperatorMode::Select | OperatorMode::Stopped);
        let resume_stopped = mode == OperatorMode::Stopped;
        let (ordinary, ordinary_excluded) = if admits_ordinary {
            self.classify_start_targets(resume_stopped).await
        } else {
            (Vec::new(), Vec::new())
        };

        if !ordinary.is_empty() {
            // Ordinary Start keeps its priority, and the retry-eligible rows it
            // left behind are reported as deferred rather than implicitly
            // retried: `explicit_retry` is a run-level launch property, so
            // mixing the two classes would apply retry startup semantics to work
            // that never asked for them.
            let excluded = self.defer_retry_only(ordinary_excluded).await;
            return Ok(StartAdmission::Ordinary {
                targets: ordinary,
                excluded,
            });
        }

        // Only marked retry-eligible rows are routed; every other marked row is
        // named as excluded rather than silently dropped, and a request with no
        // runnable target is rejected before any effect.
        let routes = self.operator.plan_retry_errors(&marked).await;
        let excluded = self.describe_non_retryable(&marked, &routes, mode).await;
        if routes.is_empty() {
            return Err(RunControlError::NoEligibleTarget {
                command: RunCommandKind::Start,
                detail: exhausted_start_detail(mode, marked.len(), &excluded),
            });
        }
        Ok(StartAdmission::Retry { routes, excluded })
    }

    /// Explain the retry-eligible rows ordinary Start admission left behind.
    ///
    /// Read-only classification over the excluded rows only: a row that the very
    /// next request would retry must not be reported as though its status made
    /// it permanently unrunnable.
    async fn defer_retry_only(&self, excluded: Vec<ExcludedTarget>) -> Vec<ExcludedTarget> {
        let ids: Vec<String> = excluded
            .iter()
            .map(|target| target.change_id.clone())
            .collect();
        if ids.is_empty() {
            return excluded;
        }
        let routes = self.operator.plan_retry_errors(&ids).await;
        excluded
            .into_iter()
            .map(|target| {
                if routes.iter().any(|(id, _)| *id == target.change_id) {
                    target.with_detail(RETRY_DEFERRED_TO_ORDINARY_START)
                } else {
                    target
                }
            })
            .collect()
    }

    /// Name every marked target the retry classification did not accept.
    async fn describe_non_retryable(
        &self,
        marked: &[String],
        routes: &[(String, RetryRoute)],
        mode: OperatorMode,
    ) -> Vec<ExcludedTarget> {
        let guard = self.state.read().await;
        marked
            .iter()
            .filter(|id| !routes.iter().any(|(routed, _)| routed == *id))
            .map(|id| {
                let status = guard.display_status(id).to_string();
                // An ordinary mark under a live run is not unrunnable; it is
                // owned by the mark-settlement path instead, and saying so is
                // what keeps a Running-mode exclusion actionable.
                let deferred = mode == OperatorMode::Running && status == NOT_QUEUED_STATUS;
                let target = ExcludedTarget::new(id.clone(), status);
                if deferred {
                    target.with_detail(ORDINARY_DEFERRED_TO_MARK_SETTLEMENT)
                } else {
                    target
                }
            })
            .collect()
    }

    /// Validate a single-change retry and reserve its dispatch.
    pub async fn prepare_retry_change(
        &self,
        change_id: &str,
    ) -> RunControlResult<PreparedRunCommand> {
        let routes = match self.operator.plan_retry_change(change_id).await? {
            Some(route) => vec![(change_id.to_string(), route)],
            None => Vec::new(),
        };
        self.prepare_routes(RunCommandKind::Retry, routes, Vec::new())
            .await
    }

    /// Validate a bulk retry and reserve its dispatch.
    pub async fn prepare_retry_errors(
        &self,
        change_ids: &[String],
    ) -> RunControlResult<PreparedRunCommand> {
        let routes = self.operator.plan_retry_errors(change_ids).await;
        self.prepare_routes(RunCommandKind::Retry, routes, Vec::new())
            .await
    }

    /// Reserve the dispatch for already-classified retry routes.
    ///
    /// `command` is the operator-facing command the routes came from, so a
    /// runtime launch refusal for a Start-selected retry reads as a refused
    /// start rather than as a retry the operator never typed.
    async fn prepare_routes(
        &self,
        command: RunCommandKind,
        routes: Vec<(String, RetryRoute)>,
        excluded: Vec<ExcludedTarget>,
    ) -> RunControlResult<PreparedRunCommand> {
        if routes.is_empty() {
            // Nothing retryable: no dispatch is reserved, so nothing has to be
            // rolled back when the caller settles this as a no-op.
            return Ok(PreparedRunCommand {
                intent: PreparedIntent::Retry { routes, excluded },
                dispatch: PreparedDispatch::None,
            });
        }
        let targets: Vec<String> = routes.iter().map(|(id, _)| id.clone()).collect();
        let dispatch = self.prepare_dispatch(command, targets, true).await?;
        Ok(PreparedRunCommand {
            intent: PreparedIntent::Retry { routes, excluded },
            dispatch,
        })
    }

    /// Validate a resolve reservation and reserve its dispatch.
    ///
    /// Whether the reservation will be active or queued is decided from the
    /// ledger before anything is written, so only a launch that will really be
    /// consumed is reserved.
    pub async fn prepare_resolve(
        &self,
        change_id: &str,
    ) -> RunControlResult<Option<PreparedRunCommand>> {
        if self.resolves.is_reserved(change_id) {
            return Ok(None);
        }

        let display_status = self.display_status(change_id).await;
        if display_status != MERGE_WAIT_STATUS {
            return Err(RunControlError::TargetIneligible {
                command: RunCommandKind::Resolve,
                change_id: change_id.to_string(),
                display_status,
            });
        }

        // Only the active resolver dispatches. A queued reservation is consumed
        // when the active resolve finishes, so reserving a launch for it now
        // would claim work that cannot start.
        let dispatch = if self.resolves.is_active() {
            PreparedDispatch::None
        } else {
            self.prepare_dispatch(RunCommandKind::Resolve, Vec::new(), false)
                .await?
        };

        Ok(Some(PreparedRunCommand {
            intent: PreparedIntent::Resolve {
                change_id: change_id.to_string(),
            },
            dispatch,
        }))
    }

    /// Commit a prepared command's decision state without issuing its dispatch.
    ///
    /// The returned value still owns the reserved dispatch; the caller publishes
    /// the accepted outcome first and only then activates it.
    pub async fn commit(
        &self,
        prepared: PreparedRunCommand,
    ) -> RunControlResult<CommittedRunCommand> {
        let PreparedRunCommand { intent, dispatch } = prepared;
        let scheduler = dispatch.effect();

        match intent {
            PreparedIntent::Start { targets, excluded } => {
                {
                    // One write guard for the whole admitted set: a resume that
                    // clears stop residue and the queue additions beside it are
                    // one indivisible transition, so no observer — and no mark
                    // settlement pass — can see a row half-resumed.
                    let mut guard = self.state.write().await;
                    for target in &targets {
                        guard.apply_command(target.command());
                    }
                }
                Ok(CommittedRunCommand {
                    outcome: RunControlOutcome::RunDispatched {
                        change_ids: targets
                            .into_iter()
                            .map(|target| target.change_id)
                            .collect(),
                        explicit_retry: false,
                        scheduler,
                        excluded,
                    },
                    dispatch,
                })
            }
            PreparedIntent::Retry { routes, excluded } => {
                let plan = self.operator.commit_retry_routes(&routes).await;
                if plan.is_empty() {
                    // Every classified route was refused by the reducer. The
                    // reserved dispatch is dropped rather than issued, so no
                    // scheduler effect survives a no-op.
                    return Ok(CommittedRunCommand {
                        outcome: RunControlOutcome::NoOp {
                            reason: RunNoOpReason::NoRetryableTarget,
                        },
                        dispatch: PreparedDispatch::None,
                    });
                }
                Ok(CommittedRunCommand {
                    outcome: RunControlOutcome::RunDispatched {
                        change_ids: plan.change_ids,
                        explicit_retry: plan.explicit_retry,
                        scheduler,
                        excluded,
                    },
                    dispatch,
                })
            }
            PreparedIntent::Resolve { change_id } => {
                // Reducer first: it owns whether the wait state really accepts a
                // resolve intent, so a stale target is refused before any
                // reservation exists. Dropping `dispatch` on this path is the
                // rollback — nothing was ever spawned.
                let reduce_outcome = {
                    let mut guard = self.state.write().await;
                    guard.apply_command(ReducerCommand::ResolveMerge(change_id.clone()))
                };
                if matches!(reduce_outcome, ReduceOutcome::NoOp) {
                    return Err(RunControlError::TargetIneligible {
                        command: RunCommandKind::Resolve,
                        change_id: change_id.clone(),
                        display_status: self.display_status(&change_id).await,
                    });
                }
                let Some(reservation) = self.resolves.reserve(&change_id) else {
                    // Another caller reserved between preparation and here.
                    return Ok(CommittedRunCommand {
                        outcome: RunControlOutcome::NoOp {
                            reason: RunNoOpReason::ResolveAlreadyReserved { change_id },
                        },
                        dispatch: PreparedDispatch::None,
                    });
                };
                let (dispatch, scheduler) = match reservation {
                    ResolveReservation::Active => (dispatch, scheduler),
                    ResolveReservation::Queued { .. } => {
                        (PreparedDispatch::None, SchedulerEffect::None)
                    }
                };
                Ok(CommittedRunCommand {
                    outcome: RunControlOutcome::ResolveReserved {
                        change_id,
                        reservation,
                        scheduler,
                    },
                    dispatch,
                })
            }
        }
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

    /// Observer a test installs to witness the moment a permit is activated.
    ///
    /// It receives the launch's change IDs and explicit-retry flag, and runs on
    /// the activating task, so a hook can assert what is true *at* activation —
    /// for example that the application gate is still held.
    type ActivationHook = Arc<dyn Fn(Vec<String>, bool) + Send + Sync>;

    /// Everything an activated launch writes, shared so a permit can own a clone.
    ///
    /// A permit outlives the `&self` borrow that produced it, so the recorder
    /// state a launch touches has to be reachable from an owned handle.
    #[derive(Default)]
    struct SchedulerRecorder {
        calls: Mutex<Vec<SchedulerCall>>,
        running: std::sync::atomic::AtomicBool,
        /// The graceful-stop request this scheduler currently holds.
        ///
        /// Modelled as the shared flag the real supervisor owns, not just as a
        /// recorded call, so coverage can hand the *same* handle to a real
        /// scheduler and prove it settles the stop an accepted command really
        /// recorded.
        graceful_stop: Arc<std::sync::atomic::AtomicBool>,
        /// Events an activated launch publishes, in activation order.
        ///
        /// This is how a test proves scheduler progress cannot precede the
        /// accepted command outcome: the launch emits nothing until its permit
        /// is activated, and the permit is activated only after the outcome has
        /// already been dispatched.
        on_activate: Mutex<Option<ActivationHook>>,
    }

    impl std::fmt::Debug for SchedulerRecorder {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("SchedulerRecorder")
                .field("calls", &self.calls)
                .field("running", &self.running)
                .finish_non_exhaustive()
        }
    }

    impl SchedulerRecorder {
        fn record(&self, call: SchedulerCall) {
            self.calls.lock().unwrap().push(call);
        }
    }

    /// A [`RunSchedulerPort`] that records calls instead of spawning work.
    ///
    /// No process, task, or repository is involved, so tests over it stay
    /// unit-scoped while still proving the side effect really happened.
    #[derive(Debug)]
    pub(crate) struct RecordingScheduler {
        recorder: Arc<SchedulerRecorder>,
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
                recorder: Arc::new(SchedulerRecorder::default()),
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
            self.recorder
                .running
                .store(running, std::sync::atomic::Ordering::SeqCst);
        }

        /// Replace the runtime activity snapshot force stop will read.
        pub(crate) fn set_activity(&self, activity: StopActivitySnapshot) {
            *self.activity.lock().unwrap() = activity;
        }

        /// Make the next launch preparation fail with `message`.
        pub(crate) fn fail_launch(&self, message: &str) {
            *self.launch_failure.lock().unwrap() = Some(message.to_string());
        }

        /// Run `hook` when a prepared launch is actually activated.
        ///
        /// The hook stands in for the first progress a real scheduler would
        /// publish, which is what makes activation ordering observable.
        #[cfg_attr(not(test), allow(dead_code))]
        pub(crate) fn on_activate(&self, hook: ActivationHook) {
            *self.recorder.on_activate.lock().unwrap() = Some(hook);
        }

        /// The graceful-stop request handle this scheduler writes through.
        ///
        /// Shared, so a scheduler that has to honour the request reads exactly
        /// what the accepted stop command recorded.
        pub(crate) fn graceful_stop_flag(&self) -> Arc<std::sync::atomic::AtomicBool> {
            self.recorder.graceful_stop.clone()
        }

        /// Every recorded call, in order.
        pub(crate) fn calls(&self) -> Vec<SchedulerCall> {
            self.recorder.calls.lock().unwrap().clone()
        }

        /// Targets of every activated launch, in order.
        pub(crate) fn started_targets(&self) -> Vec<Vec<String>> {
            self.calls()
                .into_iter()
                .filter_map(|call| match call {
                    SchedulerCall::Started { targets, .. } => Some(targets),
                    _ => None,
                })
                .collect()
        }
    }

    #[async_trait]
    impl RunSchedulerPort for RecordingScheduler {
        fn is_running(&self) -> bool {
            self.recorder
                .running
                .load(std::sync::atomic::Ordering::SeqCst)
        }

        /// Reserve a launch, consuming any armed failure.
        ///
        /// Nothing is recorded here: a prepared-but-never-activated launch must
        /// be indistinguishable from a launch that was never requested.
        async fn prepare_run(
            &self,
            targets: Vec<String>,
            explicit_retry: bool,
        ) -> std::result::Result<RunPermit, String> {
            if let Some(message) = self.launch_failure.lock().unwrap().take() {
                return Err(message);
            }
            let recorder = self.recorder.clone();
            Ok(RunPermit::new(move || {
                recorder.record(SchedulerCall::Started {
                    targets: targets.clone(),
                    explicit_retry,
                });
                recorder
                    .running
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                let hook = recorder.on_activate.lock().unwrap().clone();
                if let Some(hook) = hook {
                    hook(targets, explicit_retry);
                }
            }))
        }

        async fn notify_scheduler(&self) {
            self.recorder.record(SchedulerCall::Notified);
        }

        async fn cancel_run(&self) {
            self.recorder.record(SchedulerCall::Cancelled);
        }

        fn set_graceful_stop(&self, requested: bool) {
            self.recorder.record(SchedulerCall::GracefulStop(requested));
            self.recorder
                .graceful_stop
                .store(requested, std::sync::atomic::Ordering::SeqCst);
        }

        async fn stop_activity(&self) -> StopActivitySnapshot {
            *self.activity.lock().unwrap()
        }
    }
}

#[cfg(test)]
mod tests;
