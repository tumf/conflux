//! The one process-local operator application transaction.
//!
//! [`crate::orchestration::operator_command::OperatorCommandService`] and
//! [`crate::orchestration::run_control::RunControlService`] own *what* an
//! operator command does. This module owns *when* — the ordering that makes an
//! accepted command truthful to every frontend at once:
//!
//! ```text
//! acquire the application gate
//!   -> validate mode, target, and eligibility against Core state
//!   -> reserve every fallible runtime capability (nothing mutated yet)
//!   -> commit the staged decision state
//!   -> dispatch one authoritative outcome and capture its exact revision
//!   -> activate the infallible scheduler permit, or issue its wake
//! release the gate
//! ```
//!
//! Two properties follow from that shape and are the reason it exists.
//!
//! *Preparation precedes mutation*, so a runtime that refuses to launch leaves
//! no reducer transition, no mark, no queue intent, no explicit-retry edge, no
//! resolve reservation, no mode change, and no event behind. A failure is the
//! absence of an effect rather than a rollback anyone has to trust.
//!
//! *Activation follows dispatch*, so scheduler progress enabled by a command can
//! never emit an event before the command's own accepted effect. A frontend
//! observing the run therefore cannot see `ProcessingStarted` before it sees the
//! Start that caused it.
//!
//! Activation is nevertheless *inside* the gate, after the outcome dispatch and
//! revision capture, not after the release. Activation is infallible, so it
//! cannot fail the transaction; releasing first would only open a window in
//! which a force stop is admitted against a run whose permit has not spawned
//! yet, so its `cancel_run` finds no handle, no-ops, and the run spawns anyway
//! immediately afterwards. Holding the gate across the activation makes the
//! admitted-to-spawned interval indivisible to every other command.
//!
//! A command that must await confirmed runtime termination is the explicit
//! two-phase exception: it holds the gate only for admission and cancellation
//! issuance, waits outside it, then reacquires the gate and revalidates the
//! target's *current runtime state* — not the revision it was admitted at, which
//! unrelated commands may legitimately have advanced.
//!
//! Everything here is process-local and dropped at exit. Under
//! `openspec/CONSTITUTION.md` the next workflow action after a restart is
//! recomputed from workspace and Git evidence alone; a mode, a gate, or a
//! reservation held in memory is never durable workflow authority.

use std::sync::{Arc, Mutex};

use crate::events::{
    all_completed_may_overwrite_mode, is_admitted_work_start, persistent_idle_may_project_ready,
    EventDispatcher, ExecutionEvent, OperatorCommandEffect, OutcomeRevisions,
};
use crate::orchestration::apply_commit_evidence::ApplyCommitEvidencePort;
use crate::orchestration::operator_command::{OperatorMode, OperatorOutcome, PendingTermination};
use crate::orchestration::run_control::{
    ResolveReservation, RunControlError, RunControlOutcome, RunControlService, RunNoOpReason,
    SchedulerEffect,
};

// ============================================================================
// Core process lifecycle mode
// ============================================================================

/// The one ephemeral process lifecycle mode every frontend projects.
///
/// This is *not* reducer change lifecycle and *not* durable workflow state. It
/// answers exactly one question — which operator commands this process will
/// admit right now — and TUI `execution_mode` and Web `app_mode` are projections
/// of it rather than independent authorities.
///
/// It is transitioned from two sources and no others: an accepted command
/// outcome, and an authoritative lifecycle event delivered through the
/// process-lifetime dispatch boundary. The second source is what keeps the
/// process admissible: without it, natural completion would leave Core wedged in
/// `Running` and a later Start would be refused forever.
#[derive(Debug)]
pub struct CoreMode {
    state: Mutex<CoreModeState>,
}

/// Admission mode plus the one qualifier that changes what `Select` means.
#[derive(Debug, Clone, Copy)]
struct CoreModeState {
    mode: OperatorMode,
    /// True while `Select` describes a live persistent-scheduler idle episode
    /// rather than ordinary pre-run selection.
    ///
    /// Held next to the mode, under the same lock, because the two are read and
    /// written as one decision: a cancel-stop that saw a stale value of either
    /// would restore the wrong mode.
    persistent_idle: bool,
}

impl Default for CoreMode {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreMode {
    /// A fresh process, in `Select`.
    ///
    /// Fresh on every construction on purpose: a restart begins with no
    /// admission history, and the next action is recomputed from the workspace.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CoreModeState {
                mode: OperatorMode::Select,
                persistent_idle: false,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CoreModeState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The current admission mode.
    pub fn get(&self) -> OperatorMode {
        self.lock().mode
    }

    /// Replace the admission mode, reporting whether it changed.
    ///
    /// The idle-episode fact qualifies `Select` and nothing else, so it travels
    /// with the mode it qualifies — the same rule the `/api/v2` snapshot applies
    /// when a caller publishes a different execution mode.
    ///
    /// Production moves the mode only through [`CoreMode::apply_event`]; this is
    /// how a test arranges the process it is about to command.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set(&self, mode: OperatorMode) -> bool {
        let mut guard = self.lock();
        let changed = guard.mode != mode;
        guard.mode = mode;
        guard.persistent_idle &= mode == OperatorMode::Select;
        changed
    }

    /// Arrange the idle-episode half of the mode directly.
    ///
    /// Production only ever moves this through [`CoreMode::apply_event`]; a test
    /// that arranged a frontend in persistent-idle Ready without arranging Core
    /// would be exercising a split state the process cannot be in.
    #[cfg(test)]
    pub fn set_persistent_idle(&self, persistent_idle: bool) {
        self.lock().persistent_idle = persistent_idle;
    }

    /// Apply an authoritative lifecycle or command-outcome event.
    ///
    /// Returns the new mode when the event moved it. The guards are the same
    /// ones every frontend already applies, routed through one implementation so
    /// Core and the frontends cannot disagree about the same run:
    /// `AllCompleted` may not overwrite a retained terminal mode, and a
    /// command-outcome effect that carries no mode meaning (a mark delta, a
    /// queued resolve) moves nothing.
    ///
    /// The persistent-idle episode is tracked here for the same reason: `Select`
    /// over a live parked scheduler and `Select` before any run are different
    /// admission states, and a frontend that projects this mode must not have to
    /// re-derive which one it is holding.
    pub fn apply_event(&self, event: &ExecutionEvent) -> Option<OperatorMode> {
        let mut guard = self.lock();

        // One shared idle-episode rule, applied before the mode decision so an
        // arm below already reads the settled episode fact. It is the same rule
        // both frontends apply, over the same authoritative events, so the three
        // copies cannot drift apart.
        if is_admitted_work_start(event)
            || matches!(
                event,
                ExecutionEvent::Stopped | ExecutionEvent::Error { .. }
            )
        {
            guard.persistent_idle = false;
        }

        let next = match event {
            // Typed run activation. The scheduler really started work, so the
            // process is Running whether or not a command put it there.
            ExecutionEvent::ProcessingStarted(_) => OperatorMode::Running,
            ExecutionEvent::Stopping => OperatorMode::Stopping,
            ExecutionEvent::Stopped => OperatorMode::Stopped,
            ExecutionEvent::Error { .. } => OperatorMode::Error,
            // A change-scoped failure. The reducer records terminal Error on
            // that one change and mark reconciliation revokes its stale
            // execution intent; the *process* is not what failed, so admission
            // mode is left exactly where it was. Promoting it here is what made
            // one exhausted acceptance command disable ordinary mark controls
            // for every unrelated change still running.
            ExecutionEvent::ProcessingError { .. } => return None,
            // A persistent scheduler parked with nothing left to execute. Ready
            // here describes a *live* scheduler, so the shared guard applies:
            // only a running process becomes Ready, and pre-run Select, a
            // pending stop, and both terminal modes are left exactly as they
            // are.
            ExecutionEvent::PersistentSchedulerIdle => {
                if !persistent_idle_may_project_ready(guard.mode.as_app_mode()) {
                    return None;
                }
                guard.persistent_idle = true;
                OperatorMode::Select
            }
            ExecutionEvent::AllCompleted => {
                // Natural completion returns to Select and readmits a later
                // Start; a late completion after a stop or a fatal error does
                // not overwrite that terminal mode.
                if !all_completed_may_overwrite_mode(guard.mode.as_app_mode()) {
                    return None;
                }
                OperatorMode::Select
            }
            ExecutionEvent::OperatorCommandApplied { effect } => match effect {
                // Only a newly spawned scheduler proves execution has begun. A
                // dispatch that merely woke a scheduler which was already alive
                // has admitted nothing yet, so the mode waits for the first
                // typed work-start event rather than claiming Running here.
                OperatorCommandEffect::RunDispatched {
                    scheduler_started: true,
                    ..
                } => OperatorMode::Running,
                OperatorCommandEffect::RunDispatched { .. } => return None,
                // Withdrawing a stop restores where the stop came from. A stop
                // requested from persistent-idle Ready returns to Ready with its
                // episode intact; claiming Running would advertise execution no
                // typed event ever proved.
                OperatorCommandEffect::StopCancelled if guard.persistent_idle => {
                    OperatorMode::Select
                }
                OperatorCommandEffect::StopCancelled => OperatorMode::Running,
                OperatorCommandEffect::ForceStopAwaitingBoundary { .. } => OperatorMode::Stopping,
                OperatorCommandEffect::ResolveReserved { active: true, .. } => {
                    OperatorMode::Running
                }
                // A queued reservation starts nothing, and a mark or queue delta
                // is orthogonal to the run lifecycle.
                OperatorCommandEffect::ResolveReserved { active: false, .. }
                | OperatorCommandEffect::MarkDelta { .. }
                | OperatorCommandEffect::QueueDelta { .. } => return None,
            },
            // Work-start evidence other than `ProcessingStarted` resumes a
            // parked Ready and nothing else: a graceful stop that arrived first
            // is still owed, and a terminal mode stays terminal.
            _ if is_admitted_work_start(event) && guard.mode == OperatorMode::Select => {
                OperatorMode::Running
            }
            _ => return None,
        };
        let changed = guard.mode != next;
        guard.mode = next;
        changed.then_some(next)
    }
}

/// The typed scope of the two failure events, at the one place that decides it.
///
/// Unit-scoped: [`CoreMode`] alone, over constructed events. No reducer, no
/// frontend, no process, no repository.
#[cfg(test)]
mod core_mode_scope_tests {
    use super::*;

    fn processing_error() -> ExecutionEvent {
        ExecutionEvent::ProcessingError {
            id: "alpha".to_string(),
            error: "acceptance command attempts exhausted".to_string(),
        }
    }

    /// A change-scoped failure preserves whatever process mode it arrived in.
    ///
    /// Every mode is exercised, not just Running: the incident was Running, but
    /// a mapping that promoted the event from one mode and not another would be
    /// a second, quieter version of the same bug.
    #[test]
    fn processing_error_preserves_shared_mode() {
        for mode in [
            OperatorMode::Select,
            OperatorMode::Running,
            OperatorMode::Stopping,
            OperatorMode::Stopped,
            OperatorMode::Error,
        ] {
            let core = CoreMode::new();
            core.set(mode);

            assert_eq!(
                core.apply_event(&processing_error()),
                None,
                "a change-scoped failure moves no process mode (from {mode:?})"
            );
            assert_eq!(
                core.get(),
                mode,
                "the process mode must be exactly the one that existed before the event"
            );
        }
    }

    /// The idle-episode qualifier is a process fact too, so it survives as well.
    ///
    /// Observed through the command it actually qualifies: a stop withdrawn from
    /// persistent-idle Ready returns to Ready, and would claim Running instead if
    /// a change-local failure had closed the episode.
    #[test]
    fn processing_error_preserves_shared_mode_persistent_idle_episode() {
        let core = CoreMode::new();
        core.set(OperatorMode::Select);
        core.set_persistent_idle(true);

        assert_eq!(core.apply_event(&processing_error()), None);

        assert_eq!(
            core.apply_event(&ExecutionEvent::OperatorCommandApplied {
                effect: OperatorCommandEffect::StopCancelled,
            }),
            None,
            "the withdrawn stop returns to the Ready the episode still describes"
        );
        assert_eq!(core.get(), OperatorMode::Select);
    }

    /// The fatal control: suppressing every Error transition would also pass the
    /// test above, so the one event that really means the run died is asserted
    /// from the same starting modes.
    #[test]
    fn processing_error_preserves_shared_mode_fatal_control_still_transitions() {
        for mode in [
            OperatorMode::Select,
            OperatorMode::Running,
            OperatorMode::Stopping,
            OperatorMode::Stopped,
        ] {
            let core = CoreMode::new();
            core.set(mode);

            assert_eq!(
                core.apply_event(&ExecutionEvent::Error {
                    message: "the orchestrator could not start".to_string(),
                }),
                Some(OperatorMode::Error),
                "a global Error is process-fatal (from {mode:?})"
            );
            assert_eq!(core.get(), OperatorMode::Error);
        }
    }
}

// ============================================================================
// Intents and results
// ============================================================================

/// One operator intent, in the vocabulary both adapters submit.
///
/// The TUI and `/api/v2` translate their own request shapes into this and
/// nothing else, which is what makes "equivalent intent takes the same path" a
/// property of the type system rather than of two hand-kept implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorIntent {
    /// Start, resume, or (in `Error` mode) retry the marked set.
    Start,
    /// Request a graceful stop.
    Stop,
    /// Withdraw a pending graceful stop.
    CancelStop,
    /// Stop immediately.
    ForceStop,
    /// Retry one change.
    RetryChange {
        /// Target change.
        change_id: String,
    },
    /// Retry every change in the list that carries retryable evidence.
    RetryErrors {
        /// Candidate changes.
        change_ids: Vec<String>,
    },
    /// Reserve a merge resolution.
    ResolveMerge {
        /// Target change.
        change_id: String,
    },
    /// Set one change's execution mark.
    SetExecutionMark {
        /// Target change.
        change_id: String,
        /// Requested value.
        marked: bool,
    },
    /// Set one change's queue intent.
    SetQueueIntent {
        /// Target change.
        change_id: String,
        /// Requested membership.
        queued: bool,
    },
    /// Derive and apply one execution-mark value across every eligible change.
    SetAllExecutionMarks,
    /// Stop a change and dequeue it after confirmed termination.
    ///
    /// The two-phase intent: [`OperatorApplication::apply`] runs both phases and
    /// releases the gate between them.
    StopAndDequeue {
        /// Target change.
        change_id: String,
    },
}

/// What an accepted command actually did, in the shared service vocabulary.
///
/// Reusing the two existing outcome enums rather than inventing a third is
/// deliberate: every adapter already projects them, and a third spelling would
/// be one more place the two frontends could drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationOutcome {
    /// A run-lifecycle command settled.
    Run(RunControlOutcome),
    /// A per-change operator command settled.
    Operator(OperatorOutcome),
}

/// One completed application transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationResult {
    /// The typed outcome, or the typed refusal.
    pub outcome: Result<ApplicationOutcome, RunControlError>,
    /// The revision this command's own outcome dispatch produced.
    ///
    /// `None` when the command dispatched no outcome — an ordinary no-op or an
    /// ordinary failure — which is what a caller settles as "the unchanged
    /// admitted revision". A two-phase command that refuses *after* the wait
    /// returns `Some(unchanged revision observed at settlement)` instead,
    /// because by then the admitted revision is no longer the right answer.
    pub revision: Option<u64>,
}

impl ApplicationResult {
    fn failed(error: RunControlError) -> Self {
        Self {
            outcome: Err(error),
            revision: None,
        }
    }

    fn run(outcome: RunControlOutcome, revision: Option<u64>) -> Self {
        Self {
            outcome: Ok(ApplicationOutcome::Run(outcome)),
            revision,
        }
    }

    fn operator(outcome: OperatorOutcome, revision: Option<u64>) -> Self {
        Self {
            outcome: Ok(ApplicationOutcome::Operator(outcome)),
            revision,
        }
    }
}

/// Exclusive ownership of the application gate, held across one transaction.
pub type ApplicationGuard = tokio::sync::OwnedMutexGuard<()>;

// ============================================================================
// The coordinator
// ============================================================================

/// The single process-local operator application coordinator.
///
/// It lives outside the `web-monitoring` feature on purpose: a TUI-only build
/// runs the same transaction with the same ordering and simply has no revision
/// to report. Binding Web adds a sink and a revision source; it never creates
/// the authority.
pub struct OperatorApplication {
    gate: Arc<tokio::sync::Mutex<()>>,
    mode: Arc<CoreMode>,
    run_control: Arc<RunControlService>,
    dispatch: Arc<EventDispatcher>,
    /// Where an outcome dispatch's exact revision is read back from.
    ///
    /// `None` for a process with no `/api/v2` projection, which is also a
    /// process with no command record to bind a revision to.
    revisions: Option<Arc<dyn OutcomeRevisions>>,
    /// Where explanatory Apply-commit evidence is read from at stop settlement.
    ///
    /// `None` for a process with no repository observation port, which reports
    /// unknown rather than guessing.
    apply_commit_evidence: Option<Arc<dyn ApplyCommitEvidencePort>>,
}

impl OperatorApplication {
    /// Build the coordinator over the shared services and the process-lifetime
    /// dispatch owner.
    pub fn new(
        mode: Arc<CoreMode>,
        run_control: Arc<RunControlService>,
        dispatch: Arc<EventDispatcher>,
    ) -> Self {
        Self {
            gate: Arc::new(tokio::sync::Mutex::new(())),
            mode,
            run_control,
            dispatch,
            revisions: None,
            apply_commit_evidence: None,
        }
    }

    /// Bind the projection an outcome dispatch's revision is read back from.
    pub fn with_revisions(mut self, revisions: Option<Arc<dyn OutcomeRevisions>>) -> Self {
        self.revisions = revisions;
        self
    }

    /// Bind the repository port stop settlement reads Apply-commit evidence from.
    pub fn with_apply_commit_evidence(
        mut self,
        port: Option<Arc<dyn ApplyCommitEvidencePort>>,
    ) -> Self {
        self.apply_commit_evidence = port;
        self
    }

    /// The process-local serialization gate.
    ///
    /// A frontend that must serialize its *own* admission against command
    /// execution — `/api/v2`, whose optimistic revision check would otherwise let
    /// two new commands consume one revision — holds this across its whole
    /// submission and calls [`Self::apply_held`].
    pub fn gate(&self) -> Arc<tokio::sync::Mutex<()>> {
        self.gate.clone()
    }

    /// The shared run-lifecycle service.
    pub fn run_control(&self) -> Arc<RunControlService> {
        self.run_control.clone()
    }

    /// Apply one intent, acquiring the gate.
    pub async fn apply(&self, intent: OperatorIntent) -> ApplicationResult {
        if let OperatorIntent::StopAndDequeue { change_id } = &intent {
            return self.apply_stop_and_dequeue(change_id, None).await;
        }
        let guard = self.gate.clone().lock_owned().await;
        self.apply_ordinary(intent, &guard).await
    }

    /// Apply one intent under a gate the caller already holds.
    ///
    /// A two-phase intent takes ownership of the guard: it must release it
    /// before waiting for confirmed termination and reacquire it to commit.
    pub async fn apply_held(
        &self,
        intent: OperatorIntent,
        guard: ApplicationGuard,
    ) -> ApplicationResult {
        if let OperatorIntent::StopAndDequeue { change_id } = &intent {
            let change_id = change_id.clone();
            return self.apply_stop_and_dequeue(&change_id, Some(guard)).await;
        }
        self.apply_ordinary(intent, &guard).await
    }

    /// Begin a two-phase stop-and-dequeue under a held gate.
    ///
    /// Phase one only: validation and cancellation issuance. The caller releases
    /// the gate and later drives [`Self::settle_stop_and_dequeue`], which is what
    /// keeps force stop, unrelated commands, event fan-out, and rendering live
    /// while confirmation is pending.
    pub async fn begin_stop_and_dequeue(
        &self,
        change_id: &str,
    ) -> Result<PendingTermination, RunControlError> {
        self.run_control
            .operator()
            .begin_stop_and_dequeue(change_id)
            .await
            .map_err(RunControlError::Operator)
    }

    /// Settle a two-phase stop-and-dequeue after its cancellation was issued.
    ///
    /// Reacquires the gate, revalidates the target's current runtime state, and
    /// commits — or settles with the explicit unchanged revision observed under
    /// the reacquired boundary, without publishing a dequeue event.
    ///
    /// Between confirmation and reacquisition the terminated worktree is
    /// quiescent, which is the one window where explanatory Git evidence can be
    /// read safely *and* cheaply: doing it here rather than under the boundary
    /// keeps a Git subprocess from monopolizing operator admission, and doing it
    /// after termination rather than before keeps it from racing a worker that
    /// is still committing.
    pub async fn settle_stop_and_dequeue(&self, pending: PendingTermination) -> ApplicationResult {
        let change_id = pending.change_id().to_string();
        if let Err(error) = pending.confirm_termination().await {
            // Cancellation was already issued and is not rollbackable, but no
            // dequeue decision state may be committed. The settlement revision
            // is read explicitly rather than assumed to be the admitted one:
            // unrelated commands may have advanced it during the wait. It is
            // read under the *reacquired* gate for the same reason the
            // post-wait commit path holds it — a revision sampled outside the
            // boundary is a sample of mutable global state, which is exactly
            // what a settled record may not contain.
            let _guard = self.gate.clone().lock_owned().await;
            return ApplicationResult {
                outcome: Err(RunControlError::Operator(error)),
                revision: Some(self.current_revision()),
            };
        }

        // Outside the boundary, over a quiescent worktree.
        let apply_commit = crate::orchestration::apply_commit_evidence::observe_apply_commit(
            self.run_control.operator().execution_facts().as_deref(),
            self.apply_commit_evidence.as_deref(),
            &change_id,
        )
        .await;

        let _guard = self.gate.clone().lock_owned().await;
        match self
            .run_control
            .operator()
            .commit_stop_and_dequeue(&change_id, apply_commit)
            .await
        {
            Ok(OperatorOutcome::Dequeued {
                change_id,
                settlement,
            }) => {
                // `ChangeDequeued` already means exactly this; a second spelling
                // would give the same fact two authorities.
                let revision = self
                    .dispatch_outcome(ExecutionEvent::ChangeDequeued {
                        change_id: change_id.clone(),
                    })
                    .await;
                ApplicationResult::operator(
                    OperatorOutcome::Dequeued {
                        change_id,
                        settlement,
                    },
                    Some(revision),
                )
            }
            Ok(outcome) => ApplicationResult::operator(outcome, Some(self.current_revision())),
            Err(error) => ApplicationResult {
                outcome: Err(RunControlError::Operator(error)),
                revision: Some(self.current_revision()),
            },
        }
    }

    async fn apply_stop_and_dequeue(
        &self,
        change_id: &str,
        held: Option<ApplicationGuard>,
    ) -> ApplicationResult {
        let guard = match held {
            Some(guard) => guard,
            None => self.gate.clone().lock_owned().await,
        };
        let pending = match self.begin_stop_and_dequeue(change_id).await {
            Ok(pending) => pending,
            Err(error) => return ApplicationResult::failed(error),
        };
        // The gate is released *before* the wait, never during it.
        drop(guard);
        self.settle_stop_and_dequeue(pending).await
    }

    // ------------------------------------------------------------------
    // Ordinary transaction
    // ------------------------------------------------------------------

    async fn apply_ordinary(
        &self,
        intent: OperatorIntent,
        _guard: &ApplicationGuard,
    ) -> ApplicationResult {
        let mode = self.mode.get();
        match intent {
            OperatorIntent::Start => {
                self.apply_prepared(self.run_control.prepare_start(mode).await)
                    .await
            }
            OperatorIntent::RetryChange { change_id } => {
                self.apply_prepared(self.run_control.prepare_retry_change(&change_id).await)
                    .await
            }
            OperatorIntent::RetryErrors { change_ids } => {
                self.apply_prepared(self.run_control.prepare_retry_errors(&change_ids).await)
                    .await
            }
            OperatorIntent::ResolveMerge { change_id } => self.apply_resolve(&change_id).await,
            OperatorIntent::Stop => self.apply_stop(mode).await,
            OperatorIntent::CancelStop => self.apply_cancel_stop(mode).await,
            OperatorIntent::ForceStop => self.apply_force_stop(mode).await,
            OperatorIntent::SetExecutionMark { change_id, marked } => {
                self.apply_mark(&change_id, marked).await
            }
            OperatorIntent::SetQueueIntent { change_id, queued } => {
                self.apply_queue_intent(&change_id, queued).await
            }
            OperatorIntent::SetAllExecutionMarks => self.apply_bulk_marks().await,
            // Routed before the gate was taken.
            OperatorIntent::StopAndDequeue { .. } => {
                unreachable!("a two-phase intent is routed through apply_stop_and_dequeue")
            }
        }
    }

    /// Commit a prepared run-lifecycle command, publish its outcome, then
    /// activate its scheduler dispatch.
    async fn apply_prepared(
        &self,
        prepared: Result<crate::orchestration::run_control::PreparedRunCommand, RunControlError>,
    ) -> ApplicationResult {
        let prepared = match prepared {
            Ok(prepared) => prepared,
            // Preparation never mutated anything, so this refusal has nothing to
            // undo and nothing to publish.
            Err(error) => return ApplicationResult::failed(error),
        };

        let committed = match self.run_control.commit(prepared).await {
            Ok(committed) => committed,
            Err(error) => return ApplicationResult::failed(error),
        };

        let outcome = committed.outcome.clone();
        let Some(event) = run_outcome_event(&outcome) else {
            // A no-op publishes nothing and advances no revision.
            committed
                .activate(self.run_control.scheduler().as_ref())
                .await;
            return ApplicationResult::run(outcome, None);
        };

        let revision = self.dispatch_outcome(event).await;
        // Only now may scheduler activity enabled by this command exist.
        committed
            .activate(self.run_control.scheduler().as_ref())
            .await;
        ApplicationResult::run(outcome, Some(revision))
    }

    async fn apply_resolve(&self, change_id: &str) -> ApplicationResult {
        match self.run_control.prepare_resolve(change_id).await {
            Ok(Some(prepared)) => self.apply_prepared(Ok(prepared)).await,
            Ok(None) => ApplicationResult::run(
                RunControlOutcome::NoOp {
                    reason: RunNoOpReason::ResolveAlreadyReserved {
                        change_id: change_id.to_string(),
                    },
                },
                None,
            ),
            Err(error) => ApplicationResult::failed(error),
        }
    }

    async fn apply_stop(&self, mode: OperatorMode) -> ApplicationResult {
        match self.run_control.stop(mode).await {
            // `Stopping` already means exactly this, so no new vocabulary is
            // introduced for it.
            Ok(outcome @ RunControlOutcome::StopRequested) => {
                let revision = self.dispatch_outcome(ExecutionEvent::Stopping).await;
                ApplicationResult::run(outcome, Some(revision))
            }
            Ok(other) => ApplicationResult::run(other, None),
            Err(error) => ApplicationResult::failed(error),
        }
    }

    async fn apply_cancel_stop(&self, mode: OperatorMode) -> ApplicationResult {
        match self.run_control.cancel_stop(mode).await {
            Ok(outcome @ RunControlOutcome::StopCancelled) => {
                let revision = self
                    .dispatch_outcome(ExecutionEvent::OperatorCommandApplied {
                        effect: OperatorCommandEffect::StopCancelled,
                    })
                    .await;
                ApplicationResult::run(outcome, Some(revision))
            }
            Ok(other) => ApplicationResult::run(other, None),
            Err(error) => ApplicationResult::failed(error),
        }
    }

    async fn apply_force_stop(&self, mode: OperatorMode) -> ApplicationResult {
        let outcome = match self.run_control.force_stop(mode).await {
            Ok(outcome) => outcome,
            Err(error) => return ApplicationResult::failed(error),
        };
        let RunControlOutcome::ForceStopped {
            classification,
            awaiting_safe_boundary,
        } = &outcome
        else {
            return ApplicationResult::run(outcome, None);
        };

        let event = if *awaiting_safe_boundary {
            // Terminal state must not be published before the required cleanup:
            // the scheduler owns the eventual `Stopped`.
            ExecutionEvent::OperatorCommandApplied {
                effect: OperatorCommandEffect::ForceStopAwaitingBoundary {
                    force_stop: classification.process_report.is_force_stop(),
                },
            }
        } else {
            // No live scheduler will emit this run's terminal stop, so this
            // command is its dispatch owner and `Stopped` is exact.
            ExecutionEvent::Stopped
        };
        let revision = self.dispatch_outcome(event).await;
        ApplicationResult::run(outcome, Some(revision))
    }

    async fn apply_mark(&self, change_id: &str, marked: bool) -> ApplicationResult {
        match self
            .run_control
            .operator()
            .set_execution_mark(change_id, marked)
            .await
        {
            Ok(outcome) => self.publish_operator_outcome(outcome).await,
            Err(error) => ApplicationResult::failed(RunControlError::Operator(error)),
        }
    }

    async fn apply_queue_intent(&self, change_id: &str, queued: bool) -> ApplicationResult {
        let service = self.run_control.operator();
        let result = if queued {
            service.add_to_queue(change_id).await
        } else {
            service.remove_from_queue(change_id).await
        };
        match result {
            Ok(queue) => {
                self.publish_operator_outcome(OperatorOutcome::Queue(queue))
                    .await
            }
            Err(error) => ApplicationResult::failed(RunControlError::Operator(error)),
        }
    }

    async fn apply_bulk_marks(&self) -> ApplicationResult {
        match self.run_control.operator().set_all_execution_marks().await {
            Ok(outcome) => self.publish_operator_outcome(outcome).await,
            Err(error) => ApplicationResult::failed(RunControlError::Operator(error)),
        }
    }

    /// Publish the delta an accepted per-change command produced.
    async fn publish_operator_outcome(&self, outcome: OperatorOutcome) -> ApplicationResult {
        let Some(event) = operator_outcome_event(&outcome) else {
            return ApplicationResult::operator(outcome, None);
        };
        let revision = self.dispatch_outcome(event).await;
        ApplicationResult::operator(outcome, Some(revision))
    }

    // ------------------------------------------------------------------
    // Dispatch
    // ------------------------------------------------------------------

    /// Dispatch one accepted outcome and report the exact revision it produced.
    ///
    /// The revision is looked up by *dispatch identity*, not sampled from global
    /// state afterwards: sampling would let unrelated later progress become the
    /// command's recorded `result_revision`.
    async fn dispatch_outcome(&self, event: ExecutionEvent) -> u64 {
        let dispatch_id = self.dispatch.dispatch(event).await;
        match &self.revisions {
            Some(revisions) => revisions
                .revision_for_dispatch(dispatch_id)
                .unwrap_or_else(|| revisions.current_revision()),
            None => 0,
        }
    }

    fn current_revision(&self) -> u64 {
        self.revisions
            .as_ref()
            .map_or(0, |revisions| revisions.current_revision())
    }
}

/// The authoritative event one accepted run-lifecycle outcome publishes.
///
/// `None` for a no-op, which publishes nothing and advances no revision.
fn run_outcome_event(outcome: &RunControlOutcome) -> Option<ExecutionEvent> {
    match outcome {
        RunControlOutcome::RunDispatched {
            change_ids,
            explicit_retry,
            scheduler,
            // Exclusions are operator-facing reporting on the command result,
            // not run state: the authoritative event carries only what was
            // actually dispatched.
            excluded: _,
        } => Some(ExecutionEvent::OperatorCommandApplied {
            effect: OperatorCommandEffect::RunDispatched {
                change_ids: change_ids.clone(),
                explicit_retry: *explicit_retry,
                scheduler_started: matches!(scheduler, SchedulerEffect::Started),
            },
        }),
        RunControlOutcome::ResolveReserved {
            change_id,
            reservation,
            ..
        } => Some(ExecutionEvent::OperatorCommandApplied {
            effect: OperatorCommandEffect::ResolveReserved {
                change_id: change_id.clone(),
                active: matches!(reservation, ResolveReservation::Active),
            },
        }),
        // Published by their own dedicated paths, which choose between the exact
        // existing event and the awaiting-boundary variant.
        RunControlOutcome::StopRequested
        | RunControlOutcome::StopCancelled
        | RunControlOutcome::ForceStopped { .. } => None,
        RunControlOutcome::NoOp { .. } => None,
    }
}

/// The authoritative event one accepted per-change outcome publishes.
fn operator_outcome_event(outcome: &OperatorOutcome) -> Option<ExecutionEvent> {
    match outcome {
        OperatorOutcome::MarkSet { change_id, marked } => {
            Some(ExecutionEvent::OperatorCommandApplied {
                effect: OperatorCommandEffect::MarkDelta {
                    change_ids: vec![change_id.clone()],
                    marked: *marked,
                },
            })
        }
        OperatorOutcome::BulkMarks {
            marked, changed, ..
        } => (!changed.is_empty()).then(|| ExecutionEvent::OperatorCommandApplied {
            effect: OperatorCommandEffect::MarkDelta {
                change_ids: changed.clone(),
                marked: *marked,
            },
        }),
        OperatorOutcome::Queue(queue) => (queue.reducer_changed || queue.dynamic_queue_mutated)
            .then(|| ExecutionEvent::OperatorCommandApplied {
                effect: OperatorCommandEffect::QueueDelta {
                    change_id: queue.change_id.clone(),
                    queued: matches!(
                        queue.mutation,
                        crate::orchestration::operator_command::QueueMutation::Added
                    ),
                },
            }),
        OperatorOutcome::Retry(plan) => {
            (!plan.is_empty()).then(|| ExecutionEvent::OperatorCommandApplied {
                effect: OperatorCommandEffect::RunDispatched {
                    change_ids: plan.change_ids.clone(),
                    explicit_retry: plan.explicit_retry,
                    // A plan is the reducer transition, not the dispatch that
                    // makes it real: it carries no scheduler evidence at all.
                    // Reporting "not started" is the only truthful reading, and
                    // the run-control path — which does hold the scheduler
                    // effect — is what publishes a started retry.
                    scheduler_started: false,
                },
            })
        }
        // `ChangeDequeued` already means exactly this and is published by the
        // two-phase settlement path.
        OperatorOutcome::Dequeued { .. } => None,
        OperatorOutcome::NoOp { .. } => None,
    }
}

#[cfg(test)]
mod tests;

/// Phase-aware stop settlement, over deterministic termination ordering.
#[cfg(test)]
mod stop_settlement_tests;

/// The dispatch boundary with a real `/api/v2` projection attached.
#[cfg(all(test, feature = "web-monitoring"))]
mod mode_matrix_tests;
