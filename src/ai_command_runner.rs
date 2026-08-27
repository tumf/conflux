//! Common AI command runner layer for unified stagger state management.
//!
//! This module provides a shared execution layer for all AI-driven commands
//! (apply, archive, resolve, analyze) to ensure consistent stagger delays
//! across every execution frontend.

use crate::command_queue::{CommandQueue, CommandQueueConfig};
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::process_manager::{
    cleanup_process_group_verified, CommandTermination, ManagedChild, ProcessGroupCleanupReport,
    StreamingChildHandle, DEFAULT_PROCESS_GROUP_CLEANUP_TIMEOUT_MS,
    DEFAULT_PROCESS_GROUP_SIGTERM_GRACE_MS,
};
use crate::stream_json_textifier::{process_stdout_line, StreamJsonTextBuffer};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// Shared stagger state type: Arc<Mutex<Option<Instant>>>
/// This type is shared across all AI command executions to coordinate stagger delays
pub type SharedStaggerState = Arc<Mutex<Option<Instant>>>;

// ---------------------------------------------------------------------------
// Run command scope
// ---------------------------------------------------------------------------

/// Bounded budget for proving that every run-owned AI command reached
/// quiescence after shutdown started.
///
/// It exceeds one command's SIGTERM grace plus process-group verification path
/// and assumes active cleanups run concurrently, so together with the existing
/// 90-second pending merge/base-lane drain it stays inside the scheduler's
/// 120-second outer cancellation boundary instead of adding a new layer on top
/// of it.
pub const RUN_COMMAND_CLEANUP_DEADLINE: Duration = Duration::from_secs(30);

/// SIGTERM grace used when the scope itself force-cleans a retained identity.
const SCOPE_ESCALATION_SIGTERM_GRACE_MS: u64 = 500;

/// Total budget for one retained-identity managed escalation sweep.
const SCOPE_ESCALATION_TOTAL_MS: u64 = 5_000;

/// Bound on proving that one targeted force-stop's SIGKILL emptied the group.
///
/// It bounds a *proof*, not a grace window: nothing is given time to shut down
/// cooperatively, and the budget only limits how long membership is polled
/// before the result is reported as unconfirmed.
pub const FORCE_STOP_CHANGE_KILL_BUDGET: Duration = Duration::from_secs(5);

/// Evidence from one targeted force-stop of a single change's process groups.
///
/// Every field describes only the addressed change: an unrelated change's
/// processes are not reachable from the path that produces this, so a report
/// can never account for one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeForceStopReport {
    /// Owned process identities this force-stop signalled.
    pub identities: usize,
    /// Identities proven empty after SIGKILL.
    pub confirmed: usize,
    /// One bounded diagnostic per identity whose emptiness was not proven.
    pub unconfirmed: Vec<String>,
}

impl ChangeForceStopReport {
    /// Whether every signalled identity was proven reaped.
    ///
    /// A target that owned no process at all is confirmed: there was nothing to
    /// terminate, which is exactly the dequeue-only case.
    pub fn is_confirmed(&self) -> bool {
        self.unconfirmed.is_empty()
    }

    /// One bounded operator-facing summary of what this force-stop proved.
    pub fn diagnostics(&self) -> String {
        if self.is_confirmed() {
            return format!(
                "targeted force-stop confirmed (identities={}, killed={})",
                self.identities, self.confirmed
            );
        }
        format!(
            "targeted force-stop unconfirmed (identities={}, killed={}): {}",
            self.identities,
            self.confirmed,
            self.unconfirmed.join("; ")
        )
    }
}

/// Lifecycle of one run-owned AI command execution inside its scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionPhase {
    /// Admission is reserved; the runner task has not spawned a process yet.
    WaitingToSpawn,
    /// A real owned process group exists for the current attempt.
    Running,
    /// Termination and verification are in progress; the identity stays registered.
    Cleaning,
    /// The runner task ended without proving quiescence. The identity is kept
    /// for bounded managed escalation and diagnostics.
    UnconfirmedRetained,
}

impl ExecutionPhase {
    /// Whether the runner task behind this registration is still running.
    fn is_active(self) -> bool {
        !matches!(self, Self::UnconfirmedRetained)
    }
}

/// One run-owned execution tracked by the scope.
#[derive(Debug, Clone)]
struct ScopeEntry {
    operation: Option<String>,
    change_id: Option<String>,
    phase: ExecutionPhase,
    /// Owned process identities (PGID == leader PID) whose quiescence is not
    /// yet proven. Retained across attempts so an earlier unproven attempt
    /// cannot be forgotten by a later one.
    unproven_pids: Vec<u32>,
    /// Last actionable cleanup diagnostic recorded for this execution.
    detail: Option<String>,
}

impl ScopeEntry {
    fn describe(&self) -> String {
        format!(
            "op={}, change_id={}, pgids={:?}: {}",
            self.operation.as_deref().unwrap_or("unknown"),
            self.change_id.as_deref().unwrap_or("none"),
            self.unproven_pids,
            self.detail
                .as_deref()
                .unwrap_or("owned process-set quiescence was never proven")
        )
    }
}

#[derive(Debug, Default)]
struct ScopeState {
    /// Once closed, no new execution and no new process spawn is admitted.
    closed: bool,
    next_id: u64,
    entries: std::collections::BTreeMap<u64, ScopeEntry>,
}

struct RunCommandScopeInner {
    state: std::sync::Mutex<ScopeState>,
    cancel: tokio_util::sync::CancellationToken,
    quiescence: tokio::sync::Notify,
}

/// Ephemeral, clone-shared ownership of every AI command one orchestration
/// invocation launches.
///
/// The scope is the missing layer above `StreamingChildHandle`: it closes final
/// spawn admission atomically, notifies runner tasks directly, and retains each
/// execution and owned process identity until the runner task ended *and*
/// typed cleanup evidence confirmed quiescence. It is process-local, is
/// recreated for every run, and is never persisted or used for restart routing.
#[derive(Clone)]
pub struct RunCommandScope {
    inner: Arc<RunCommandScopeInner>,
}

impl Default for RunCommandScope {
    fn default() -> Self {
        Self::new()
    }
}

impl RunCommandScope {
    /// Create one fresh, open scope for a single orchestration invocation.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RunCommandScopeInner {
                state: std::sync::Mutex::new(ScopeState::default()),
                cancel: tokio_util::sync::CancellationToken::new(),
                quiescence: tokio::sync::Notify::new(),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ScopeState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Cancellation observed directly by runner tasks.
    ///
    /// Runner shutdown never depends on a caller-held `StreamingChildHandle`:
    /// dropping the handle cannot silence this token.
    pub fn cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.inner.cancel.clone()
    }

    /// Whether admission is already closed.
    pub fn is_closed(&self) -> bool {
        self.lock().closed
    }

    /// Whether both handles are clones of the *same* scope.
    ///
    /// Scope identity is what the run's cleanup barrier is built on: a runner
    /// carrying an equal-but-separate scope reports into a barrier nobody waits
    /// on.
    #[allow(dead_code)] // Read by scope-ownership coverage, not by the binary.
    pub fn is_same(&self, other: &RunCommandScope) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Atomically close final spawn admission and broadcast runner shutdown.
    ///
    /// Idempotent. A command parked in stagger or retry delay cannot reach
    /// `Command::spawn` after this returns, because the admission check and the
    /// spawn share this lock.
    pub fn close(&self) {
        {
            let mut state = self.lock();
            state.closed = true;
        }
        self.inner.cancel.cancel();
        self.inner.quiescence.notify_waiters();
    }

    /// Close this scope as soon as `token` is cancelled.
    ///
    /// The scope observes the run's global token directly, so a caller blocked
    /// on command output far from the scheduler loop still starts cleaning up
    /// the instant cancellation happens.
    pub fn link_cancellation(&self, token: tokio_util::sync::CancellationToken) {
        let scope = self.clone();
        tokio::spawn(async move {
            token.cancelled().await;
            scope.close();
        });
    }

    /// Number of registrations whose runner task has not ended yet.
    pub fn active_executions(&self) -> usize {
        self.lock()
            .entries
            .values()
            .filter(|entry| entry.phase.is_active())
            .count()
    }

    /// Owned process identities the scope still cannot prove quiescent.
    #[allow(dead_code)] // Consumed by TUI escalation coverage and heavy regressions.
    pub fn retained_process_ids(&self) -> Vec<u32> {
        let state = self.lock();
        let mut pids: Vec<u32> = state
            .entries
            .values()
            .flat_map(|entry| entry.unproven_pids.iter().copied())
            .collect();
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    /// Whether every run-owned command for `change_id` reached terminal cleanup.
    ///
    /// A change with no registration at all is quiescent: it owns no command.
    /// A change with a live or retained registration is not, and its execution
    /// `done` handshake must stay unfired.
    pub fn change_is_quiescent(&self, change_id: &str) -> bool {
        !self
            .lock()
            .entries
            .values()
            .any(|entry| entry.change_id.as_deref() == Some(change_id))
    }

    /// Whether this scope owns a live managed process group for `change_id`.
    ///
    /// The eligibility fact a targeted force-stop needs, read from the same
    /// ownership graph the kill itself walks: a change with a registration but
    /// no spawned identity is admitted work with nothing to signal, and a change
    /// with no registration owns no command at all.
    pub fn change_owns_managed_process(&self, change_id: &str) -> bool {
        self.lock().entries.values().any(|entry| {
            entry.change_id.as_deref() == Some(change_id) && !entry.unproven_pids.is_empty()
        })
    }

    /// Immediately SIGKILL every process group this scope owns for `change_id`
    /// and prove each one was reaped.
    ///
    /// Target-scoped by construction: the entry filter is the *only* way a PGID
    /// reaches the signal, so an unrelated change's process group is
    /// unreachable from here — there is no PID lookup, no "kill everything
    /// retained", and no scope closure. Admission stays open and every other
    /// registration keeps running, which is what separates this from
    /// [`Self::force_cleanup_retained`].
    ///
    /// No SIGTERM is sent. `budget` bounds the proof of quiescence, not a grace
    /// window, and an identity whose emptiness cannot be proven is reported as
    /// unconfirmed rather than dropped from the ownership graph.
    pub async fn force_stop_change(
        &self,
        change_id: &str,
        budget: Duration,
    ) -> ChangeForceStopReport {
        let targets: Vec<(u64, ScopeEntry)> = {
            let state = self.lock();
            state
                .entries
                .iter()
                .filter(|(_, entry)| entry.change_id.as_deref() == Some(change_id))
                .filter(|(_, entry)| !entry.unproven_pids.is_empty())
                .map(|(id, entry)| (*id, entry.clone()))
                .collect()
        };

        let per_identity_ms = budget.as_millis().min(u64::MAX as u128) as u64;
        let mut report = ChangeForceStopReport::default();

        for (id, entry) in targets {
            let mut proven = Vec::new();
            for pid in &entry.unproven_pids {
                report.identities += 1;
                let evidence = crate::process_manager::kill_process_group_immediately(
                    *pid,
                    per_identity_ms,
                    entry.operation.as_deref(),
                    entry.change_id.as_deref(),
                )
                .await;
                if evidence.is_confirmed() {
                    report.confirmed += 1;
                    proven.push(*pid);
                } else {
                    report
                        .unconfirmed
                        .push(format!("pgid={pid}: {}", evidence.diagnostics()));
                }
            }

            let mut state = self.lock();
            if let Some(current) = state.entries.get_mut(&id) {
                current.unproven_pids.retain(|pid| !proven.contains(pid));
                if current.unproven_pids.is_empty() && !current.phase.is_active() {
                    state.entries.remove(&id);
                }
            }
            drop(state);
            self.inner.quiescence.notify_waiters();
        }

        report
    }

    /// Reserve one execution before its runner task is spawned.
    ///
    /// Returns `None` when the scope is already closing, which is what refuses
    /// a command that raced shutdown before anything was launched.
    fn register(&self, operation: Option<&str>, change_id: Option<&str>) -> Option<ScopeExecution> {
        let mut state = self.lock();
        if state.closed {
            return None;
        }
        state.next_id += 1;
        let id = state.next_id;
        state.entries.insert(
            id,
            ScopeEntry {
                operation: operation.map(|s| s.to_string()),
                change_id: change_id.map(|s| s.to_string()),
                phase: ExecutionPhase::WaitingToSpawn,
                unproven_pids: Vec::new(),
                detail: None,
            },
        );
        drop(state);
        Some(ScopeExecution {
            scope: self.clone(),
            id,
            finished: std::sync::atomic::AtomicBool::new(false),
        })
    }

    fn set_phase(&self, id: u64, phase: ExecutionPhase) {
        let mut state = self.lock();
        if let Some(entry) = state.entries.get_mut(&id) {
            entry.phase = phase;
        }
        drop(state);
        self.inner.quiescence.notify_waiters();
    }

    fn remove_entry(&self, id: u64) {
        let mut state = self.lock();
        state.entries.remove(&id);
        drop(state);
        self.inner.quiescence.notify_waiters();
    }

    /// Close the barrier on this scope and wait for bounded quiescence.
    pub async fn shutdown(&self, deadline: Duration) -> RunCommandScopeCleanup {
        self.close();
        self.wait_quiescent(deadline).await
    }

    /// Await runner-task exit for every registration, then escalate whatever
    /// could not be proven quiescent, all inside one absolute `deadline`.
    pub async fn wait_quiescent(&self, deadline: Duration) -> RunCommandScopeCleanup {
        let started = Instant::now();
        let mut timed_out = false;

        loop {
            if self.active_executions() == 0 {
                break;
            }
            let elapsed = started.elapsed();
            if elapsed >= deadline {
                timed_out = true;
                break;
            }
            // Register interest before re-reading so a completion that lands
            // between the check and the wait cannot be missed.
            let notified = self.inner.quiescence.notified();
            if self.active_executions() == 0 {
                break;
            }
            if tokio::time::timeout(deadline - elapsed, notified)
                .await
                .is_err()
            {
                timed_out = true;
                break;
            }
        }

        let remaining = deadline.saturating_sub(started.elapsed());
        self.escalate_retained(remaining, timed_out).await
    }

    /// Force-clean and verify every retained owned identity.
    ///
    /// Used by the local TUI supervisor when the orchestrator task itself
    /// stopped cooperating: the scope is retained outside that task, so it is
    /// still the path to the PGIDs the run owns.
    pub async fn force_cleanup_retained(&self, budget: Duration) -> RunCommandScopeCleanup {
        self.close();
        self.escalate_retained(budget, self.active_executions() > 0)
            .await
    }

    async fn escalate_retained(&self, budget: Duration, timed_out: bool) -> RunCommandScopeCleanup {
        let pending: Vec<(u64, ScopeEntry)> = {
            let state = self.lock();
            state
                .entries
                .iter()
                .filter(|(_, entry)| !entry.unproven_pids.is_empty())
                .map(|(id, entry)| (*id, entry.clone()))
                .collect()
        };

        let per_identity_ms = if budget.is_zero() {
            0
        } else {
            SCOPE_ESCALATION_TOTAL_MS.min(budget.as_millis() as u64)
        };

        let mut escalated = 0usize;
        for (id, entry) in pending {
            // Per entry: only this entry's own confirmed identities may be
            // dropped from its list.
            let mut proven = Vec::new();
            let mut first_failure = None;
            for pid in &entry.unproven_pids {
                let report = crate::process_manager::cleanup_process_group_verified(
                    *pid,
                    SCOPE_ESCALATION_SIGTERM_GRACE_MS.min(per_identity_ms),
                    per_identity_ms,
                    entry.operation.as_deref(),
                    entry.change_id.as_deref(),
                )
                .await;
                if report.is_confirmed() {
                    proven.push(*pid);
                } else if first_failure.is_none() {
                    first_failure = Some(report.diagnostics());
                }
            }
            escalated += proven.len();

            let mut state = self.lock();
            if let Some(current) = state.entries.get_mut(&id) {
                current.unproven_pids.retain(|pid| !proven.contains(pid));
                if let Some(detail) = first_failure {
                    current.detail = Some(detail);
                }
                if current.unproven_pids.is_empty() && !current.phase.is_active() {
                    state.entries.remove(&id);
                }
            }
        }

        let state = self.lock();
        let unconfirmed: Vec<String> = state.entries.values().map(ScopeEntry::describe).collect();
        drop(state);

        RunCommandScopeCleanup {
            escalated,
            unconfirmed,
            timed_out,
        }
    }
}

/// Result of one bounded run command scope cleanup barrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCommandScopeCleanup {
    /// Owned identities that only managed escalation could prove quiescent.
    pub escalated: usize,
    /// One bounded actionable diagnostic per unproven registration.
    pub unconfirmed: Vec<String>,
    /// Whether the bounded barrier expired before every runner task ended.
    pub timed_out: bool,
}

impl RunCommandScopeCleanup {
    /// Whether the run may treat every owned command as quiescent.
    pub fn is_quiescent(&self) -> bool {
        self.unconfirmed.is_empty() && !self.timed_out
    }

    /// One bounded operator-facing summary of what could not be proven.
    pub fn diagnostics(&self) -> String {
        if self.is_quiescent() {
            return format!(
                "run command cleanup confirmed (escalated={})",
                self.escalated
            );
        }
        format!(
            "run command cleanup unconfirmed (timed_out={}, escalated={}): {}",
            self.timed_out,
            self.escalated,
            self.unconfirmed.join("; ")
        )
    }
}

/// A run-owned execution reserved in a [`RunCommandScope`].
///
/// Held by the detached runner task for its whole life. Dropping it without
/// [`ScopeExecution::finish`] — a panicked task — leaves the registration
/// retained as unconfirmed rather than silently quiescent.
struct ScopeExecution {
    scope: RunCommandScope,
    id: u64,
    finished: std::sync::atomic::AtomicBool,
}

impl ScopeExecution {
    /// Whether scope shutdown has already started.
    fn is_shutdown(&self) -> bool {
        self.scope.is_closed()
    }

    fn cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.scope.cancel_token()
    }

    fn mark_waiting_to_spawn(&self) {
        self.scope
            .set_phase(self.id, ExecutionPhase::WaitingToSpawn);
    }

    fn mark_cleaning(&self) {
        self.scope.set_phase(self.id, ExecutionPhase::Cleaning);
    }

    /// Final admission serialized with scope shutdown.
    ///
    /// The admission check and `spawn` share one critical section, so a scope
    /// closed anywhere in between cannot leave a started process behind.
    /// Returns `None` when admission was refused.
    fn admit_spawn<F>(&self, spawn: F) -> Option<std::io::Result<tokio::process::Child>>
    where
        F: FnOnce() -> std::io::Result<tokio::process::Child>,
    {
        let mut state = self.scope.lock();
        if state.closed {
            return None;
        }
        let result = spawn();
        if let Ok(child) = &result {
            if let Some(entry) = state.entries.get_mut(&self.id) {
                entry.phase = ExecutionPhase::Running;
                if let Some(pid) = child.id() {
                    entry.unproven_pids.push(pid);
                }
            }
        }
        Some(result)
    }

    /// Record typed cleanup evidence for one owned identity.
    fn record_cleanup(&self, pid: u32, report: &ProcessGroupCleanupReport) {
        let mut state = self.scope.lock();
        if let Some(entry) = state.entries.get_mut(&self.id) {
            if report.is_confirmed() {
                entry.unproven_pids.retain(|owned| *owned != pid);
            } else {
                entry.detail = Some(report.diagnostics());
            }
        }
        drop(state);
        self.scope.inner.quiescence.notify_waiters();
    }

    /// The runner task is ending. The registration disappears only when every
    /// owned identity is already proven quiescent.
    fn finish(&self) {
        if self.finished.swap(true, Ordering::SeqCst) {
            return;
        }
        let retained = {
            let mut state = self.scope.lock();
            match state.entries.get_mut(&self.id) {
                Some(entry) if entry.unproven_pids.is_empty() => false,
                Some(entry) => {
                    entry.phase = ExecutionPhase::UnconfirmedRetained;
                    true
                }
                None => false,
            }
        };
        if retained {
            self.scope.inner.quiescence.notify_waiters();
        } else {
            self.scope.remove_entry(self.id);
        }
    }
}

/// A scope registration created directly by in-crate tests.
///
/// Lets a caller hold the run's cleanup barrier open — and release it as either
/// confirmed or unproven — without spawning a real process.
#[cfg(test)]
pub(crate) struct ScopeTestRegistration(ScopeExecution);

#[cfg(test)]
impl ScopeTestRegistration {
    /// Release as a runner task that ended with proven quiescence.
    pub(crate) fn release_confirmed(self) {
        self.0.finish();
    }
}

#[cfg(test)]
impl RunCommandScope {
    /// Reserve an execution the way `execute_streaming_with_retry` does.
    pub(crate) fn register_for_test(
        &self,
        operation: &str,
        change_id: Option<&str>,
    ) -> Option<ScopeTestRegistration> {
        self.register(Some(operation), change_id)
            .map(ScopeTestRegistration)
    }

    /// Reserve an execution that already owns an unproven process identity.
    pub(crate) fn register_unproven_for_test(
        &self,
        operation: &str,
        change_id: Option<&str>,
        pid: u32,
    ) -> ScopeTestRegistration {
        let execution = self
            .register(Some(operation), change_id)
            .expect("an open scope admits the registration");
        {
            let mut state = self.lock();
            if let Some(entry) = state.entries.get_mut(&execution.id) {
                entry.phase = ExecutionPhase::Running;
                entry.unproven_pids.push(pid);
            }
        }
        ScopeTestRegistration(execution)
    }
}

impl Drop for ScopeExecution {
    fn drop(&mut self) {
        if self.finished.swap(true, Ordering::SeqCst) {
            return;
        }
        // A runner task that never reached its own finalization proved nothing:
        // retain the registration so the barrier still sees it.
        let mut state = self.scope.lock();
        if let Some(entry) = state.entries.get_mut(&self.id) {
            entry.phase = ExecutionPhase::UnconfirmedRetained;
            if entry.detail.is_none() {
                entry.detail =
                    Some("the runner task ended without publishing cleanup evidence".to_string());
            }
            if entry.unproven_pids.is_empty() {
                state.entries.remove(&self.id);
            }
        }
        drop(state);
        self.scope.inner.quiescence.notify_waiters();
    }
}

/// Output line from a child process
#[derive(Debug, Clone)]
#[allow(dead_code)] // Infrastructure ready, integration pending (tasks 3.2, 3.3, 4.1-4.3)
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

/// Common AI command runner with shared stagger state.
///
/// This runner wraps CommandQueue and provides streaming execution
/// for AI-driven commands (apply, archive, resolve, analyze).
/// The shared stagger state ensures consistent delays across all
/// parallel workspaces and command types.
#[derive(Clone)]
#[allow(dead_code)] // Infrastructure ready, integration pending (tasks 3.2, 3.3, 4.1-4.3)
pub struct AiCommandRunner {
    command_queue: CommandQueue,
    /// When true, stdout lines that are Claude Code stream-json (NDJSON) events are
    /// converted to human-readable text before being emitted to the output channel.
    stream_json_textify: bool,
    /// When true, perform a strict post-completion SIGTERM→SIGKILL sweep on the spawned
    /// process group after every command outcome (success, failure, cancellation, or
    /// inactivity timeout) to prevent orphaned background processes.
    strict_process_cleanup: bool,
    /// Bounded budget for proving that the owned process group became quiescent
    /// after termination was requested. Callers that gate repository work on
    /// cleanup evidence fail when this budget expires without proof.
    process_group_cleanup_timeout_ms: u64,
    command_envs: HashMap<String, String>,
    /// Invocation-scoped ownership of every command this runner launches.
    ///
    /// `None` is the caller-owned lifecycle used outside a scheduler run (the
    /// TUI's standalone worktree command and in-crate tests): such a command is
    /// deliberately not attached to a later run's scope.
    run_command_scope: Option<RunCommandScope>,
}

impl AiCommandRunner {
    /// Create a new AiCommandRunner with shared stagger state.
    ///
    /// Stream-JSON textification is enabled by default.  Use
    /// [`AiCommandRunner::set_stream_json_textify`] to override.
    ///
    /// # Arguments
    ///
    /// * `config` - CommandQueue configuration
    /// * `shared_state` - Shared last execution timestamp for stagger coordination
    pub fn new(config: CommandQueueConfig, shared_state: SharedStaggerState) -> Self {
        Self {
            command_queue: CommandQueue::new_with_shared_state(config, shared_state),
            stream_json_textify: true,
            strict_process_cleanup: true,
            process_group_cleanup_timeout_ms: DEFAULT_PROCESS_GROUP_CLEANUP_TIMEOUT_MS,
            command_envs: HashMap::new(),
            run_command_scope: None,
        }
    }

    /// Create a runner with every command-related setting from the orchestrator config.
    pub fn from_orchestrator_config(
        config: &OrchestratorConfig,
        shared_state: SharedStaggerState,
    ) -> Self {
        let mut runner = Self::new(CommandQueueConfig::from(config), shared_state);
        runner.set_stream_json_textify(config.get_stream_json_textify());
        runner.set_strict_process_cleanup(config.get_command_strict_process_cleanup());
        runner.set_command_envs(config.get_command_envs());
        runner
    }

    /// Create a run-owned runner: same settings, bound to `scope`.
    ///
    /// Every production command surface of one orchestration invocation is
    /// constructed through here (or cloned from a runner that was), so no run
    /// path can build an unscoped runner out of a bare stagger timestamp.
    pub fn for_run(
        config: &OrchestratorConfig,
        shared_state: SharedStaggerState,
        scope: RunCommandScope,
    ) -> Self {
        let mut runner = Self::from_orchestrator_config(config, shared_state);
        runner.run_command_scope = Some(scope);
        runner
    }

    /// The invocation scope this runner is bound to, if any.
    #[allow(dead_code)] // Read by scope-ownership coverage, not by the binary.
    pub fn run_command_scope(&self) -> Option<&RunCommandScope> {
        self.run_command_scope.as_ref()
    }

    /// Bind (or rebind) this runner to an invocation scope.
    pub fn set_run_command_scope(&mut self, scope: RunCommandScope) {
        self.run_command_scope = Some(scope);
    }

    pub fn set_command_envs(&mut self, envs: HashMap<String, String>) {
        self.command_envs = envs;
    }

    /// Override stream-JSON textification setting.
    ///
    /// When `false`, raw stdout lines are forwarded unchanged (useful for troubleshooting).
    pub fn set_stream_json_textify(&mut self, enabled: bool) {
        self.stream_json_textify = enabled;
    }

    /// Override strict post-completion process-group cleanup setting.
    ///
    /// When `false`, no SIGTERM/SIGKILL sweep is performed after a command completes
    /// successfully.  Cancellation and inactivity-timeout paths continue to clean up
    /// regardless (they have independent termination logic).  Set to `false` only for
    /// debugging workflows where intentional background processes must outlive the command.
    pub fn set_strict_process_cleanup(&mut self, enabled: bool) {
        self.strict_process_cleanup = enabled;
    }

    /// Override the bounded budget for proving process-group quiescence.
    ///
    /// A shorter budget makes an unconfirmed cleanup surface sooner; it never
    /// makes an unproven group count as quiescent.
    #[allow(dead_code)] // Exercised by the process-group barrier tests.
    pub fn set_process_group_cleanup_timeout_ms(&mut self, timeout_ms: u64) {
        self.process_group_cleanup_timeout_ms = timeout_ms;
    }

    /// Get access to the underlying CommandQueue configuration.
    ///
    /// This is useful for implementing custom retry logic that respects
    /// the configured retry parameters.
    #[allow(dead_code)] // Used by parallel executor for retry logic
    pub fn queue_config(&self) -> &crate::command_queue::CommandQueueConfig {
        self.command_queue.config()
    }

    #[cfg(test)]
    pub(crate) fn shared_stagger_state(&self) -> SharedStaggerState {
        self.command_queue.shared_stagger_state()
    }

    /// Execute a command with streaming output, stagger delay, and automatic retry.
    ///
    /// Returns a real process handle ([`StreamingChildHandle`]) that targets the actual
    /// spawned command (or its process group) rather than a placeholder. Cancellation and
    /// inactivity-timeout termination send SIGTERM/SIGKILL to the full process group, so
    /// pipeline children (e.g. `claude | jq`) cannot be left as orphans.
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute (run via `sh -c`)
    /// * `cwd` - Optional working directory (for worktree execution)
    /// * `operation_type` - Optional operation type for logging (apply/archive/resolve/analyze/acceptance)
    /// * `change_id` - Optional change ID for logging context
    ///
    /// # Returns
    ///
    /// A tuple of (`StreamingChildHandle`, `Receiver<OutputLine>`). Drain the receiver first
    /// (it closes when all retries complete), then call `.wait()` on the handle to obtain
    /// the final exit status.
    ///
    /// # Retry Behaviour
    ///
    /// Retries are governed by the `CommandQueueConfig`:
    /// - Error pattern matching (`retry_error_patterns`)
    /// - Short execution duration (`retry_if_duration_under_secs`)
    /// - Non-zero exit code (agent crash)
    ///
    /// Retry notifications are emitted as stderr lines on the output channel.
    pub async fn execute_streaming_with_retry(
        &self,
        command: &str,
        cwd: Option<&Path>,
        operation_type: Option<&str>,
        change_id: Option<&str>,
    ) -> Result<(StreamingChildHandle, mpsc::Receiver<OutputLine>)> {
        // Admission is reserved before anything is launched. A scope that is
        // already closing refuses the execution outright, so a command that
        // raced shutdown never reaches stagger, retry, or `Command::spawn`.
        let scope_execution = match &self.run_command_scope {
            Some(scope) => match scope.register(operation_type, change_id) {
                Some(execution) => Some(execution),
                None => {
                    return Ok(refused_after_shutdown(
                        "run command admission is closed",
                        operation_type,
                        change_id,
                    ));
                }
            },
            None => None,
        };

        // Output channel that callers drain while the background task streams.
        let (out_tx, out_rx) = mpsc::channel::<OutputLine>(1024);

        // Cancel signal: StreamingChildHandle.terminate() → background task.
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        // Shared current PID (0 = no process running).
        let current_pid = Arc::new(AtomicU32::new(0));

        // Completion signal: background task → StreamingChildHandle.wait().
        let (status_tx, status_rx) = tokio::sync::oneshot::channel::<std::process::ExitStatus>();

        // Process-group cleanup evidence: background task → callers that gate
        // repository finalization on confirmed quiescence. Always published
        // before the final status so a caller that observes completion can also
        // observe why cleanup succeeded or failed.
        let (cleanup_tx, cleanup_rx) = tokio::sync::oneshot::channel::<ProcessGroupCleanupReport>();

        // Why this invocation ended: an ordinary exit, the inactivity timeout,
        // the absolute runtime limit, or a deliberate termination. Published
        // before the final status so a caller that observes completion can also
        // decide whether another attempt is admissible at all.
        let (termination_tx, termination_rx) =
            tokio::sync::oneshot::channel::<CommandTermination>();

        // Clone values for the background task.
        let command_queue = self.command_queue.clone();
        let command_str = command.to_string();
        let cwd_owned = cwd.map(|p| p.to_path_buf());
        let operation_type_owned = operation_type.map(|s| s.to_string());
        let change_id_owned = change_id.map(|s| s.to_string());
        let pid_arc = current_pid.clone();
        let stream_json_textify = self.stream_json_textify;
        let strict_process_cleanup = self.strict_process_cleanup;
        let cleanup_timeout_ms = self.process_group_cleanup_timeout_ms;
        let command_envs = self.command_envs.clone();

        // Spawn the background retry task. It owns the real child processes and responds
        // to the cancel signal by terminating the current process group via SIGTERM/SIGKILL.
        tokio::spawn(async move {
            // Owned for the whole runner task: the registration is what keeps
            // the run's cleanup barrier waiting, independently of whether the
            // caller still holds its `StreamingChildHandle`.
            let scope_execution = scope_execution;
            let scope_cancel = scope_execution.as_ref().map(ScopeExecution::cancel_token);
            let scoped = scope_execution.is_some();

            let max_retries = command_queue.config().max_retries;
            let retry_delay_ms = command_queue.config().retry_delay_ms;
            let inactivity_timeout_secs = command_queue.config().inactivity_timeout_secs;
            let kill_grace_secs = command_queue.config().inactivity_kill_grace_secs;
            let inactivity_timeout_max_retries =
                command_queue.config().inactivity_timeout_max_retries;
            // Absolute invocation deadline, selected here from the operation
            // type this invocation already declared: Acceptance carries its own
            // shorter deadline, every other class keeps `command_max_runtime_secs`
            // and its `0`-disable semantics. Resolving it inside the common
            // runner rather than at the call site is what keeps a bounded class
            // from being an optional per-caller decision.
            let max_runtime_secs = command_queue
                .config()
                .effective_max_runtime_secs(operation_type_owned.as_deref());

            // cancel_rx is wrapped in Option so we can neutralise it after first use.
            let mut cancel_rx_opt = Some(cancel_rx);
            let mut cancel_observed = false;

            // Terminal reason for this invocation, published exactly once.
            let mut termination_tx = Some(termination_tx);

            // Cleanup evidence for the attempt that ends this execution. A run
            // that never spawns a process has no owned group to prove quiescent.
            let mut cleanup_tx = Some(cleanup_tx);
            let mut cleanup_report = ProcessGroupCleanupReport::not_applicable(
                "no command process was started, so there is no owned process group",
            );

            let mut attempt = 0u32;
            let mut inactivity_retries_used = 0u32;
            let mut final_exit_status: Option<std::process::ExitStatus> = None;
            // Stays `NotStarted` for every `break 'retry` that never reached a
            // completed attempt (refused admission, launch failure, unwaitable
            // child), so those are never reported as an ordinary exit.
            let mut final_termination = CommandTermination::NotStarted;

            'retry: loop {
                // Shutdown recheck before every attempt. A scope that closed
                // while the previous retry delay was sleeping admits no further
                // attempt, so the counter never advances after closure.
                if let Some(execution) = &scope_execution {
                    if execution.is_shutdown() {
                        let _ = out_tx
                            .send(OutputLine::Stderr(shutdown_refusal_line(
                                "retry",
                                operation_type_owned.as_deref(),
                                change_id_owned.as_deref(),
                                attempt + 1,
                            )))
                            .await;
                        break 'retry;
                    }
                    execution.mark_waiting_to_spawn();
                }

                attempt += 1;
                let start_time = Instant::now();

                // Build the real command and attach it to a new process group so the
                // entire pipeline (sh + agent + filter) can be killed as one unit.
                let mut cmd = Command::new("sh");
                cmd.arg("-c")
                    .arg(&command_str)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                if let Some(ref dir) = cwd_owned {
                    cmd.current_dir(dir);
                }
                cmd.envs(
                    command_envs
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.as_str())),
                );

                // Set the spawned process as its own process group leader (PGID = PID).
                // This allows killpg to reach all pipeline children.
                #[cfg(unix)]
                {
                    use crate::process_manager::configure_process_group;
                    configure_process_group(&mut cmd);
                }

                // Apply the stagger delay first, then take final scope
                // admission and spawn inside one critical section. Checking
                // admission before the delay would leave a window in which
                // shutdown starts and a process is still launched.
                command_queue.wait_for_stagger_slot().await;
                let spawned = match &scope_execution {
                    Some(execution) => execution.admit_spawn(|| cmd.spawn()),
                    None => Some(cmd.spawn()),
                };
                let child = match spawned {
                    None => {
                        // Admission closed between registration and spawn: the
                        // command body never runs.
                        let _ = out_tx
                            .send(OutputLine::Stderr(shutdown_refusal_line(
                                "spawn",
                                operation_type_owned.as_deref(),
                                change_id_owned.as_deref(),
                                attempt,
                            )))
                            .await;
                        break 'retry;
                    }
                    Some(Ok(c)) => c,
                    Some(Err(e)) => {
                        error!(
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            attempt,
                            "Failed to spawn command: {}",
                            e
                        );
                        // The caller only ever sees a synthetic failure status
                        // for this path, so the cause travels on the output
                        // channel it already collects. That is what puts a
                        // launch failure into Apply history, the Acceptance
                        // command diagnostic, and the cleanup-review diagnosis
                        // instead of leaving them with a bare exit code.
                        let _ = out_tx
                            .send(OutputLine::Stderr(launch_failure_line(
                                "spawn",
                                operation_type_owned.as_deref(),
                                change_id_owned.as_deref(),
                                attempt,
                                &e.to_string(),
                            )))
                            .await;
                        break 'retry;
                    }
                };

                let mut managed_child = match ManagedChild::new(child) {
                    Ok(mc) => mc,
                    Err(e) => {
                        error!(
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            "Failed to wrap child in ManagedChild: {}",
                            e
                        );
                        let _ = out_tx
                            .send(OutputLine::Stderr(launch_failure_line(
                                "process-manager",
                                operation_type_owned.as_deref(),
                                change_id_owned.as_deref(),
                                attempt,
                                &e.to_string(),
                            )))
                            .await;
                        break 'retry;
                    }
                };

                // Publish the real PID so StreamingChildHandle.id() is accurate.
                let pid = managed_child.id().unwrap_or(0);
                pid_arc.store(pid, Ordering::SeqCst);

                // The absolute deadline starts here — at successful child spawn
                // — not at admission, stagger, or retry-delay time, so queueing
                // never eats the agent's own budget.
                let runtime_deadline = (max_runtime_secs > 0)
                    .then(|| tokio::time::Instant::now() + Duration::from_secs(max_runtime_secs));
                debug!(
                    pid,
                    op = ?operation_type_owned,
                    change_id = ?change_id_owned,
                    attempt,
                    "Streaming child started"
                );

                // Take stdout/stderr handles before lending managed_child to the
                // inactivity/cancel select loop.
                let stdout = managed_child.child.stdout.take();
                let stderr = managed_child.child.stderr.take();

                // Activity channel: readers signal liveness to the inactivity monitor.
                let (activity_tx, mut activity_rx) = mpsc::channel::<()>(100);

                // Stderr accumulator (for retry-condition check after exit).
                let (stderr_acc_tx, mut stderr_acc_rx) = mpsc::channel::<String>(2);

                // Spawn stdout reader.
                let out_tx_stdout = out_tx.clone();
                let activity_tx_stdout = activity_tx.clone();
                let textify = stream_json_textify;
                let stdout_handle = tokio::spawn(async move {
                    if let Some(stdout) = stdout {
                        let mut lines = BufReader::new(stdout).lines();
                        let mut text_buf = StreamJsonTextBuffer::new();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = activity_tx_stdout.send(()).await;
                            if textify {
                                let emitted = process_stdout_line(&line, &mut text_buf);
                                for l in emitted {
                                    let _ = out_tx_stdout.send(OutputLine::Stdout(l)).await;
                                }
                            } else {
                                let _ = out_tx_stdout.send(OutputLine::Stdout(line)).await;
                            }
                        }
                        // Flush any remaining partial line in the buffer.
                        if textify {
                            if let Some(tail) = text_buf.finalize() {
                                if !tail.is_empty() {
                                    let _ = out_tx_stdout.send(OutputLine::Stdout(tail)).await;
                                }
                            }
                        }
                    }
                });

                // Spawn stderr reader.
                let out_tx_stderr = out_tx.clone();
                let activity_tx_stderr = activity_tx.clone();
                let stderr_handle = tokio::spawn(async move {
                    let mut buf = String::new();
                    if let Some(stderr) = stderr {
                        let mut lines = BufReader::new(stderr).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = activity_tx_stderr.send(()).await;
                            buf.push_str(&line);
                            buf.push('\n');
                            let _ = out_tx_stderr.send(OutputLine::Stderr(line)).await;
                        }
                    }
                    let _ = stderr_acc_tx.send(buf).await;
                });

                // Drop the extra activity sender so the channel closes naturally when
                // both reader tasks finish.
                drop(activity_tx);

                // --- Monitoring loop: activity reset, inactivity timeout, cancellation ---
                let mut inactivity_triggered = false;

                if inactivity_timeout_secs > 0 {
                    let mut last_activity = Instant::now();
                    let timeout_dur = Duration::from_secs(inactivity_timeout_secs);

                    'watch: loop {
                        let elapsed = last_activity.elapsed();
                        let remaining = if elapsed < timeout_dur {
                            timeout_dur - elapsed
                        } else {
                            Duration::from_secs(0)
                        };

                        tokio::select! {
                            biased;

                            // Scope shutdown reaches the runner task directly,
                            // so it is not silenced by a dropped handle.
                            _ = wait_for_scope_shutdown(&scope_cancel) => {
                                warn!(
                                    pid,
                                    op = ?operation_type_owned,
                                    change_id = ?change_id_owned,
                                    "Run command scope shutdown, terminating process group (pid={})", pid
                                );
                                if let Some(execution) = &scope_execution {
                                    execution.mark_cleaning();
                                }
                                let _ = managed_child
                                    .terminate_with_timeout(Duration::from_secs(5))
                                    .await;
                                let report = verify_owned_process_group(
                                    pid,
                                    cleanup_timeout_ms,
                                    operation_type_owned.as_deref(),
                                    change_id_owned.as_deref(),
                                    &out_tx,
                                )
                                .await;
                                pid_arc.store(0, Ordering::SeqCst);
                                if let Some(execution) = &scope_execution {
                                    execution.record_cleanup(pid, &report);
                                }
                                publish_cleanup_report(&mut cleanup_tx, report);
                                publish_termination(
                                    &mut termination_tx,
                                    CommandTermination::Cancelled,
                                );
                                let _ = status_tx.send(make_fail_status());
                                if let Some(execution) = &scope_execution {
                                    execution.finish();
                                }
                                return;
                            }

                            // Cancellation from StreamingChildHandle.terminate().
                            result = async {
                                match cancel_rx_opt {
                                    Some(ref mut rx) => rx.await,
                                    None => std::future::pending().await,
                                }
                            }, if !cancel_observed => {
                                cancel_observed = true;
                                cancel_rx_opt = None;
                                // A scoped runner treats handle-channel closure
                                // as cancellation too: losing the caller's
                                // handle is never permission to detach.
                                if result.is_ok() || scoped {
                                    warn!(
                                        pid,
                                        op = ?operation_type_owned,
                                        change_id = ?change_id_owned,
                                        handle_dropped = result.is_err(),
                                        "Streaming command cancelled, terminating process group (pid={})", pid
                                    );
                                    if let Some(execution) = &scope_execution {
                                        execution.mark_cleaning();
                                    }
                                    let _ = managed_child
                                        .terminate_with_timeout(Duration::from_secs(5))
                                        .await;
                                    // Reaping the leader is not proof that the
                                    // group is empty: verify quiescence before
                                    // the caller may touch the worktree.
                                    let report = verify_owned_process_group(
                                        pid,
                                        cleanup_timeout_ms,
                                        operation_type_owned.as_deref(),
                                        change_id_owned.as_deref(),
                                        &out_tx,
                                    )
                                    .await;
                                    pid_arc.store(0, Ordering::SeqCst);
                                    if let Some(execution) = &scope_execution {
                                        execution.record_cleanup(pid, &report);
                                    }
                                    publish_cleanup_report(&mut cleanup_tx, report);
                                    publish_termination(
                                        &mut termination_tx,
                                        CommandTermination::Cancelled,
                                    );
                                    let _ = status_tx.send(make_fail_status());
                                    if let Some(execution) = &scope_execution {
                                        execution.finish();
                                    }
                                    return;
                                }
                                // Err = handle was dropped without calling terminate() — continue.
                            }

                            // Absolute runtime limit reached. Evaluated in the
                            // same loop as the inactivity timer but from an
                            // independent deadline, so a command that keeps
                            // printing cannot postpone it.
                            _ = wait_for_runtime_limit(runtime_deadline) => {
                                warn!(
                                    pid,
                                    max_runtime_secs,
                                    op = ?operation_type_owned,
                                    change_id = ?change_id_owned,
                                    cwd = ?cwd_owned,
                                    "Absolute runtime limit reached, terminating process group \
                                     (pid={}, limit={}s)",
                                    pid, max_runtime_secs
                                );
                                let _ = out_tx
                                    .send(OutputLine::Stderr(runtime_limit_line(
                                        max_runtime_secs,
                                        operation_type_owned.as_deref(),
                                        change_id_owned.as_deref(),
                                        pid,
                                    )))
                                    .await;
                                if let Some(execution) = &scope_execution {
                                    execution.mark_cleaning();
                                }
                                let _ = managed_child
                                    .terminate_with_timeout(Duration::from_secs(5))
                                    .await;
                                let report = verify_owned_process_group(
                                    pid,
                                    cleanup_timeout_ms,
                                    operation_type_owned.as_deref(),
                                    change_id_owned.as_deref(),
                                    &out_tx,
                                )
                                .await;
                                pid_arc.store(0, Ordering::SeqCst);
                                if let Some(execution) = &scope_execution {
                                    execution.record_cleanup(pid, &report);
                                }
                                publish_cleanup_report(&mut cleanup_tx, report);
                                // Retry admission for this invocation closes
                                // here: the reason is published before the
                                // status, and the task returns without ever
                                // re-entering the retry loop.
                                publish_termination(
                                    &mut termination_tx,
                                    CommandTermination::RuntimeLimit,
                                );
                                let _ = status_tx.send(make_fail_status());
                                if let Some(execution) = &scope_execution {
                                    execution.finish();
                                }
                                return;
                            }

                            // Output activity resets the inactivity timer.
                            a = activity_rx.recv() => {
                                if a.is_some() {
                                    last_activity = Instant::now();
                                } else {
                                    // All readers finished.
                                    break 'watch;
                                }
                            }

                            // Inactivity timeout reached.
                            _ = tokio::time::sleep(remaining) => {
                                inactivity_triggered = true;
                                let last_activity_age_secs = last_activity.elapsed().as_secs();

                                // Get PGID for structured logging (Unix only).
                                #[cfg(unix)]
                                let pgid_opt: Option<u32> = {
                                    use nix::unistd::{getpgid, Pid};
                                    getpgid(Some(Pid::from_raw(pid as i32)))
                                        .ok()
                                        .map(|p| p.as_raw() as u32)
                                };
                                #[cfg(not(unix))]
                                let pgid_opt: Option<u32> = None;

                                warn!(
                                    pid,
                                    pgid = pgid_opt,
                                    timeout_secs = inactivity_timeout_secs,
                                    grace_secs = kill_grace_secs,
                                    last_activity_age_secs,
                                    op = ?operation_type_owned,
                                    change_id = ?change_id_owned,
                                    cwd = ?cwd_owned,
                                    "Inactivity timeout triggered: no output for {}s \
                                     (pid={}, pgid={:?}, timeout={}s, grace={}s, \
                                     last_activity_age={}s, op={:?}, change_id={:?}, cwd={:?})",
                                    last_activity_age_secs, pid, pgid_opt,
                                    inactivity_timeout_secs, kill_grace_secs,
                                    last_activity_age_secs,
                                    operation_type_owned, change_id_owned, cwd_owned
                                );

                                // Emit a user-facing message so callers see the timeout context.
                                let timeout_msg = format!(
                                    "Command terminated by inactivity timeout after {}s \
                                     (op={}, change_id={}, pid={}, last_activity_age={}s)",
                                    inactivity_timeout_secs,
                                    operation_type_owned.as_deref().unwrap_or("unknown"),
                                    change_id_owned.as_deref().unwrap_or("none"),
                                    pid,
                                    last_activity_age_secs,
                                );
                                let _ = out_tx.send(OutputLine::Stderr(timeout_msg)).await;

                                tokio::time::sleep(Duration::from_secs(kill_grace_secs)).await;
                                if managed_child.id().is_some() {
                                    warn!(
                                        pid,
                                        pgid = pgid_opt,
                                        signal = "SIGTERM",
                                        op = ?operation_type_owned,
                                        change_id = ?change_id_owned,
                                        "Grace period expired, sending SIGTERM to process group \
                                         (pid={}, pgid={:?})",
                                        pid, pgid_opt
                                    );
                                    match managed_child.terminate() {
                                        Ok(()) => {
                                            debug!(
                                                pid,
                                                signal = "SIGTERM",
                                                target_pgid = pgid_opt,
                                                "SIGTERM delivered to process group"
                                            );
                                        }
                                        Err(e) => {
                                            warn!(
                                                pid,
                                                signal = "SIGTERM",
                                                target_pid = pid,
                                                target_pgid = pgid_opt,
                                                errno = %e,
                                                op = ?operation_type_owned,
                                                change_id = ?change_id_owned,
                                                "SIGTERM failed for process group \
                                                 (pid={}, pgid={:?}): {}",
                                                pid, pgid_opt, e
                                            );
                                        }
                                    }
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    warn!(
                                        pid,
                                        pgid = pgid_opt,
                                        signal = "SIGKILL",
                                        op = ?operation_type_owned,
                                        change_id = ?change_id_owned,
                                        "Sending SIGKILL to process group (pid={}, pgid={:?})",
                                        pid, pgid_opt
                                    );
                                    match managed_child.force_kill().await {
                                        Ok(()) => {
                                            debug!(
                                                pid,
                                                signal = "SIGKILL",
                                                target_pgid = pgid_opt,
                                                "SIGKILL delivered to process group"
                                            );
                                        }
                                        Err(e) => {
                                            warn!(
                                                pid,
                                                signal = "SIGKILL",
                                                target_pid = pid,
                                                target_pgid = pgid_opt,
                                                errno = %e,
                                                op = ?operation_type_owned,
                                                change_id = ?change_id_owned,
                                                "SIGKILL failed for process group \
                                                 (pid={}, pgid={:?}): {}",
                                                pid, pgid_opt, e
                                            );
                                        }
                                    }
                                }
                                break 'watch;
                            }
                        }
                    }
                } else {
                    // No inactivity timeout — only watch for cancel and reader completion.
                    'watch_no_timeout: loop {
                        tokio::select! {
                            biased;

                            _ = wait_for_scope_shutdown(&scope_cancel) => {
                                warn!(
                                    pid,
                                    op = ?operation_type_owned,
                                    change_id = ?change_id_owned,
                                    "Run command scope shutdown, terminating process group (pid={})", pid
                                );
                                if let Some(execution) = &scope_execution {
                                    execution.mark_cleaning();
                                }
                                let _ = managed_child
                                    .terminate_with_timeout(Duration::from_secs(5))
                                    .await;
                                let report = verify_owned_process_group(
                                    pid,
                                    cleanup_timeout_ms,
                                    operation_type_owned.as_deref(),
                                    change_id_owned.as_deref(),
                                    &out_tx,
                                )
                                .await;
                                pid_arc.store(0, Ordering::SeqCst);
                                if let Some(execution) = &scope_execution {
                                    execution.record_cleanup(pid, &report);
                                }
                                publish_cleanup_report(&mut cleanup_tx, report);
                                publish_termination(
                                    &mut termination_tx,
                                    CommandTermination::Cancelled,
                                );
                                let _ = status_tx.send(make_fail_status());
                                if let Some(execution) = &scope_execution {
                                    execution.finish();
                                }
                                return;
                            }

                            result = async {
                                match cancel_rx_opt {
                                    Some(ref mut rx) => rx.await,
                                    None => std::future::pending().await,
                                }
                            }, if !cancel_observed => {
                                cancel_observed = true;
                                cancel_rx_opt = None;
                                if result.is_ok() || scoped {
                                    warn!(
                                        pid,
                                        op = ?operation_type_owned,
                                        change_id = ?change_id_owned,
                                        handle_dropped = result.is_err(),
                                        "Streaming command cancelled, terminating process group (pid={})", pid
                                    );
                                    if let Some(execution) = &scope_execution {
                                        execution.mark_cleaning();
                                    }
                                    let _ = managed_child
                                        .terminate_with_timeout(Duration::from_secs(5))
                                        .await;
                                    let report = verify_owned_process_group(
                                        pid,
                                        cleanup_timeout_ms,
                                        operation_type_owned.as_deref(),
                                        change_id_owned.as_deref(),
                                        &out_tx,
                                    )
                                    .await;
                                    pid_arc.store(0, Ordering::SeqCst);
                                    if let Some(execution) = &scope_execution {
                                        execution.record_cleanup(pid, &report);
                                    }
                                    publish_cleanup_report(&mut cleanup_tx, report);
                                    publish_termination(
                                        &mut termination_tx,
                                        CommandTermination::Cancelled,
                                    );
                                    let _ = status_tx.send(make_fail_status());
                                    if let Some(execution) = &scope_execution {
                                        execution.finish();
                                    }
                                    return;
                                }
                            }

                            // The absolute deadline is enforced whether or not
                            // the inactivity timeout is configured: they are
                            // independent limits.
                            _ = wait_for_runtime_limit(runtime_deadline) => {
                                warn!(
                                    pid,
                                    max_runtime_secs,
                                    op = ?operation_type_owned,
                                    change_id = ?change_id_owned,
                                    cwd = ?cwd_owned,
                                    "Absolute runtime limit reached, terminating process group \
                                     (pid={}, limit={}s)",
                                    pid, max_runtime_secs
                                );
                                let _ = out_tx
                                    .send(OutputLine::Stderr(runtime_limit_line(
                                        max_runtime_secs,
                                        operation_type_owned.as_deref(),
                                        change_id_owned.as_deref(),
                                        pid,
                                    )))
                                    .await;
                                if let Some(execution) = &scope_execution {
                                    execution.mark_cleaning();
                                }
                                let _ = managed_child
                                    .terminate_with_timeout(Duration::from_secs(5))
                                    .await;
                                let report = verify_owned_process_group(
                                    pid,
                                    cleanup_timeout_ms,
                                    operation_type_owned.as_deref(),
                                    change_id_owned.as_deref(),
                                    &out_tx,
                                )
                                .await;
                                pid_arc.store(0, Ordering::SeqCst);
                                if let Some(execution) = &scope_execution {
                                    execution.record_cleanup(pid, &report);
                                }
                                publish_cleanup_report(&mut cleanup_tx, report);
                                publish_termination(
                                    &mut termination_tx,
                                    CommandTermination::RuntimeLimit,
                                );
                                let _ = status_tx.send(make_fail_status());
                                if let Some(execution) = &scope_execution {
                                    execution.finish();
                                }
                                return;
                            }

                            a = activity_rx.recv() => {
                                if a.is_none() {
                                    break 'watch_no_timeout;
                                }
                            }
                        }
                    }
                }

                // Wait for readers to finish before collecting status.
                let _ = stdout_handle.await;
                let _ = stderr_handle.await;

                let stderr_collected = stderr_acc_rx.recv().await.unwrap_or_default();

                // Collect the child's exit status.
                let status = match managed_child.wait().await {
                    Ok(s) => s,
                    Err(e) => {
                        error!(
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            "Failed to wait for child process: {}", e
                        );
                        break 'retry;
                    }
                };

                pid_arc.store(0, Ordering::SeqCst);
                if let Some(execution) = &scope_execution {
                    execution.mark_cleaning();
                }

                // Strict post-completion cleanup: sweep the process group after every
                // command outcome (success, failure, inactivity timeout) to ensure no
                // background processes spawned by the agent command outlive it.
                let attempt_cleanup = if strict_process_cleanup {
                    verify_owned_process_group(
                        pid,
                        cleanup_timeout_ms,
                        operation_type_owned.as_deref(),
                        change_id_owned.as_deref(),
                        &out_tx,
                    )
                    .await
                } else {
                    ProcessGroupCleanupReport::not_applicable(
                        "strict post-completion process-group cleanup is disabled",
                    )
                };
                if let Some(execution) = &scope_execution {
                    execution.record_cleanup(pid, &attempt_cleanup);
                }
                // An earlier attempt that could not be proven quiescent stays
                // the published verdict: its survivors may still be running.
                if cleanup_report.is_confirmed() {
                    cleanup_report = attempt_cleanup;
                }

                // Handle inactivity-timeout exits with dedicated retry policy.
                if inactivity_triggered {
                    if inactivity_timeout_max_retries > 0
                        && inactivity_retries_used < inactivity_timeout_max_retries
                        // Shutdown is rechecked immediately before the retry
                        // delay, not only at the top of the loop, so a scope
                        // that closed during this attempt never buys a sleep.
                        && !scope_execution
                            .as_ref()
                            .is_some_and(ScopeExecution::is_shutdown)
                    {
                        inactivity_retries_used += 1;
                        warn!(
                            inactivity_retries_used,
                            inactivity_timeout_max_retries,
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            "Inactivity timeout retry {}/{}, retrying in {}ms",
                            inactivity_retries_used, inactivity_timeout_max_retries,
                            retry_delay_ms
                        );
                        let _ = managed_child.terminate();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        let _ = managed_child.force_kill().await;
                        let retry_msg = format!(
                            "[Retry {}/{}] Inactivity timeout, retrying in {}ms \
                             (op={}, change_id={})",
                            inactivity_retries_used,
                            inactivity_timeout_max_retries,
                            retry_delay_ms,
                            operation_type_owned.as_deref().unwrap_or("unknown"),
                            change_id_owned.as_deref().unwrap_or("none"),
                        );
                        let _ = out_tx.send(OutputLine::Stderr(retry_msg)).await;
                        tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
                        continue 'retry;
                    }

                    // Exhausted inactivity retries (or retries disabled): emit final message.
                    if inactivity_timeout_max_retries > 0 {
                        let exhausted_msg = format!(
                            "Inactivity timeout: exhausted all {} retries \
                             (op={}, change_id={})",
                            inactivity_timeout_max_retries,
                            operation_type_owned.as_deref().unwrap_or("unknown"),
                            change_id_owned.as_deref().unwrap_or("none"),
                        );
                        let _ = out_tx.send(OutputLine::Stderr(exhausted_msg)).await;
                    }

                    // Do not fall through to the crash/pattern retry check.
                    final_exit_status = Some(status);
                    final_termination = CommandTermination::InactivityTimeout;
                    break 'retry;
                }

                // Check whether a retry is warranted for non-inactivity exits.
                if !status.success() {
                    let exit_code = status.code().unwrap_or(-1);
                    let duration = start_time.elapsed();

                    // Shutdown suppresses the ordinary retry branch as well: an
                    // observed closure is checked before the delay and again at
                    // final spawn admission.
                    if command_queue.should_retry(attempt, duration, &stderr_collected, exit_code)
                        && !scope_execution
                            .as_ref()
                            .is_some_and(ScopeExecution::is_shutdown)
                    {
                        warn!(
                            attempt,
                            max_retries,
                            exit_code,
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            "Retryable error detected, retrying in {}ms", retry_delay_ms
                        );
                        // Enforce full process-group cleanup before the next attempt:
                        //   1. SIGTERM → cooperative shutdown of all PGID members.
                        //   2. 100ms grace period → let SIGTERM-responsive processes exit.
                        //   3. SIGKILL → force-kill any survivors (e.g. SIGTERM-immune loops).
                        // managed_child.wait() has already reaped `sh`, but pipeline siblings
                        // sharing the same PGID may still be running. terminate() alone is
                        // best-effort; force_kill() after the grace window ensures they die.
                        let _ = managed_child.terminate();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        let _ = managed_child.force_kill().await;
                        let retry_msg = format!(
                            "[Retry {}/{}] Command crashed, retrying in {}ms...",
                            attempt, max_retries, retry_delay_ms
                        );
                        let _ = out_tx.send(OutputLine::Stderr(retry_msg)).await;
                        tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
                        continue 'retry;
                    }
                }

                final_exit_status = Some(status);
                final_termination = CommandTermination::Exited;
                break 'retry;
            }

            // Send final exit status (failure if we exited the retry loop without one).
            // An unconfirmed process-group cleanup can never be published as a
            // successful completion: descendants may still be mutating the
            // worktree the caller is about to finalize.
            let mut final_status = final_exit_status.unwrap_or_else(make_fail_status);
            if !cleanup_report.is_confirmed() && final_status.success() {
                warn!(
                    op = ?operation_type_owned,
                    change_id = ?change_id_owned,
                    "Downgrading successful command status: {}",
                    cleanup_report.diagnostics()
                );
                final_status = make_fail_status();
            }
            publish_cleanup_report(&mut cleanup_tx, cleanup_report);
            publish_termination(&mut termination_tx, final_termination);
            let _ = status_tx.send(final_status);
            // The registration is released only here: the runner task has
            // ended, and it disappears from the scope only when every owned
            // identity was already proven quiescent.
            if let Some(execution) = &scope_execution {
                execution.finish();
            }
            // Dropping out_tx closes the output channel, signalling end-of-output to callers.
        });

        let handle = StreamingChildHandle::new(
            cancel_tx,
            current_pid,
            status_rx,
            cleanup_rx,
            termination_rx,
        );
        Ok((handle, out_rx))
    }

    /// Execute a command with streaming output and stagger delay.
    ///
    /// This is the core execution method used by all AI-driven commands.
    /// It spawns the command through CommandQueue (with stagger), then
    /// streams stdout/stderr to an mpsc channel.
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute (will be run via `sh -c`)
    /// * `cwd` - Optional working directory (for worktree execution)
    ///
    /// # Returns
    ///
    /// A tuple of (ManagedChild, Receiver<OutputLine>) for process control and output streaming
    #[allow(dead_code)] // Infrastructure ready, integration pending (tasks 3.2, 3.3, 4.1-4.3)
    pub async fn execute_streaming(
        &self,
        command: &str,
        cwd: Option<&Path>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        debug!(
            module = module_path!(),
            "Executing shell command with stagger: sh -c {} (cwd: {:?})", command, cwd
        );

        let child = self
            .command_queue
            .execute_with_stagger(move || {
                let mut cmd = Command::new("sh");
                cmd.arg("-c")
                    .arg(command)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                if let Some(dir) = cwd {
                    cmd.current_dir(dir);
                }
                cmd
            })
            .await?;

        // Wrap in ManagedChild for proper cleanup
        let mut managed = ManagedChild::new(child)?;

        // Take stdout/stderr from the child field
        let stdout = managed.child.stdout.take().ok_or_else(|| {
            OrchestratorError::AgentCommand(format!(
                "Failed to capture stdout for command '{}' (cwd: {:?})",
                command, cwd
            ))
        })?;
        let stderr = managed.child.stderr.take().ok_or_else(|| {
            OrchestratorError::AgentCommand(format!(
                "Failed to capture stderr for command '{}' (cwd: {:?})",
                command, cwd
            ))
        })?;

        // Create channel for output streaming
        let (tx, rx) = mpsc::channel(1024);

        // Spawn stdout reader
        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx_stdout.send(OutputLine::Stdout(line)).await.is_err() {
                    break;
                }
            }
        });

        // Spawn stderr reader
        let tx_stderr = tx;
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx_stderr.send(OutputLine::Stderr(line)).await.is_err() {
                    break;
                }
            }
        });

        Ok((managed, rx))
    }
}

/// Longest launch-error cause carried on the output channel.
///
/// The cause is operational evidence for the next agent prompt and for terminal
/// diagnostics, so it is bounded exactly like every other tail this workflow
/// carries.
const MAX_LAUNCH_ERROR_CHARS: usize = 400;

/// One bounded stderr line describing why a command never started.
///
/// A command that fails before it runs produces no output of its own and only a
/// synthetic failure status, so this line is the only evidence the caller can
/// collect. It is emitted on the same output channel every operation already
/// drains, which is what carries it into Apply history, the Acceptance command
/// diagnostic, and the cleanup-review diagnosis.
fn launch_failure_line(
    stage: &str,
    operation_type: Option<&str>,
    change_id: Option<&str>,
    attempt: u32,
    cause: &str,
) -> String {
    let single_line = cause.split_whitespace().collect::<Vec<_>>().join(" ");
    let bounded = match single_line.char_indices().nth(MAX_LAUNCH_ERROR_CHARS) {
        Some((idx, _)) => format!("{}...", &single_line[..idx]),
        None => single_line,
    };
    format!(
        "Command launch failed (stage={}, op={}, change_id={}, attempt={}): {}",
        stage,
        operation_type.unwrap_or("unknown"),
        change_id.unwrap_or("none"),
        attempt,
        bounded
    )
}

/// One bounded stderr line describing a command refused by scope shutdown.
///
/// `stage` distinguishes the two closure points a command can hit: `spawn` is
/// the final serialized admission check, `retry` is a later attempt that was
/// never admitted at all.
fn shutdown_refusal_line(
    stage: &str,
    operation_type: Option<&str>,
    change_id: Option<&str>,
    attempt: u32,
) -> String {
    format!(
        "Command refused by run command scope shutdown (stage={}, op={}, change_id={}, attempt={})",
        stage,
        operation_type.unwrap_or("unknown"),
        change_id.unwrap_or("none"),
        attempt
    )
}

/// Result handed back when the scope refuses an execution before it starts.
///
/// The caller gets the same shape it always gets — a handle plus an output
/// receiver — so no operation needs a shutdown-specific code path: it drains
/// one diagnostic line and observes a failure status.
fn refused_after_shutdown(
    reason: &str,
    operation_type: Option<&str>,
    change_id: Option<&str>,
) -> (StreamingChildHandle, mpsc::Receiver<OutputLine>) {
    let (out_tx, out_rx) = mpsc::channel::<OutputLine>(1);
    let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let (status_tx, status_rx) = tokio::sync::oneshot::channel::<std::process::ExitStatus>();
    let (cleanup_tx, cleanup_rx) = tokio::sync::oneshot::channel::<ProcessGroupCleanupReport>();
    let (termination_tx, termination_rx) = tokio::sync::oneshot::channel::<CommandTermination>();

    let _ = out_tx.try_send(OutputLine::Stderr(shutdown_refusal_line(
        "admission",
        operation_type,
        change_id,
        1,
    )));
    let _ = cleanup_tx.send(ProcessGroupCleanupReport::not_applicable(reason));
    let _ = termination_tx.send(CommandTermination::NotStarted);
    let _ = status_tx.send(make_fail_status());
    drop(out_tx);

    (
        StreamingChildHandle::new(
            cancel_tx,
            Arc::new(AtomicU32::new(0)),
            status_rx,
            cleanup_rx,
            termination_rx,
        ),
        out_rx,
    )
}

/// Await scope shutdown, or never resolve for an unscoped runner.
async fn wait_for_scope_shutdown(token: &Option<tokio_util::sync::CancellationToken>) {
    match token {
        Some(token) => token.cancelled().await,
        None => std::future::pending().await,
    }
}

/// Await the absolute runtime deadline, or never resolve when it is disabled.
///
/// The deadline is absolute rather than a per-iteration duration, so re-entering
/// the monitoring loop after each output line cannot push it further away.
async fn wait_for_runtime_limit(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// One bounded stderr line describing an invocation stopped by its runtime limit.
///
/// It names the limit rather than the elapsed time so an operator can act on the
/// configuration knob directly.
fn runtime_limit_line(
    max_runtime_secs: u64,
    operation_type: Option<&str>,
    change_id: Option<&str>,
    pid: u32,
) -> String {
    format!(
        "Command terminated by absolute runtime limit after {}s \
         (op={}, change_id={}, pid={}); this invocation is not retried",
        max_runtime_secs,
        operation_type.unwrap_or("unknown"),
        change_id.unwrap_or("none"),
        pid
    )
}

/// Publishes the terminal reason for this invocation exactly once.
fn publish_termination(
    termination_tx: &mut Option<tokio::sync::oneshot::Sender<CommandTermination>>,
    termination: CommandTermination,
) {
    if let Some(tx) = termination_tx.take() {
        let _ = tx.send(termination);
    }
}

/// Runs bounded cleanup on the owned process group and returns typed evidence.
///
/// The report is the only accepted proof that no owned descendant can still be
/// touching the managed worktree; unconfirmed cleanup is surfaced on the output
/// stream so operators see the same diagnostics the caller acts on.
async fn verify_owned_process_group(
    pid: u32,
    cleanup_timeout_ms: u64,
    op: Option<&str>,
    change_id: Option<&str>,
    out_tx: &mpsc::Sender<OutputLine>,
) -> ProcessGroupCleanupReport {
    if pid == 0 {
        return ProcessGroupCleanupReport::not_applicable(
            "no process group id was available for cleanup",
        );
    }

    let report = cleanup_process_group_verified(
        pid,
        DEFAULT_PROCESS_GROUP_SIGTERM_GRACE_MS,
        cleanup_timeout_ms,
        op,
        change_id,
    )
    .await;

    if !report.is_confirmed() {
        let _ = out_tx.send(OutputLine::Stderr(report.diagnostics())).await;
    }

    report
}

/// Publishes cleanup evidence exactly once for this execution.
fn publish_cleanup_report(
    cleanup_tx: &mut Option<tokio::sync::oneshot::Sender<ProcessGroupCleanupReport>>,
    report: ProcessGroupCleanupReport,
) {
    if let Some(tx) = cleanup_tx.take() {
        let _ = tx.send(report);
    }
}

/// Construct a synthetic failure [`std::process::ExitStatus`] for error paths.
fn make_fail_status() -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(1)
    }
    #[cfg(not(unix))]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::defaults::*;

    /// The absolute deadline is a decision about *when*, so it is verified with
    /// paused time rather than by waiting: nothing here spawns a process, and no
    /// assertion depends on how long the test actually took.
    mod absolute_runtime_deadline {
        use super::*;

        /// A disabled limit is a future that never resolves, which is what lets
        /// it sit in the same `select!` as every other lifecycle arm without
        /// needing a separate code path for "no deadline".
        #[tokio::test(start_paused = true)]
        async fn a_disabled_limit_never_fires() {
            // A full simulated day passes and the arm still has not resolved.
            let elapsed =
                tokio::time::timeout(Duration::from_secs(86_400), wait_for_runtime_limit(None))
                    .await;
            assert!(
                elapsed.is_err(),
                "`0` must disable the deadline entirely, not defer it"
            );
        }

        /// Re-entering the monitoring loop cannot postpone the deadline: it is
        /// an absolute instant, so awaiting it repeatedly still resolves at the
        /// same moment. This is precisely why output activity cannot extend it.
        #[tokio::test(start_paused = true)]
        async fn an_absolute_deadline_is_not_pushed_back_by_re_entry() {
            let start = tokio::time::Instant::now();
            let deadline = Some(start + Duration::from_secs(60));

            // Ten monitoring-loop iterations, each one abandoning the wait after
            // five seconds the way an output line does.
            for _ in 0..10 {
                let _ =
                    tokio::time::timeout(Duration::from_secs(5), wait_for_runtime_limit(deadline))
                        .await;
            }

            wait_for_runtime_limit(deadline).await;
            assert_eq!(
                start.elapsed(),
                Duration::from_secs(60),
                "the deadline must fire 60s after it was set, regardless of how \
                 many times the loop re-awaited it"
            );
        }

        /// The runner reads the limit from its own queue config, so the
        /// configured value is what an invocation is actually bounded by.
        #[test]
        fn the_runner_carries_the_configured_limit() {
            let config = OrchestratorConfig {
                command_max_runtime_secs: Some(120),
                ..OrchestratorConfig::default()
            };
            let runner =
                AiCommandRunner::from_orchestrator_config(&config, Arc::new(Mutex::new(None)));
            assert_eq!(runner.queue_config().max_runtime_secs, 120);

            let disabled = OrchestratorConfig {
                command_max_runtime_secs: Some(0),
                ..OrchestratorConfig::default()
            };
            let runner =
                AiCommandRunner::from_orchestrator_config(&disabled, Arc::new(Mutex::new(None)));
            assert_eq!(
                runner.queue_config().max_runtime_secs,
                0,
                "an explicit disable must survive into the runner"
            );
        }

        /// The operator-facing line names the limit rather than the elapsed
        /// time, so the diagnostic points at the knob that has to change.
        #[test]
        fn the_runtime_limit_line_names_the_limit_and_the_no_retry_decision() {
            let line = runtime_limit_line(3600, Some("apply"), Some("change-a"), 4242);
            assert!(line.contains("3600s"), "the configured limit: {line}");
            assert!(line.contains("op=apply"), "the operation: {line}");
            assert!(line.contains("change_id=change-a"), "the change: {line}");
            assert!(line.contains("pid=4242"), "the owned identity: {line}");
            assert!(
                line.contains("not retried"),
                "the retry decision must be visible to the operator: {line}"
            );
        }
    }

    /// A termination reason exists to answer one question: may this invocation
    /// be attempted again? A boundary decision may not; a command that ended on
    /// its own may.
    mod command_termination {
        use crate::process_manager::CommandTermination;

        #[test]
        fn deliberate_terminations_never_permit_a_retry() {
            for termination in [
                CommandTermination::RuntimeLimit,
                CommandTermination::Cancelled,
                CommandTermination::NotStarted,
            ] {
                assert!(
                    !termination.permits_retry(),
                    "{} must not admit a retry",
                    termination.as_str()
                );
            }
        }

        #[test]
        fn command_owned_endings_stay_retryable() {
            for termination in [
                CommandTermination::Exited,
                CommandTermination::InactivityTimeout,
            ] {
                assert!(
                    termination.permits_retry(),
                    "{} is a command outcome the existing retry policy owns",
                    termination.as_str()
                );
            }
        }

        /// Only the runtime limit answers to `is_runtime_limit`, so an operator
        /// stop and a runaway command stay distinguishable in reporting.
        #[test]
        fn only_the_runtime_limit_identifies_as_one() {
            assert!(CommandTermination::RuntimeLimit.is_runtime_limit());
            for other in [
                CommandTermination::Exited,
                CommandTermination::InactivityTimeout,
                CommandTermination::Cancelled,
                CommandTermination::NotStarted,
            ] {
                assert!(!other.is_runtime_limit(), "{}", other.as_str());
            }
        }
    }

    #[test]
    fn configured_constructor_preserves_runner_settings() {
        let config = OrchestratorConfig {
            command_queue_stagger_delay_ms: Some(41),
            stream_json_textify: Some(false),
            command_strict_process_cleanup: Some(false),
            envs: Some(HashMap::from([(
                "CFLX_TEST_ENV".to_string(),
                "configured-value".to_string(),
            )])),
            ..OrchestratorConfig::default()
        };

        let shared_state = Arc::new(Mutex::new(None));
        let runner = AiCommandRunner::from_orchestrator_config(&config, shared_state.clone());

        assert!(Arc::ptr_eq(&runner.shared_stagger_state(), &shared_state));
        assert_eq!(runner.queue_config().stagger_delay_ms, 41);
        assert!(!runner.stream_json_textify);
        assert!(!runner.strict_process_cleanup);
        assert_eq!(
            runner.command_envs,
            HashMap::from([("CFLX_TEST_ENV".to_string(), "configured-value".to_string())])
        );
    }

    #[tokio::test]
    async fn test_shared_stagger_state() {
        let shared_state = Arc::new(Mutex::new(None));

        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 100,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };

        let runner1 = AiCommandRunner::new(config.clone(), shared_state.clone());
        let runner2 = AiCommandRunner::new(config.clone(), shared_state.clone());

        // Execute first command
        let start = Instant::now();
        let (mut child1, _rx1) = runner1.execute_streaming("echo test1", None).await.unwrap();
        let _ = child1.wait().await;

        // Execute second command - should wait for stagger
        let (mut child2, _rx2) = runner2.execute_streaming("echo test2", None).await.unwrap();
        let elapsed = start.elapsed();
        let _ = child2.wait().await;

        // Second command should have waited at least 100ms
        assert!(
            elapsed.as_millis() >= 90,
            "Stagger delay not applied: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_streaming_with_retry_applies_configured_envs_without_logging_values() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let mut runner = AiCommandRunner::new(config, shared_state);
        runner.set_command_envs(HashMap::from([(
            "CFLX_TEST_AGENT_ENV".to_string(),
            "secret-value".to_string(),
        )]));

        let command = "printf %s \"$CFLX_TEST_AGENT_ENV\"";
        assert!(!command.contains("secret-value"));
        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(command, None, Some("test"), None)
            .await
            .unwrap();

        let mut stdout = String::new();
        while let Some(line) = rx.recv().await {
            if let OutputLine::Stdout(s) = line {
                stdout.push_str(&s);
            }
        }
        let status = handle.wait().await.unwrap();
        assert!(status.success());
        assert_eq!(stdout, "secret-value");
    }

    /// Verify that execute_streaming_with_retry returns a real child PID (not 0).
    #[tokio::test]
    async fn test_streaming_with_retry_real_pid() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry("sleep 0.2", None, Some("test"), None)
            .await
            .unwrap();

        // Give the background task time to spawn the real child.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // The handle must expose the PID of the real child, not 0.
        let pid = handle.id();
        assert!(pid.is_some(), "Expected a real PID, got None");
        assert!(pid.unwrap() > 0, "Expected PID > 0");

        // Drain output and wait.
        while rx.recv().await.is_some() {}
        let _ = handle.wait().await;
    }

    /// Verify that retry-attempt cleanup does not leave leaked processes.
    ///
    /// Spawns a command that starts a lingering background subprocess (`sleep 30`) then
    /// exits with failure, triggering a retry. After all retries complete the test asserts
    /// that the process group from attempt 1 has no surviving members.
    ///
    /// This is the regression test for the "Streaming retry does not leak processes across
    /// attempts" scenario from Acceptance #2 Follow-up.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_streaming_retry_no_leaked_processes() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 2,
            retry_delay_ms: 300,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 5, // treat short exits as retryable
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 5,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        // Each attempt writes its sh PID (= PGID because configure_process_group makes sh
        // the group leader) to a temp file, spawns a lingering `sleep 30` that shares the
        // PGID, then exits with failure so a retry is triggered.
        let pgid_file =
            std::env::temp_dir().join(format!("retry_leak_pgid_{}.txt", std::process::id()));
        let pgid_path = pgid_file.display().to_string();
        // Redirect sleep's I/O away from the inherited pipes so the stdout/stderr readers
        // reach EOF immediately when sh exits (instead of waiting 30 s for sleep to end).
        // sleep 30 stays in the same PGID as sh and is the "orphan candidate" we verify
        // is killed by the retry cleanup before the next attempt begins.
        let cmd = format!(
            "echo $$ >> {path}; sleep 30 >/dev/null 2>&1 </dev/null & exit 1",
            path = pgid_path
        );

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(&cmd, None, Some("test"), None)
            .await
            .unwrap();

        // Drain output to avoid backpressure stalling the background task.
        while rx.recv().await.is_some() {}
        let _ = handle.wait().await;

        // Read PGIDs recorded by the attempts.
        assert!(
            pgid_file.exists(),
            "PGID file should have been created by at least one attempt"
        );
        let content = std::fs::read_to_string(&pgid_file).unwrap_or_default();
        let _ = std::fs::remove_file(&pgid_file);

        let pgids: Vec<i32> = content
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect();
        assert!(
            pgids.len() >= 2,
            "Expected PGIDs from at least 2 attempts (attempt 1 + retry), got: {:?}",
            pgids
        );

        // Give a brief moment for OS signal delivery to fully propagate.
        tokio::time::sleep(Duration::from_millis(150)).await;

        // The retry cleanup fires only between attempts (before `continue 'retry`).
        // The *final* attempt has no subsequent retry, so its background process is not
        // cleaned up by the retry logic. We verify all non-final attempt PGIDs are dead.
        //
        // `killpg(pgid, 0)` returns 0 if any process in the group is alive (ESRCH otherwise).
        let non_final_count = pgids.len() - 1;
        for pgid in &pgids[..non_final_count] {
            let result = unsafe { libc::killpg(*pgid, 0) };
            assert_eq!(
                result, -1,
                "Process group {} (non-final attempt) should be dead after retry cleanup, \
                 but it still has live members (killpg returned 0)",
                pgid
            );
        }

        // Clean up the final attempt's background sleep so the test does not leak
        // a `sleep 30` process into the test runner's process table.
        if let Some(last_pgid) = pgids.last() {
            unsafe {
                libc::killpg(*last_pgid, libc::SIGKILL);
            }
        }
    }

    /// Invocation-scoped command ownership.
    ///
    /// These drive the real runner control flow — a real scope, a real retry
    /// loop, a real `Command::spawn` — because the defect they pin is precisely
    /// that a command could still start, or still retry, after the run that
    /// owned it had already begun shutting down.
    mod run_command_scope {
        use super::*;

        fn scoped_runner(
            config: CommandQueueConfig,
            stagger: SharedStaggerState,
        ) -> (AiCommandRunner, RunCommandScope) {
            let scope = RunCommandScope::new();
            let mut runner = AiCommandRunner::new(config, stagger);
            runner.set_run_command_scope(scope.clone());
            (runner, scope)
        }

        fn marker_path(label: &str) -> std::path::PathBuf {
            std::env::temp_dir().join(format!(
                "cflx_scope_{}_{}_{}.txt",
                label,
                std::process::id(),
                Instant::now().elapsed().as_nanos()
            ))
        }

        /// Closing the scope while a command is parked at the stagger delay
        /// must refuse the spawn itself, not merely the next retry.
        ///
        /// The marker file is the proof: it exists only if the command body ran.
        #[cfg(unix)]
        #[tokio::test]
        async fn run_command_scope_refuses_spawn_after_shutdown() {
            let marker = marker_path("refuse");
            let _ = std::fs::remove_file(&marker);

            // Pre-armed stagger timestamp: the runner parks in the stagger wait
            // with its final admission still pending, which is exactly the
            // window a check taken *before* the delay would miss.
            let stagger: SharedStaggerState = Arc::new(Mutex::new(Some(Instant::now())));
            let config = CommandQueueConfig {
                acceptance_max_runtime_secs:
                    crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
                stagger_delay_ms: 250,
                max_retries: 3,
                retry_delay_ms: 0,
                retry_error_patterns: vec![],
                retry_if_duration_under_secs: 0,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 1,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
                max_runtime_secs: 0,
            };
            let (runner, scope) = scoped_runner(config, stagger);

            let command = format!("touch {}", marker.display());
            let (mut handle, mut rx) = runner
                .execute_streaming_with_retry(&command, None, Some("apply"), Some("change-a"))
                .await
                .expect("the call itself succeeds; the spawn is what is refused");

            tokio::time::sleep(Duration::from_millis(60)).await;
            scope.close();

            let mut stderr_lines = Vec::new();
            while let Some(line) = rx.recv().await {
                if let OutputLine::Stderr(s) = line {
                    stderr_lines.push(s);
                }
            }
            let status = handle.wait().await.expect("a final status is reported");

            assert!(
                !marker.exists(),
                "the command body must never run after admission closed"
            );
            assert!(!status.success(), "a refused command is not a success");
            assert!(
                stderr_lines.iter().any(|line| line
                    .contains("refused by run command scope shutdown")
                    && line.contains("op=apply")
                    && line.contains("change_id=change-a")),
                "the refusal must be legible to the caller: {stderr_lines:?}"
            );
            assert_eq!(
                scope.active_executions(),
                0,
                "a command that never spawned holds no barrier open"
            );
            let _ = std::fs::remove_file(&marker);
        }

        /// A scope that closes while an attempt sits in its retry delay must
        /// stop the attempt counter where it is.
        #[cfg(unix)]
        #[tokio::test]
        async fn run_command_scope_suppresses_retry_after_shutdown() {
            let attempts = marker_path("retry");
            let _ = std::fs::remove_file(&attempts);

            let config = CommandQueueConfig {
                acceptance_max_runtime_secs:
                    crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
                stagger_delay_ms: 0,
                max_retries: 5,
                retry_delay_ms: 300,
                retry_error_patterns: vec![],
                // Treat the fast failing exit as retryable, so an unscoped run
                // would keep going.
                retry_if_duration_under_secs: 30,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 1,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
                max_runtime_secs: 0,
            };
            let (runner, scope) = scoped_runner(config, Arc::new(Mutex::new(None)));

            let command = format!("echo attempt >> {}; exit 1", attempts.display());
            let (mut handle, mut rx) = runner
                .execute_streaming_with_retry(&command, None, Some("apply"), Some("change-a"))
                .await
                .expect("the first attempt is admitted");

            // Attempt 1 has failed and is sleeping out its retry delay by now.
            tokio::time::sleep(Duration::from_millis(120)).await;
            scope.close();

            let mut stderr_lines = Vec::new();
            while let Some(line) = rx.recv().await {
                if let OutputLine::Stderr(s) = line {
                    stderr_lines.push(s);
                }
            }
            let _ = handle.wait().await;

            let recorded = std::fs::read_to_string(&attempts).unwrap_or_default();
            let _ = std::fs::remove_file(&attempts);
            assert_eq!(
                recorded.lines().count(),
                1,
                "no attempt may start after shutdown, got: {recorded:?}"
            );
            assert!(
                stderr_lines.iter().any(|line| line.contains("stage=retry")
                    && line.contains("refused by run command scope shutdown")),
                "the suppressed retry must say why: {stderr_lines:?}"
            );
            assert_eq!(scope.active_executions(), 0);
        }

        /// A registered execution keeps the barrier open, and multiple
        /// registrations are awaited concurrently under one deadline rather
        /// than one after another.
        #[tokio::test]
        async fn run_command_scope_awaits_registrations_concurrently() {
            let scope = RunCommandScope::new();
            let held: Vec<_> = ["alpha", "beta", "gamma"]
                .iter()
                .map(|change| {
                    scope
                        .register_for_test("apply", Some(change))
                        .expect("an open scope admits registrations")
                })
                .collect();
            assert_eq!(scope.active_executions(), 3);

            let releasing = scope.clone();
            tokio::spawn(async move {
                for (index, registration) in held.into_iter().enumerate() {
                    tokio::time::sleep(Duration::from_millis(40 * (index as u64 + 1))).await;
                    registration.release_confirmed();
                }
                let _ = releasing;
            });

            let started = Instant::now();
            let cleanup = scope.shutdown(Duration::from_secs(2)).await;
            let elapsed = started.elapsed();

            assert!(
                cleanup.is_quiescent(),
                "every registration reported quiescence: {}",
                cleanup.diagnostics()
            );
            assert!(
                elapsed < Duration::from_millis(400),
                "cleanups share one absolute budget instead of running back to back: {elapsed:?}"
            );
        }

        /// An execution that never reports leaves the barrier at its deadline
        /// with diagnostics naming the operation and change.
        #[tokio::test]
        async fn run_command_scope_reports_bounded_escalation_diagnostics() {
            let scope = RunCommandScope::new();
            let _held = scope
                .register_for_test("acceptance", Some("change-a"))
                .expect("an open scope admits the registration");

            let cleanup = scope.shutdown(Duration::from_millis(60)).await;

            assert!(
                !cleanup.is_quiescent(),
                "an unreported execution is never silently quiescent"
            );
            assert!(cleanup.timed_out, "the bounded barrier must expire");
            let diagnostics = cleanup.diagnostics();
            assert!(
                diagnostics.contains("op=acceptance") && diagnostics.contains("change_id=change-a"),
                "diagnostics must be actionable: {diagnostics}"
            );
        }

        /// An owned identity that is already gone is proven quiescent by the
        /// scope's own managed escalation, and the change stops blocking its
        /// completion handshake only then.
        #[cfg(unix)]
        #[tokio::test]
        async fn run_command_scope_escalates_a_retained_identity() {
            // A short-lived real child gives a genuinely dead PGID rather than a
            // number that might belong to an unrelated live process.
            let child = std::process::Command::new("sh")
                .arg("-c")
                .arg("exit 0")
                .spawn()
                .expect("spawn");
            let pid = child.id();
            let mut child = child;
            let _ = child.wait();

            let scope = RunCommandScope::new();
            let registration = scope.register_unproven_for_test("apply", Some("change-a"), pid);
            assert!(
                !scope.change_is_quiescent("change-a"),
                "a retained identity keeps the change unproven"
            );
            registration.release_confirmed();
            assert!(
                !scope.change_is_quiescent("change-a"),
                "runner-task exit alone is not cleanup evidence"
            );
            assert_eq!(scope.retained_process_ids(), vec![pid]);

            let cleanup = scope.shutdown(Duration::from_secs(2)).await;

            assert!(
                cleanup.is_quiescent(),
                "managed escalation proves the dead group quiescent: {}",
                cleanup.diagnostics()
            );
            assert!(
                scope.change_is_quiescent("change-a"),
                "only confirmed cleanup releases the change"
            );
        }

        /// A targeted force-stop kills exactly one change's process group and
        /// proves it reaped, while an unrelated change's group keeps running.
        ///
        /// Integration evidence, deliberately: the property is about real
        /// processes and real signals, and a double could not show that an
        /// unrelated PGID survived. Both children are `setsid` leaders so each
        /// PGID names one owned group and nothing else.
        #[cfg(unix)]
        #[tokio::test]
        async fn force_stop_change_kills_only_the_named_changes_process_group() {
            fn spawn_group() -> (tokio::process::Child, u32) {
                let mut command = tokio::process::Command::new("sh");
                command
                    .arg("-c")
                    .arg("sleep 30")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                crate::process_manager::configure_process_group(&mut command);
                let child = command.spawn().expect("spawn a real process group");
                let pid = child.id().expect("a spawned child has a pid");
                (child, pid)
            }

            fn group_is_alive(pgid: u32) -> bool {
                use nix::sys::signal::killpg;
                use nix::unistd::Pid;
                killpg(Pid::from_raw(pgid as i32), None).is_ok()
            }

            let (target_child, target_pgid) = spawn_group();
            let (bystander_child, bystander_pgid) = spawn_group();

            let scope = RunCommandScope::new();
            let _target = scope.register_unproven_for_test("apply", Some("alpha"), target_pgid);
            let _bystander =
                scope.register_unproven_for_test("apply", Some("beta"), bystander_pgid);

            assert!(scope.change_owns_managed_process("alpha"));
            assert!(scope.change_owns_managed_process("beta"));

            // The owner of the target child reaps it, exactly as the workspace
            // task that spawned it does in production. Without a reaper the
            // killed leader stays a zombie and the group is *not* provably
            // empty — which is the point: settlement waits for reaping, not
            // merely for the signal.
            let mut target_child = target_child;
            let reaper = tokio::spawn(async move { target_child.wait().await });

            let report = scope
                .force_stop_change("alpha", FORCE_STOP_CHANGE_KILL_BUDGET)
                .await;

            assert!(
                report.is_confirmed(),
                "the target group must be proven empty: {}",
                report.diagnostics()
            );
            assert_eq!(report.identities, 1);
            assert_eq!(report.confirmed, 1);
            assert!(
                !group_is_alive(target_pgid),
                "the target's process group must be gone before settlement"
            );
            assert!(
                !scope.change_owns_managed_process("alpha"),
                "a proven identity is released from the ownership graph"
            );

            // The unrelated change is untouched: same registration, same live
            // group, and admission is still open for the rest of the run.
            assert!(
                group_is_alive(bystander_pgid),
                "an unrelated change's process group must keep running"
            );
            assert!(scope.change_owns_managed_process("beta"));
            assert!(
                !scope.is_closed(),
                "a targeted force-stop must not close run admission"
            );

            let status = reaper
                .await
                .expect("the reaper task")
                .expect("the killed child is waitable");
            assert!(
                !status.success(),
                "a SIGKILLed child never exits successfully"
            );

            // Leave no live process behind.
            let mut bystander_child = bystander_child;
            let _ = bystander_child.kill().await;
            let _ = bystander_child.wait().await;
        }

        /// A target that owns no identity is neither signalled nor claimed.
        #[tokio::test]
        async fn force_stop_change_signals_nothing_for_a_change_with_no_process_group() {
            let scope = RunCommandScope::new();
            let _registration = scope
                .register_for_test("apply", Some("alpha"))
                .expect("an open scope admits the registration");

            assert!(
                !scope.change_owns_managed_process("alpha"),
                "a reserved execution that never spawned owns no process group"
            );

            let report = scope
                .force_stop_change("alpha", FORCE_STOP_CHANGE_KILL_BUDGET)
                .await;

            assert!(report.is_confirmed());
            assert_eq!(
                report.identities, 0,
                "nothing was signalled, so nothing needed reaping"
            );
        }

        /// A closed scope admits nothing at all, so a later operation cannot
        /// slip a command into a run that has already stopped.
        #[tokio::test]
        async fn run_command_scope_refuses_registration_once_closed() {
            let scope = RunCommandScope::new();
            scope.close();
            assert!(scope
                .register_for_test("archive", Some("change-a"))
                .is_none());
            assert!(scope.cancel_token().is_cancelled());
        }

        /// Losing the caller's handle is treated as cancellation, never as
        /// permission to keep the process group running.
        #[cfg(unix)]
        #[tokio::test]
        async fn run_command_scope_treats_handle_loss_as_cancellation() {
            let config = CommandQueueConfig {
                acceptance_max_runtime_secs:
                    crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
                stagger_delay_ms: 0,
                max_retries: 1,
                retry_delay_ms: 0,
                retry_error_patterns: vec![],
                retry_if_duration_under_secs: 0,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 1,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
                max_runtime_secs: 0,
            };
            let (runner, scope) = scoped_runner(config, Arc::new(Mutex::new(None)));

            let (handle, _rx) = runner
                .execute_streaming_with_retry(
                    "sleep 300 >/dev/null 2>&1 </dev/null & sleep 300",
                    None,
                    Some("apply"),
                    Some("change-a"),
                )
                .await
                .expect("admitted");
            tokio::time::sleep(Duration::from_millis(150)).await;
            let pgid = handle.id().expect("a real pid") as i32;

            // The workspace future was aborted: its handle is simply gone.
            drop(handle);

            let cleanup = scope.wait_quiescent(Duration::from_secs(5)).await;

            // Reap before asserting so a failure cannot leak the group.
            let survived = unsafe { libc::killpg(pgid, 0) } == 0;
            if survived {
                unsafe { libc::killpg(pgid, libc::SIGKILL) };
            }
            assert!(
                cleanup.is_quiescent(),
                "a dropped handle must still reach cleanup: {}",
                cleanup.diagnostics()
            );
            assert!(
                !survived,
                "process group {pgid} outlived the run that owned it"
            );
        }
    }

    /// A command that never starts still owes the caller its cause.
    ///
    /// Apply history, the Acceptance command diagnostic, and the cleanup-review
    /// diagnosis are all built from the bounded tails collected off this output
    /// channel, so forwarding the launch error here is what makes a launch
    /// failure legible to all three instead of a bare synthetic exit code.
    mod launch_failure_diagnostics {
        use super::*;

        #[test]
        fn the_line_names_the_stage_operation_change_and_cause() {
            let line = launch_failure_line(
                "spawn",
                Some("acceptance"),
                Some("change-a"),
                2,
                "No such file or directory (os error 2)",
            );

            assert!(line.contains("stage=spawn"), "{line}");
            assert!(line.contains("op=acceptance"), "{line}");
            assert!(line.contains("change_id=change-a"), "{line}");
            assert!(line.contains("attempt=2"), "{line}");
            assert!(
                line.contains("No such file or directory (os error 2)"),
                "the cause must survive, not just the fact of failure: {line}"
            );
        }

        #[test]
        fn an_unlabelled_invocation_still_reports_its_cause() {
            let line = launch_failure_line("process-manager", None, None, 1, "broken pipe");

            assert!(line.contains("op=unknown"), "{line}");
            assert!(line.contains("change_id=none"), "{line}");
            assert!(line.contains("broken pipe"), "{line}");
        }

        #[test]
        fn the_cause_is_bounded_and_single_line() {
            let line = launch_failure_line(
                "spawn",
                Some("apply"),
                Some("change-a"),
                1,
                &format!("failure\n{}", "x".repeat(5_000)),
            );

            assert_eq!(line.lines().count(), 1, "diagnostics stay one line");
            assert!(line.ends_with("..."), "an over-long cause is truncated");
            assert!(
                line.chars().count() < MAX_LAUNCH_ERROR_CHARS + 200,
                "the bounded cause keeps the line small: {} chars",
                line.chars().count()
            );
        }

        /// End to end over the real channel: a command that cannot be spawned at
        /// all reaches the caller as a stderr line carrying the cause, followed
        /// by the failure status.
        #[cfg(unix)]
        #[tokio::test]
        async fn a_command_that_cannot_start_reports_its_cause_on_the_output_channel() {
            let config = CommandQueueConfig {
                acceptance_max_runtime_secs:
                    crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
                stagger_delay_ms: 0,
                max_retries: 0,
                retry_delay_ms: 0,
                retry_error_patterns: vec![],
                retry_if_duration_under_secs: 0,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 1,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
                max_runtime_secs: 0,
            };
            let runner = AiCommandRunner::new(config, Arc::new(Mutex::new(None)));

            // A working directory that does not exist makes the spawn itself
            // fail, which is the path that previously produced no output at all.
            let missing_cwd = std::path::Path::new("/nonexistent-cflx-launch-failure-dir");
            let (mut handle, mut rx) = runner
                .execute_streaming_with_retry(
                    "echo never-runs",
                    Some(missing_cwd),
                    Some("acceptance"),
                    Some("change-a"),
                )
                .await
                .expect("the call itself succeeds; the launch is what fails");

            let mut stderr_lines = Vec::new();
            while let Some(line) = rx.recv().await {
                if let OutputLine::Stderr(s) = line {
                    stderr_lines.push(s);
                }
            }

            let status = handle.wait().await.expect("a final status is reported");
            assert!(!status.success(), "a launch failure is not a success");
            let joined = stderr_lines.join("\n");
            assert!(
                joined.contains("Command launch failed"),
                "the failure must be visible to every diagnostic built from this channel: \
                 {stderr_lines:?}"
            );
            assert!(
                joined.contains("op=acceptance") && joined.contains("change_id=change-a"),
                "{stderr_lines:?}"
            );
        }
    }

    /// Verify that terminating a pipeline via StreamingChildHandle kills the entire
    /// process group (sh + children), leaving no orphaned processes.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_streaming_with_retry_terminates_pipeline() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        // Pipeline: sleep 999 | cat — both processes should be killed by terminate().
        let (mut handle, _rx) = runner
            .execute_streaming_with_retry("sleep 999 | cat", None, Some("test"), None)
            .await
            .unwrap();

        // Wait for the child to be spawned.
        tokio::time::sleep(Duration::from_millis(150)).await;

        let pid = handle.id();
        assert!(pid.is_some(), "Should have a real PID");

        // Terminate the process group.
        let outcome = handle
            .terminate_with_timeout(Duration::from_secs(5))
            .await
            .unwrap();

        // Process should have exited (not timed out).
        assert!(
            !matches!(
                outcome,
                crate::process_manager::TerminationOutcome::TimedOut
            ),
            "Expected process to exit after termination, got TimedOut"
        );
    }

    #[cfg(feature = "heavy-tests")]
    #[tokio::test]
    async fn test_inactivity_timeout_streaming_pipeline() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 2,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        let start = Instant::now();
        // Pipeline with no output — sleep 30 piped through cat produces nothing.
        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(
                "sleep 30 | cat",
                None,
                Some("apply"),
                Some("test-change"),
            )
            .await
            .unwrap();

        // Collect all output lines emitted before the channel closes.
        let mut lines: Vec<String> = Vec::new();
        while let Some(line) = rx.recv().await {
            match line {
                OutputLine::Stdout(s) | OutputLine::Stderr(s) => lines.push(s),
            }
        }

        let _ = handle.wait().await;
        let elapsed = start.elapsed();

        // Should complete after timeout + grace (2s + 1s = ~3s), well under 15s.
        assert!(
            elapsed.as_secs() >= 2 && elapsed.as_secs() <= 15,
            "Expected completion between 2–15s, got {:?}",
            elapsed
        );

        // The output channel should contain a message about inactivity timeout.
        let has_timeout_msg = lines
            .iter()
            .any(|l| l.contains("inactivity timeout") && l.contains("2s"));
        assert!(
            has_timeout_msg,
            "Expected inactivity timeout message in output (with timeout seconds), got: {:?}",
            lines
        );
    }

    #[cfg(feature = "heavy-tests")]
    #[tokio::test]
    async fn test_inactivity_timeout_retry() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 100,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 2,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 3,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        // Command that produces no output — will trigger inactivity timeout on every attempt.
        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(
                "sleep 30 | cat",
                None,
                Some("apply"),
                Some("test-change-retry"),
            )
            .await
            .unwrap();

        let mut lines: Vec<String> = Vec::new();
        while let Some(line) = rx.recv().await {
            match line {
                OutputLine::Stdout(s) | OutputLine::Stderr(s) => lines.push(s),
            }
        }
        let _ = handle.wait().await;

        // Expect 3 retry messages ("[Retry 1/3]", "[Retry 2/3]", "[Retry 3/3]").
        for i in 1u32..=3 {
            let expected = format!("[Retry {}/3]", i);
            let found = lines
                .iter()
                .any(|l| l.contains(&expected) && l.contains("Inactivity timeout"));
            assert!(
                found,
                "Expected retry message '{}' with 'Inactivity timeout' in output, got: {:?}",
                expected, lines
            );
        }

        // Expect the exhaustion message.
        let exhausted = lines
            .iter()
            .any(|l| l.contains("Inactivity timeout") && l.contains("exhausted all 3 retries"));
        assert!(
            exhausted,
            "Expected 'exhausted all 3 retries' message in output, got: {:?}",
            lines
        );
    }

    /// Regression test (task 1.6): a successful command that backgrounds a child process is
    /// cleaned up by strict_process_cleanup. After the command exits with status 0, no
    /// members should remain in its process group.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_post_completion_cleanup_on_success() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 5,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        let pgid_file =
            std::env::temp_dir().join(format!("post_cleanup_success_{}.txt", std::process::id()));
        let pgid_path = pgid_file.display().to_string();
        // Write the sh PID (= PGID after setsid) to a file, background a long sleep, then exit 0.
        let cmd = format!(
            "echo $$ > {path}; sleep 30 >/dev/null 2>&1 </dev/null & exit 0",
            path = pgid_path
        );

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(&cmd, None, Some("test"), None)
            .await
            .unwrap();

        while rx.recv().await.is_some() {}
        let status = handle.wait().await.expect("wait");
        assert!(status.success(), "Command should succeed");

        // Allow signal delivery to propagate.
        tokio::time::sleep(Duration::from_millis(250)).await;

        let content = std::fs::read_to_string(&pgid_file).unwrap_or_default();
        let _ = std::fs::remove_file(&pgid_file);
        let pgid: i32 = content.trim().parse().expect("valid pgid");

        // killpg(pgid, 0) should return -1 with ESRCH — no live members remain.
        let result = unsafe { libc::killpg(pgid, 0) };
        if result == 0 {
            // Kill the leaked process so the test doesn't leave orphans.
            unsafe { libc::killpg(pgid, libc::SIGKILL) };
            panic!(
                "post-cleanup: process group {} still has live members after successful command completion",
                pgid
            );
        }
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        assert_eq!(
            errno,
            libc::ESRCH,
            "post-cleanup: expected ESRCH for pgid={}, got errno={}",
            pgid,
            errno
        );
    }

    /// Regression test (task 1.7): a failed command that backgrounds a child process is
    /// cleaned up by strict_process_cleanup. After the command exits with status 1, no
    /// members should remain in its process group.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_post_completion_cleanup_on_failure() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1, // One attempt only — no retry on failure
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0, // Disable short-duration retry
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 5,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        let pgid_file =
            std::env::temp_dir().join(format!("post_cleanup_failure_{}.txt", std::process::id()));
        let pgid_path = pgid_file.display().to_string();
        // Write the sh PID, background a long sleep, then exit 1.
        let cmd = format!(
            "echo $$ > {path}; sleep 30 >/dev/null 2>&1 </dev/null & exit 1",
            path = pgid_path
        );

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(&cmd, None, Some("test"), None)
            .await
            .unwrap();

        while rx.recv().await.is_some() {}
        let _ = handle.wait().await;

        // Allow signal delivery to propagate.
        tokio::time::sleep(Duration::from_millis(250)).await;

        let content = std::fs::read_to_string(&pgid_file).unwrap_or_default();
        let _ = std::fs::remove_file(&pgid_file);
        let pgid: i32 = content.trim().parse().expect("valid pgid");

        // killpg(pgid, 0) should return -1 with ESRCH — no live members remain.
        let result = unsafe { libc::killpg(pgid, 0) };
        if result == 0 {
            unsafe { libc::killpg(pgid, libc::SIGKILL) };
            panic!(
                "post-cleanup: process group {} still has live members after failed command completion",
                pgid
            );
        }
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        assert_eq!(
            errno,
            libc::ESRCH,
            "post-cleanup: expected ESRCH for pgid={}, got errno={}",
            pgid,
            errno
        );
    }

    /// A natural, clean completion publishes confirmed cleanup evidence, so
    /// callers that gate repository work on it may proceed.
    #[cfg(unix)]
    #[tokio::test]
    async fn apply_completion_publishes_confirmed_cleanup_for_clean_exit() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 5,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry("echo done", None, Some("apply"), Some("change-a"))
            .await
            .unwrap();

        while rx.recv().await.is_some() {}
        let status = handle.wait().await.expect("wait");
        let report = handle.process_group_cleanup().await;

        assert!(status.success());
        assert!(
            report.is_confirmed(),
            "clean completion must publish confirmed quiescence: {}",
            report.diagnostics()
        );
    }

    /// Completion-grace cancellation must not report success when the owned
    /// process group cannot be proven quiescent within the cleanup budget.
    #[cfg(unix)]
    #[tokio::test]
    async fn apply_completion_grace_termination_reports_unconfirmed_cleanup() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 5,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let mut runner = AiCommandRunner::new(config, shared_state);
        // Zero budget: a SIGTERM-immune descendant can never be proven gone.
        runner.set_process_group_cleanup_timeout_ms(0);

        let (mut handle, _rx) = runner
            .execute_streaming_with_retry(
                "sh -c 'trap \"\" TERM; while :; do sleep 0.2; done' >/dev/null 2>&1 </dev/null & \
                 sleep 120",
                None,
                Some("apply"),
                Some("change-a"),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;
        let pgid = handle.id().expect("real pid") as i32;

        // Simulate the apply completion-grace termination: signal, then wait for
        // the runner to publish its status and cleanup evidence.
        handle.terminate().expect("terminate");
        let status = handle.wait().await.expect("wait");
        let report = handle.process_group_cleanup().await;

        // Reap the survivor before asserting so a failure cannot leak it.
        unsafe { libc::killpg(pgid, libc::SIGKILL) };

        assert!(
            !report.is_confirmed(),
            "a surviving descendant must not be published as quiescent: {}",
            report.diagnostics()
        );
        assert!(
            !status.success(),
            "unconfirmed cleanup must not be published as a successful completion"
        );
        assert!(
            report.diagnostics().contains("cleanup budget expired"),
            "diagnostics must be actionable: {}",
            report.diagnostics()
        );
    }

    /// Completion-grace termination of a cooperative group publishes confirmed
    /// quiescence only after every owned member is gone.
    #[cfg(unix)]
    #[tokio::test]
    async fn apply_completion_grace_termination_confirms_quiescent_group() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 5,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        // Leader plus a descendant that outlives it until the group is swept.
        let (mut handle, _rx) = runner
            .execute_streaming_with_retry(
                "sleep 300 >/dev/null 2>&1 </dev/null & sleep 300",
                None,
                Some("apply"),
                Some("change-a"),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;
        let pgid = handle.id().expect("real pid") as i32;

        handle.terminate().expect("terminate");
        let _ = handle.wait().await;
        let report = handle.process_group_cleanup().await;

        assert!(
            report.is_confirmed(),
            "cooperative group must reach confirmed quiescence: {}",
            report.diagnostics()
        );
        // Independently verify the published evidence against the real group.
        let result = unsafe { libc::killpg(pgid, 0) };
        if result == 0 {
            unsafe { libc::killpg(pgid, libc::SIGKILL) };
            panic!("cleanup reported quiescence while pgid {pgid} still has live members");
        }
    }

    /// Regression test (task 1.8): cancellation via StreamingChildHandle triggers full
    /// process-group cleanup. After terminate_with_timeout, no members should remain.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_post_completion_cleanup_on_cancellation() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 5,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        // Long-running command: background a sleep then loop so the shell itself stays alive.
        let (mut handle, _rx) = runner
            .execute_streaming_with_retry(
                "sleep 999 >/dev/null 2>&1 </dev/null & sleep 999",
                None,
                Some("test"),
                None,
            )
            .await
            .unwrap();

        // Give the child time to start.
        tokio::time::sleep(Duration::from_millis(150)).await;

        let pid = handle.id().expect("should have a real PID") as i32;

        // Cancel via the handle.
        let outcome = handle
            .terminate_with_timeout(Duration::from_secs(10))
            .await
            .unwrap();
        assert!(
            !matches!(
                outcome,
                crate::process_manager::TerminationOutcome::TimedOut
            ),
            "Expected termination, not timeout"
        );

        // Allow OS signal delivery to settle.
        tokio::time::sleep(Duration::from_millis(250)).await;

        // The process group (PGID == PID for setsid'd process) should be fully gone.
        let result = unsafe { libc::killpg(pid, 0) };
        if result == 0 {
            unsafe { libc::killpg(pid, libc::SIGKILL) };
            panic!(
                "post-cleanup: process group {} still has live members after cancellation",
                pid
            );
        }
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        assert_eq!(
            errno,
            libc::ESRCH,
            "post-cleanup: expected ESRCH for pgid={} after cancellation, got errno={}",
            pid,
            errno
        );
    }
}
