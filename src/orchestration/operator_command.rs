//! Shared, frontend-independent operator command service.
//!
//! Frontends (TUI today, remote adapters later) map operator intent onto
//! [`OperatorCommand`] values and call this service. The service owns lifecycle
//! validation and coordinates authoritative reducer transitions with runtime
//! side effects (dynamic queue mutation, per-change cancellation, queue hooks,
//! retry routing) so no frontend has to duplicate that matrix.
//!
//! State axes stay separate on purpose:
//!
//! - execution mark: process-local operator intent ([`ExecutionMarkStore`])
//! - queue intent: reducer-owned membership in the dynamic pending set
//! - activity / wait / terminal: reducer-owned runtime facts
//! - `display_status()`: projection of the reducer-owned axes
//!
//! Execution marks are never written outside the process, so a restart starts
//! with every mark `false` and workflow routing keeps coming from workspace and
//! Git evidence.

// This module is a boundary, not an implementation detail: the TUI adapter uses
// part of it today and the remote frontend adapter will use the rest. Keeping the
// whole boundary defined (and tested) is deliberate, so unused-API warnings from
// the binary crate are allowed here.
#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::orchestration::state::{OrchestratorState, ReduceOutcome, ReducerCommand};

/// Default bound for waiting on confirmed task termination during stop-and-dequeue.
pub const DEFAULT_CANCELLATION_TIMEOUT: Duration = Duration::from_secs(30);

// ============================================================================
// Display-status vocabulary helpers
// ============================================================================

/// Display statuses that represent active execution.
const ACTIVE_STATUSES: [&str; 5] = [
    "applying",
    "accepting",
    "rejecting",
    "archiving",
    "resolving",
];

/// Display statuses that are final and cannot be mutated by operator intent.
const FINAL_STATUSES: [&str; 4] = ["archived", "merged", "pushed", "rejected"];

/// Display statuses that only accept mark-only mutation (base-lane waits).
const MARK_ONLY_WAIT_STATUSES: [&str; 2] = ["merge wait", "resolve pending"];

/// Returns true when the display status means Core is actively executing the change.
pub fn is_active_status(display_status: &str) -> bool {
    ACTIVE_STATUSES.contains(&display_status)
}

/// Returns true when the display status is a final outcome.
pub fn is_final_status(display_status: &str) -> bool {
    FINAL_STATUSES.contains(&display_status)
}

// ============================================================================
// Mode and routing
// ============================================================================

/// Frontend-neutral projection of the operator-facing application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorMode {
    /// Pre-run selection.
    Select,
    /// A run is active.
    Running,
    /// A graceful stop was requested but the run has not finished.
    Stopping,
    /// The run stopped and can be resumed.
    Stopped,
    /// The run ended in an error state and requires explicit retry.
    Error,
}

/// How an execution-mark request must be routed for a mode/status pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkRoute {
    /// Mutate the process-local execution mark only; no reducer or queue effect.
    MarkOnly,
    /// Translate the mark into dynamic queue intent (add/remove).
    QueueIntent,
    /// Reject: recovery in this mode is owned by retry commands.
    RetryRequired,
    /// Reject: the row cannot be mutated by mark or queue intent.
    Immutable,
}

/// Decide how an execution-mark request must be handled.
///
/// This is the single lifecycle matrix shared by every frontend. It mirrors the
/// TUI key semantics that existed before the service was introduced, so moving
/// a frontend onto the service cannot silently change operator behavior.
pub fn classify_mark_route(mode: OperatorMode, display_status: &str) -> MarkRoute {
    if is_final_status(display_status) {
        return MarkRoute::Immutable;
    }

    match mode {
        // Error mode never mutates marks: `retry_change` / `retry_errors` own recovery.
        OperatorMode::Error => MarkRoute::RetryRequired,
        // Select mode has no runtime queue yet, so marks are pure operator intent.
        OperatorMode::Select => MarkRoute::MarkOnly,
        // A pending graceful stop is a transition; intent changes wait for it.
        OperatorMode::Stopping => MarkRoute::Immutable,
        OperatorMode::Stopped => {
            if matches!(display_status, "not queued" | "error")
                || MARK_ONLY_WAIT_STATUSES.contains(&display_status)
            {
                MarkRoute::MarkOnly
            } else {
                MarkRoute::Immutable
            }
        }
        OperatorMode::Running => {
            if MARK_ONLY_WAIT_STATUSES.contains(&display_status) {
                return MarkRoute::MarkOnly;
            }
            if is_active_status(display_status) {
                // Active rows are stopped through `StopAndDequeue`, not through marks.
                return MarkRoute::Immutable;
            }
            match display_status {
                "not queued" | "queued" | "error" => MarkRoute::QueueIntent,
                _ => MarkRoute::Immutable,
            }
        }
    }
}

/// Where a retry request must be routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryRoute {
    /// A recoverable terminal error: use `ReducerCommand::RetryError`.
    TerminalError,
    /// A resumable acceptance hold: resume acceptance via the explicit-retry run path.
    AcceptanceStall,
}

/// Decide the retry route for a change from its display status.
///
/// Returns `None` when the status carries no retryable evidence.
pub fn classify_retry_route(display_status: &str) -> Option<RetryRoute> {
    match display_status {
        "error" => Some(RetryRoute::TerminalError),
        "stalled" => Some(RetryRoute::AcceptanceStall),
        _ => None,
    }
}

// ============================================================================
// Commands, outcomes, errors
// ============================================================================

/// Frontend-independent operator intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorCommand {
    /// Set the process-local execution mark for a change.
    SetExecutionMark {
        /// Target change.
        change_id: String,
        /// Requested mark value.
        marked: bool,
    },
    /// Add a change to the dynamic queue.
    AddToQueue {
        /// Target change.
        change_id: String,
    },
    /// Remove a change from the dynamic queue.
    RemoveFromQueue {
        /// Target change.
        change_id: String,
    },
    /// Stop an in-flight change and dequeue it once termination is confirmed.
    StopAndDequeue {
        /// Target change.
        change_id: String,
    },
    /// Retry a single change using its reconciled evidence.
    RetryChange {
        /// Target change.
        change_id: String,
    },
}

/// Which direction a dynamic queue mutation went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMutation {
    /// The change was added.
    Added,
    /// The change was removed.
    Removed,
}

/// Result of a queue-intent command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueOutcome {
    /// Target change.
    pub change_id: String,
    /// Requested direction.
    pub mutation: QueueMutation,
    /// True when the reducer accepted the intent change.
    pub reducer_changed: bool,
    /// True when the runtime dynamic queue really changed.
    ///
    /// Queue hooks run exactly once when, and only when, this is true.
    pub dynamic_queue_mutated: bool,
    /// Display status after the command.
    pub display_status: String,
}

/// Why a command produced no state change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoOpReason {
    /// The execution mark already had the requested value.
    MarkUnchanged,
    /// The reducer rejected the intent in the current lifecycle state.
    ReducerRejected,
}

/// What a successful command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorOutcome {
    /// The process-local execution mark changed.
    MarkSet {
        /// Target change.
        change_id: String,
        /// New mark value.
        marked: bool,
    },
    /// A queue-intent command completed.
    Queue(QueueOutcome),
    /// An active change was cancelled, confirmed terminated, and dequeued.
    Dequeued {
        /// Target change.
        change_id: String,
    },
    /// A retry was accepted and routed.
    Retry(RetryPlan),
    /// Nothing changed.
    NoOp {
        /// Target change (empty for bulk commands).
        change_id: String,
        /// Why nothing changed.
        reason: NoOpReason,
    },
}

/// Routing decision for a retry request.
///
/// The service decides routing and applies the reducer transition; starting or
/// waking a run stays with the caller so the existing scheduler ownership model
/// is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPlan {
    /// Changes that were accepted for retry, in request order.
    pub change_ids: Vec<String>,
    /// Route per accepted change, in the same order as `change_ids`.
    pub routes: Vec<RetryRoute>,
    /// True when the run must be started with explicit-retry semantics so a
    /// reconciled acceptance hold is consumed and acceptance resumes.
    pub explicit_retry: bool,
}

impl RetryPlan {
    /// True when no change was accepted for retry.
    pub fn is_empty(&self) -> bool {
        self.change_ids.is_empty()
    }
}

/// Why a command was rejected without side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorCommandError {
    /// Mark mutation is not allowed for this mode/status pair.
    MarkNotAllowed {
        /// Target change.
        change_id: String,
        /// Mode the request arrived in.
        mode: OperatorMode,
        /// Route that rejected the request.
        route: MarkRoute,
        /// Display status at rejection time.
        display_status: String,
    },
    /// The change is active but has no registered cancellation handle.
    MissingCancellationHandle {
        /// Target change.
        change_id: String,
    },
    /// Cancellation could not be issued.
    CancellationFailed {
        /// Target change.
        change_id: String,
        /// Failure detail from the runtime.
        message: String,
    },
    /// Termination was not confirmed within the bound.
    TerminationTimeout {
        /// Target change.
        change_id: String,
        /// How long the service waited.
        waited: Duration,
    },
    /// Retry is not supported for the change's current evidence.
    RetryUnsupported {
        /// Target change.
        change_id: String,
        /// Display status that carries no retryable evidence.
        display_status: String,
    },
}

impl std::fmt::Display for OperatorCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MarkNotAllowed {
                change_id,
                mode,
                route,
                display_status,
            } => write!(
                f,
                "execution mark for '{change_id}' is not allowed in {mode:?} mode \
                 (status '{display_status}', route {route:?})"
            ),
            Self::MissingCancellationHandle { change_id } => write!(
                f,
                "no cancellation handle registered for active change '{change_id}'"
            ),
            Self::CancellationFailed { change_id, message } => {
                write!(f, "cancellation failed for '{change_id}': {message}")
            }
            Self::TerminationTimeout { change_id, waited } => write!(
                f,
                "termination of '{change_id}' was not confirmed within {waited:?}"
            ),
            Self::RetryUnsupported {
                change_id,
                display_status,
            } => write!(
                f,
                "retry is not supported for '{change_id}' with status '{display_status}'"
            ),
        }
    }
}

/// Result alias for operator command execution.
pub type OperatorResult<T> = std::result::Result<T, OperatorCommandError>;

// ============================================================================
// Execution marks (process-local)
// ============================================================================

/// Process-local store of execution marks.
///
/// Marks express "the operator wants this change considered at the next
/// applicable boundary". They are never persisted, so a new process starts with
/// every mark `false` by construction and marks can never become durable
/// workflow-control evidence.
#[derive(Debug, Default)]
pub struct ExecutionMarkStore {
    marks: Mutex<HashSet<String>>,
}

impl ExecutionMarkStore {
    /// Create an empty store. A restarted process always begins here.
    pub fn new() -> Self {
        Self::default()
    }

    /// True when the change currently carries an execution mark.
    pub fn is_marked(&self, change_id: &str) -> bool {
        self.lock().contains(change_id)
    }

    /// Set the mark for a change. Returns true when the value changed.
    pub fn set(&self, change_id: &str, marked: bool) -> bool {
        let mut guard = self.lock();
        if marked {
            guard.insert(change_id.to_string())
        } else {
            guard.remove(change_id)
        }
    }

    /// Replace the whole mark set (used by frontends that own a mark projection).
    pub fn replace(&self, change_ids: impl IntoIterator<Item = String>) {
        *self.lock() = change_ids.into_iter().collect();
    }

    /// All marked change IDs, sorted for deterministic output.
    pub fn marked_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.lock().iter().cloned().collect();
        ids.sort();
        ids
    }

    /// Drop every mark.
    pub fn clear(&self) {
        self.lock().clear();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.marks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

// ============================================================================
// Ports
// ============================================================================

/// Waiter that completes once an executor confirms a change's task exited.
#[derive(Clone, Debug)]
pub struct TerminationWaiter {
    done: CancellationToken,
}

impl TerminationWaiter {
    /// Wrap the `done` token an executor cancels at task completion.
    pub fn new(done: CancellationToken) -> Self {
        Self { done }
    }

    /// A waiter that is already satisfied.
    pub fn already_terminated() -> Self {
        let done = CancellationToken::new();
        done.cancel();
        Self { done }
    }

    /// A waiter that never completes (used to exercise timeout handling).
    pub fn never() -> Self {
        Self {
            done: CancellationToken::new(),
        }
    }

    /// Wait until termination is confirmed.
    pub async fn wait(&self) {
        self.done.cancelled().await;
    }
}

/// Runtime queue operations the service coordinates.
#[async_trait]
pub trait QueuePort: Send + Sync {
    /// Add a change to the runtime queue. Returns true when the queue really changed.
    async fn add(&self, change_id: &str) -> bool;

    /// Remove a change from the runtime queue. Returns true when the queue really changed.
    async fn remove(&self, change_id: &str) -> bool;

    /// Issue cancellation for a change.
    ///
    /// `Ok(None)` means no cancellation handle is registered. `Err` means the
    /// cancellation request itself failed and no termination will follow.
    async fn request_cancellation(
        &self,
        change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String>;

    /// Wake the scheduler without changing queue contents.
    async fn notify_scheduler(&self);
}

/// Queue hook dispatch, isolated so the service can be verified without
/// executing user commands.
#[async_trait]
pub trait QueueHookPort: Send + Sync {
    /// Run `on_queue_add` for a completed dynamic addition.
    async fn on_queue_add(&self, change_id: &str);

    /// Run `on_queue_remove` for a completed dynamic removal.
    async fn on_queue_remove(&self, change_id: &str);
}

/// Hook port that runs nothing (CLI/headless callers without hook config).
pub struct NoopQueueHooks;

#[async_trait]
impl QueueHookPort for NoopQueueHooks {
    async fn on_queue_add(&self, _change_id: &str) {}
    async fn on_queue_remove(&self, _change_id: &str) {}
}

/// Hook port backed by the real configured [`crate::hooks::HookRunner`].
pub struct HookRunnerQueueHooks {
    runner: crate::hooks::HookRunner,
}

impl HookRunnerQueueHooks {
    /// Wrap a configured hook runner.
    pub fn new(runner: crate::hooks::HookRunner) -> Self {
        Self { runner }
    }

    async fn run(&self, hook_type: crate::hooks::HookType, change_id: &str) {
        let context = crate::hooks::HookContext::new(0, 0, 0, false).with_change(change_id, 0, 0);
        if let Err(error) = self.runner.run_hook(hook_type, &context).await {
            tracing::warn!("{hook_type} hook failed for '{change_id}': {error}");
        }
    }
}

#[async_trait]
impl QueueHookPort for HookRunnerQueueHooks {
    async fn on_queue_add(&self, change_id: &str) {
        self.run(crate::hooks::HookType::OnQueueAdd, change_id)
            .await;
    }

    async fn on_queue_remove(&self, change_id: &str) {
        self.run(crate::hooks::HookType::OnQueueRemove, change_id)
            .await;
    }
}

// ============================================================================
// Service
// ============================================================================

/// Process-local application service for operator commands.
pub struct OperatorCommandService {
    state: Arc<RwLock<OrchestratorState>>,
    queue: Arc<dyn QueuePort>,
    hooks: Arc<dyn QueueHookPort>,
    marks: Arc<ExecutionMarkStore>,
    cancellation_timeout: Duration,
}

impl OperatorCommandService {
    /// Build a service over the shared reducer state and runtime ports.
    pub fn new(
        state: Arc<RwLock<OrchestratorState>>,
        queue: Arc<dyn QueuePort>,
        hooks: Arc<dyn QueueHookPort>,
        marks: Arc<ExecutionMarkStore>,
    ) -> Self {
        Self {
            state,
            queue,
            hooks,
            marks,
            cancellation_timeout: DEFAULT_CANCELLATION_TIMEOUT,
        }
    }

    /// Override the bound used when waiting for confirmed task termination.
    pub fn with_cancellation_timeout(mut self, timeout: Duration) -> Self {
        self.cancellation_timeout = timeout;
        self
    }

    /// Shared process-local execution marks.
    pub fn marks(&self) -> Arc<ExecutionMarkStore> {
        self.marks.clone()
    }

    /// Current display status for a change.
    pub async fn display_status(&self, change_id: &str) -> String {
        self.state
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }

    /// Execute a typed operator command.
    pub async fn execute(
        &self,
        mode: OperatorMode,
        command: OperatorCommand,
    ) -> OperatorResult<OperatorOutcome> {
        match command {
            OperatorCommand::SetExecutionMark { change_id, marked } => {
                self.set_execution_mark(mode, &change_id, marked).await
            }
            OperatorCommand::AddToQueue { change_id } => self
                .add_to_queue(&change_id)
                .await
                .map(OperatorOutcome::Queue),
            OperatorCommand::RemoveFromQueue { change_id } => self
                .remove_from_queue(&change_id)
                .await
                .map(OperatorOutcome::Queue),
            OperatorCommand::StopAndDequeue { change_id } => {
                self.stop_and_dequeue(&change_id).await
            }
            OperatorCommand::RetryChange { change_id } => self
                .retry_change(&change_id)
                .await
                .map(OperatorOutcome::Retry),
        }
    }

    /// Apply an execution-mark request through the shared lifecycle matrix.
    pub async fn set_execution_mark(
        &self,
        mode: OperatorMode,
        change_id: &str,
        marked: bool,
    ) -> OperatorResult<OperatorOutcome> {
        let display_status = self.display_status(change_id).await;
        match classify_mark_route(mode, &display_status) {
            MarkRoute::MarkOnly => {
                if self.marks.set(change_id, marked) {
                    Ok(OperatorOutcome::MarkSet {
                        change_id: change_id.to_string(),
                        marked,
                    })
                } else {
                    Ok(OperatorOutcome::NoOp {
                        change_id: change_id.to_string(),
                        reason: NoOpReason::MarkUnchanged,
                    })
                }
            }
            MarkRoute::QueueIntent => {
                let outcome = if marked {
                    self.add_to_queue(change_id).await?
                } else {
                    self.remove_from_queue(change_id).await?
                };
                self.marks.set(change_id, marked);
                Ok(OperatorOutcome::Queue(outcome))
            }
            route @ (MarkRoute::RetryRequired | MarkRoute::Immutable) => {
                Err(OperatorCommandError::MarkNotAllowed {
                    change_id: change_id.to_string(),
                    mode,
                    route,
                    display_status,
                })
            }
        }
    }

    /// Add a change to the dynamic queue.
    ///
    /// A dependency-ineligible change keeps its queue intent: dependency
    /// blocking is reported later as `blocked` display status, never by
    /// rejecting the operator's request.
    pub async fn add_to_queue(&self, change_id: &str) -> OperatorResult<QueueOutcome> {
        let reduce_outcome = {
            let mut guard = self.state.write().await;
            if guard.is_terminal_error_change(change_id) {
                guard.apply_command(ReducerCommand::RetryError(change_id.to_string()))
            } else {
                guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()))
            }
        };
        let reducer_changed = matches!(reduce_outcome, ReduceOutcome::Changed(_));

        // Effect before commit: hooks describe real runtime mutations only.
        let dynamic_queue_mutated = if reducer_changed {
            self.queue.add(change_id).await
        } else {
            false
        };
        if dynamic_queue_mutated {
            // Wake the scheduler so newly queued work is reconsidered immediately,
            // then run the hook for the mutation that actually happened.
            self.queue.notify_scheduler().await;
            self.hooks.on_queue_add(change_id).await;
        }

        Ok(QueueOutcome {
            change_id: change_id.to_string(),
            mutation: QueueMutation::Added,
            reducer_changed,
            dynamic_queue_mutated,
            display_status: self.display_status(change_id).await,
        })
    }

    /// Remove a change from the dynamic queue.
    pub async fn remove_from_queue(&self, change_id: &str) -> OperatorResult<QueueOutcome> {
        let reduce_outcome = {
            let mut guard = self.state.write().await;
            guard.apply_command(ReducerCommand::RemoveFromQueue(change_id.to_string()))
        };
        let reducer_changed = matches!(reduce_outcome, ReduceOutcome::Changed(_));

        // A removal is a real dynamic mutation when the change actually left the
        // pending set: either it was sitting in the dynamic queue, or the reducer
        // accepted a Queued -> NotQueued intent transition. A duplicate removal
        // satisfies neither and must not run the hook.
        let removed_from_dynamic_queue = self.queue.remove(change_id).await;
        let dynamic_queue_mutated = reducer_changed || removed_from_dynamic_queue;
        if dynamic_queue_mutated {
            self.hooks.on_queue_remove(change_id).await;
        }

        Ok(QueueOutcome {
            change_id: change_id.to_string(),
            mutation: QueueMutation::Removed,
            reducer_changed,
            dynamic_queue_mutated,
            display_status: self.display_status(change_id).await,
        })
    }

    /// Stop an in-flight change and dequeue it only after confirmed termination.
    ///
    /// Ordering is validate, cancel, confirm, commit. A missing handle, a failed
    /// cancellation, or a confirmation timeout leaves the active reducer state
    /// untouched.
    pub async fn stop_and_dequeue(&self, change_id: &str) -> OperatorResult<OperatorOutcome> {
        let display_status = self.display_status(change_id).await;
        let was_active = is_active_status(&display_status);

        let waiter = match self.queue.request_cancellation(change_id).await {
            Err(message) => {
                return Err(OperatorCommandError::CancellationFailed {
                    change_id: change_id.to_string(),
                    message,
                })
            }
            Ok(Some(waiter)) => waiter,
            Ok(None) if was_active => {
                // An active change without a handle cannot be proven terminated,
                // so dequeue must not be applied.
                return Err(OperatorCommandError::MissingCancellationHandle {
                    change_id: change_id.to_string(),
                });
            }
            // Idle/queued rows have no task to terminate.
            Ok(None) => TerminationWaiter::already_terminated(),
        };

        if tokio::time::timeout(self.cancellation_timeout, waiter.wait())
            .await
            .is_err()
        {
            return Err(OperatorCommandError::TerminationTimeout {
                change_id: change_id.to_string(),
                waited: self.cancellation_timeout,
            });
        }

        let reduce_outcome = {
            let mut guard = self.state.write().await;
            guard.apply_command(ReducerCommand::DequeueChange(change_id.to_string()))
        };
        if matches!(reduce_outcome, ReduceOutcome::NoOp) {
            return Ok(OperatorOutcome::NoOp {
                change_id: change_id.to_string(),
                reason: NoOpReason::ReducerRejected,
            });
        }
        self.marks.set(change_id, false);

        Ok(OperatorOutcome::Dequeued {
            change_id: change_id.to_string(),
        })
    }

    /// Route a retry request for one change.
    pub async fn retry_change(&self, change_id: &str) -> OperatorResult<RetryPlan> {
        let display_status = self.display_status(change_id).await;
        let Some(route) = classify_retry_route(&display_status) else {
            return Err(OperatorCommandError::RetryUnsupported {
                change_id: change_id.to_string(),
                display_status,
            });
        };
        let plan = self.apply_retry_route(change_id, route).await;
        Ok(plan)
    }

    /// Route a bulk retry request.
    ///
    /// Changes without retryable evidence are skipped rather than rejected, so a
    /// bulk retry never consumes an unsupported or identity-mismatched hold.
    pub async fn retry_errors(&self, change_ids: &[String]) -> RetryPlan {
        let mut plan = RetryPlan {
            change_ids: Vec::new(),
            routes: Vec::new(),
            explicit_retry: false,
        };
        for change_id in change_ids {
            let display_status = self.display_status(change_id).await;
            let Some(route) = classify_retry_route(&display_status) else {
                continue;
            };
            let accepted = self.apply_retry_route(change_id, route).await;
            plan.change_ids.extend(accepted.change_ids);
            plan.routes.extend(accepted.routes);
            plan.explicit_retry |= accepted.explicit_retry;
        }
        plan
    }

    async fn apply_retry_route(&self, change_id: &str, route: RetryRoute) -> RetryPlan {
        let command = match route {
            RetryRoute::TerminalError => ReducerCommand::RetryError(change_id.to_string()),
            // A reconciled acceptance hold resumes through the explicit-retry run
            // path; the reducer only has to restore ordinary queue intent.
            RetryRoute::AcceptanceStall => ReducerCommand::AddToQueue(change_id.to_string()),
        };
        let reduce_outcome = {
            let mut guard = self.state.write().await;
            guard.apply_command(command)
        };
        if matches!(reduce_outcome, ReduceOutcome::NoOp) {
            return RetryPlan {
                change_ids: Vec::new(),
                routes: Vec::new(),
                explicit_retry: false,
            };
        }
        self.marks.set(change_id, true);
        RetryPlan {
            change_ids: vec![change_id.to_string()],
            routes: vec![route],
            // Both routes must start the run with explicit-retry semantics: it
            // releases the repair budget and lets a valid acceptance hold resume
            // acceptance instead of rerunning apply.
            explicit_retry: true,
        }
    }
}

// ============================================================================
// DynamicQueue adapter
// ============================================================================

#[async_trait]
impl QueuePort for crate::tui::queue::DynamicQueue {
    async fn add(&self, change_id: &str) -> bool {
        self.push(change_id.to_string()).await
    }

    async fn remove(&self, change_id: &str) -> bool {
        let removed_from_queue = crate::tui::queue::DynamicQueue::remove(self, change_id).await;
        // Always record the pending removal so the scheduler drops the change from
        // its own pending set, even when it was not sitting in the dynamic queue.
        self.mark_removed(change_id.to_string()).await;
        removed_from_queue
    }

    async fn request_cancellation(
        &self,
        change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        Ok(
            crate::tui::queue::DynamicQueue::request_cancellation(self, change_id)
                .await
                .map(TerminationWaiter::new),
        )
    }

    async fn notify_scheduler(&self) {
        crate::tui::queue::DynamicQueue::notify_scheduler(self);
    }
}

#[cfg(test)]
mod tests;
