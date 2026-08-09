//! Process-local execution facts: typed phases, boundaries, and activity.
//!
//! This is an *observability* store, not a second lifecycle authority. It holds
//! nothing the workspace cannot re-derive, it is created fresh with the process,
//! and it is discarded at exit — under `openspec/CONSTITUTION.md` the next
//! workflow action after a restart is still recomputed from workspace and Git
//! evidence alone.
//!
//! Two rules keep it from becoming a second state machine:
//!
//! * **The reducer owns the current phase.** [`ExecutionFactsStore::observe`] is
//!   called from the authoritative typed-event dispatch boundary with the
//!   reducer state that event produced, and projects
//!   [`crate::orchestration::state::ActivityState`] into the closed wire
//!   vocabulary. Nothing here classifies a phase from a display string, a task
//!   count, a log line, or a commit subject.
//! * **Completion comes from typed completion events.** A phase is recorded as
//!   *completed* only when its own typed completion event is dispatched, so an
//!   Apply that failed is never reported as the last completed phase merely
//!   because the reducer left Applying.
//!
//! The facts are read by the `/api/v2` execution-status resource and by
//! stop-and-dequeue settlement, which is what lets both answer "what was
//! actually happening" with the same evidence.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, Utc};

use crate::events::ExecutionEvent;
use crate::orchestration::state::{
    ActivityState, ChangeRuntimeState, OrchestratorState, QueueIntent, TerminalState, WaitState,
};

/// Closed per-change lifecycle phase vocabulary.
///
/// `Merge` is deliberately reachable only as a *completed* phase: the reducer
/// has no merging activity, so a per-change merge is observed through its typed
/// completion fact and never advertised as something currently running.
///
/// No `hook` value exists yet. Production emits typed hook start/completion
/// events, but they are not per-change lifecycle phases the reducer tracks, and
/// advertising a vocabulary value the server cannot classify truthfully would be
/// worse than reporting the phase the reducer really holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ExecutionPhase {
    /// Admitted to a slot and preparing its managed workspace.
    Preparing,
    /// Running Apply.
    Apply,
    /// Running acceptance.
    Acceptance,
    /// Running dedicated rejection review.
    RejectionReview,
    /// Running archive.
    Archive,
    /// Running merge resolution.
    Resolve,
    /// A typed push episode is open.
    Push,
    /// A typed per-change merge completed (completed-phase only).
    Merge,
    /// No phase is active.
    #[default]
    None,
    /// Typed evidence exists but cannot be classified.
    Unknown,
}

impl ExecutionPhase {
    /// Stable wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Apply => "apply",
            Self::Acceptance => "acceptance",
            Self::RejectionReview => "rejection_review",
            Self::Archive => "archive",
            Self::Resolve => "resolve",
            Self::Push => "push",
            Self::Merge => "merge",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }

    /// True when this value names a phase that can actually be running.
    pub fn is_active(self) -> bool {
        !matches!(self, Self::None | Self::Unknown)
    }
}

/// Closed per-change execution-state vocabulary.
///
/// Terminal outcomes take precedence, then the graceful-stop episode, then the
/// reducer's own activity and wait facts. A row the reducer tracks but has
/// neither queued, activated, held, nor finished is reported as `Unknown`
/// rather than mapped onto a state the reducer never claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChangeExecutionState {
    /// Requested to run and waiting for a slot.
    Queued,
    /// The reducer holds an active execution stage.
    Active,
    /// Held on a wait or blocker condition.
    Waiting,
    /// Active while a graceful process stop is in flight.
    Stopping,
    /// Stopped by operator request.
    Stopped,
    /// Reached a terminal failure or rejection.
    Failed,
    /// Reached a terminal success.
    Completed,
    /// Tracked, but no typed evidence classifies it.
    #[default]
    Unknown,
}

impl ChangeExecutionState {
    /// Stable wire token.
    #[allow(dead_code)] // Read by execution-facts vocabulary coverage, not by the binary.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Active => "active",
            Self::Waiting => "waiting",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Completed => "completed",
            Self::Unknown => "unknown",
        }
    }
}

/// Closed process-level activity vocabulary.
///
/// These are the episodes that are real lifecycle work without belonging to any
/// single change. Each one is opened by its typed start event and closed by its
/// typed terminal event; a process terminal closes every open episode so a
/// crashed lane cannot leave the process reporting work forever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProcessActivity {
    /// Dependency analysis over the remaining changes.
    DependencyAnalysis,
    /// Sequential base-branch merge of a completed batch.
    BaseBranchMerge,
    /// Conflict resolution inside a base-branch merge.
    ConflictResolution,
    /// Worktree branch merge requested through the worktree surface.
    BranchMerge,
    /// Managed workspace cleanup.
    WorkspaceCleanup,
}

impl ProcessActivity {
    /// Stable wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DependencyAnalysis => "dependency_analysis",
            Self::BaseBranchMerge => "base_branch_merge",
            Self::ConflictResolution => "conflict_resolution",
            Self::BranchMerge => "branch_merge",
            Self::WorkspaceCleanup => "workspace_cleanup",
        }
    }
}

/// One change's observed execution facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeExecutionFacts {
    /// Closed execution state.
    pub execution_state: ChangeExecutionState,
    /// Phase the reducer currently holds.
    pub current_phase: ExecutionPhase,
    /// When the current phase became active; `None` when no phase is active or
    /// the boundary was not observed in this incarnation.
    pub phase_started_at: Option<DateTime<Utc>>,
    /// Last phase that published its own typed completion fact.
    pub last_completed_phase: Option<ExecutionPhase>,
    /// When that completion was observed.
    pub last_completed_at: Option<DateTime<Utc>>,
    /// Retained non-empty `ApplyCompleted.revision` OID for this incarnation.
    pub apply_commit_oid: Option<String>,
}

impl ChangeExecutionFacts {
    /// The value for a change this incarnation has no typed evidence about.
    ///
    /// Deliberately not the `Default`: "no phase is running" and "nothing was
    /// observed" are different claims, and only the second one is honest for a
    /// change the store has never seen.
    pub fn unknown() -> Self {
        Self {
            execution_state: ChangeExecutionState::Unknown,
            current_phase: ExecutionPhase::Unknown,
            phase_started_at: None,
            last_completed_phase: None,
            last_completed_at: None,
            apply_commit_oid: None,
        }
    }
}

/// A coherent read of the whole store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionFactsSnapshot {
    /// Per-change facts, keyed by change ID.
    pub changes: HashMap<String, ChangeExecutionFacts>,
    /// Process-level episodes that have started and not reached a terminal.
    pub activities: Vec<ProcessActivity>,
}

impl ExecutionFactsSnapshot {
    /// Facts for one change, explicitly unknown when nothing was observed.
    pub fn change(&self, change_id: &str) -> ChangeExecutionFacts {
        self.changes
            .get(change_id)
            .cloned()
            .unwrap_or_else(ChangeExecutionFacts::unknown)
    }

    /// True when a per-change phase or a process-level episode is running.
    ///
    /// Scheduler liveness is deliberately *not* an input: a parked persistent
    /// scheduler is alive without any admitted work, and conflating the two is
    /// exactly the ambiguity this resource exists to remove.
    pub fn has_active_work(&self) -> bool {
        !self.activities.is_empty()
            || self
                .changes
                .values()
                .any(|facts| facts.current_phase.is_active())
    }
}

#[derive(Debug, Clone, Default)]
struct ChangeFactsState {
    execution_state: ChangeExecutionState,
    current_phase: ExecutionPhase,
    phase_started_at: Option<DateTime<Utc>>,
    last_completed_phase: Option<ExecutionPhase>,
    last_completed_at: Option<DateTime<Utc>>,
    apply_commit_oid: Option<String>,
    push_open: bool,
}

/// Bounded set of dispatch identities this store has already absorbed.
///
/// Two boundaries feed the store — the authoritative dispatch owner and the web
/// projection sink — and in a process that has both, one dispatch reaches it
/// twice. Absorbing an identity once is what stops the second delivery from
/// restamping a completion boundary with a later instant.
#[derive(Debug, Default)]
struct AbsorbedDispatches {
    order: VecDeque<u64>,
    seen: HashSet<u64>,
}

impl AbsorbedDispatches {
    const CAPACITY: usize = 1024;

    fn admit(&mut self, id: u64) -> bool {
        if !self.seen.insert(id) {
            return false;
        }
        self.order.push_back(id);
        while self.order.len() > Self::CAPACITY {
            if let Some(evicted) = self.order.pop_front() {
                self.seen.remove(&evicted);
            }
        }
        true
    }
}

#[derive(Debug, Default)]
struct Inner {
    changes: HashMap<String, ChangeFactsState>,
    activities: HashSet<ProcessActivity>,
    stop_requested: bool,
    absorbed: AbsorbedDispatches,
}

/// The shared process-local execution-facts store.
#[derive(Debug, Default)]
pub struct ExecutionFactsStore {
    inner: Mutex<Inner>,
}

impl ExecutionFactsStore {
    /// A store for a fresh process incarnation.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Absorb one typed dispatch and the reducer state it produced.
    ///
    /// `state` is `Some` only for a state-owning dispatch, which is the only
    /// kind that can have moved a phase. A log or presentation event still
    /// reaches here because a few typed episodes (analysis, cleanup, branch
    /// merge, conflict resolution) are presentation-owned yet are real work.
    ///
    /// `dispatch_id` makes absorption exactly-once: a repeated delivery of the
    /// same dispatch returns without restamping any boundary. Reports whether
    /// this delivery was the first.
    pub fn observe(
        &self,
        dispatch_id: u64,
        event: &ExecutionEvent,
        state: Option<&OrchestratorState>,
        now: DateTime<Utc>,
    ) -> bool {
        let mut inner = self.lock();
        if !inner.absorbed.admit(dispatch_id) {
            return false;
        }
        Self::observe_completion(&mut inner, event, now);
        Self::observe_push(&mut inner, event);
        Self::observe_process(&mut inner, event);
        if let Some(state) = state {
            Self::refresh_from_reducer(&mut inner, state, now);
        }
        true
    }

    /// Typed completion facts. Only these set the last completed phase.
    fn observe_completion(inner: &mut Inner, event: &ExecutionEvent, now: DateTime<Utc>) {
        use ExecutionEvent as E;
        let (change_id, phase) = match event {
            E::WorkspacePreparationEnded { change_id } => (change_id, ExecutionPhase::Preparing),
            E::ApplyCompleted {
                change_id,
                revision,
            } => {
                // The OID is retained *before* the completion is recorded so a
                // settlement racing this dispatch cannot see the phase without
                // the evidence that explains it. An empty revision is no
                // evidence at all and is never stored as one.
                if !revision.trim().is_empty() {
                    inner
                        .changes
                        .entry(change_id.clone())
                        .or_default()
                        .apply_commit_oid = Some(revision.trim().to_string());
                }
                (change_id, ExecutionPhase::Apply)
            }
            E::AcceptanceCompleted { change_id } => (change_id, ExecutionPhase::Acceptance),
            E::RejectionReviewCompleted { change_id, .. } => {
                (change_id, ExecutionPhase::RejectionReview)
            }
            E::ChangeArchived(change_id) => (change_id, ExecutionPhase::Archive),
            E::ResolveCompleted { change_id, .. } => (change_id, ExecutionPhase::Resolve),
            E::MergeCompleted { change_id, .. } => (change_id, ExecutionPhase::Merge),
            E::PushCompleted { change_id, .. } => (change_id, ExecutionPhase::Push),
            _ => return,
        };
        let facts = inner.changes.entry(change_id.clone()).or_default();
        facts.last_completed_phase = Some(phase);
        facts.last_completed_at = Some(now);
    }

    /// The typed push episode, which the reducer does not track as an activity.
    fn observe_push(inner: &mut Inner, event: &ExecutionEvent) {
        use ExecutionEvent as E;
        let (change_id, open) = match event {
            E::PushStarted { change_id, .. } => (change_id, true),
            E::PushCompleted { change_id, .. } | E::PushFailed { change_id, .. } => {
                (change_id, false)
            }
            _ => return,
        };
        inner
            .changes
            .entry(change_id.clone())
            .or_default()
            .push_open = open;
    }

    /// Process-level episodes and the graceful-stop qualifier.
    fn observe_process(inner: &mut Inner, event: &ExecutionEvent) {
        use ExecutionEvent as E;
        match event {
            E::AnalysisStarted { .. } => {
                inner.activities.insert(ProcessActivity::DependencyAnalysis);
            }
            E::AnalysisCompleted { .. } => {
                inner
                    .activities
                    .remove(&ProcessActivity::DependencyAnalysis);
            }
            E::MergeStarted { .. } => {
                inner.activities.insert(ProcessActivity::BaseBranchMerge);
            }
            // The base-lane episode has no terminal event of its own: it ends
            // when the batch it merges reaches a per-change outcome. Closing it
            // on any of those is what stops a finished lane from reporting work
            // forever.
            E::MergeCompleted { .. } | E::MergeDeferred { .. } | E::ResolveFailed { .. } => {
                inner.activities.remove(&ProcessActivity::BaseBranchMerge);
            }
            E::ConflictResolutionStarted => {
                inner.activities.insert(ProcessActivity::ConflictResolution);
            }
            E::ConflictResolutionCompleted | E::ConflictResolutionFailed { .. } => {
                inner
                    .activities
                    .remove(&ProcessActivity::ConflictResolution);
            }
            E::BranchMergeStarted { .. } => {
                inner.activities.insert(ProcessActivity::BranchMerge);
            }
            E::BranchMergeCompleted { .. } | E::BranchMergeFailed { .. } => {
                inner.activities.remove(&ProcessActivity::BranchMerge);
            }
            E::CleanupStarted { .. } => {
                inner.activities.insert(ProcessActivity::WorkspaceCleanup);
            }
            E::CleanupCompleted { .. } => {
                inner.activities.remove(&ProcessActivity::WorkspaceCleanup);
            }
            E::Stopping => inner.stop_requested = true,
            // A process terminal ends every episode. Anything still open at that
            // point is a lane that died with the process, not running work.
            E::Stopped | E::Error { .. } | E::AllCompleted => {
                inner.activities.clear();
                inner.stop_requested = false;
            }
            E::ProcessingStarted(_) => inner.stop_requested = false,
            _ => {}
        }
    }

    /// Re-project every tracked change from the reducer state.
    ///
    /// Every change is refreshed, not only the event's target: one typed
    /// transition can move another row (a released merge wait, a revoked
    /// dequeue), and a store that only followed the addressed change would
    /// publish a phase the reducer had already left.
    fn refresh_from_reducer(inner: &mut Inner, state: &OrchestratorState, now: DateTime<Utc>) {
        let stop_requested = inner.stop_requested;
        for change_id in state.tracked_change_ids() {
            let Some(runtime) = state.change_runtime(&change_id) else {
                continue;
            };
            let facts = inner.changes.entry(change_id).or_default();
            let phase = project_phase(runtime, facts.push_open);
            if phase != facts.current_phase {
                facts.current_phase = phase;
                facts.phase_started_at = phase.is_active().then_some(now);
            }
            facts.execution_state = project_execution_state(runtime, phase, stop_requested);
        }
    }

    /// A coherent read of every fact this incarnation has observed.
    pub fn snapshot(&self) -> ExecutionFactsSnapshot {
        let inner = self.lock();
        let mut activities: Vec<ProcessActivity> = inner.activities.iter().copied().collect();
        activities.sort();
        ExecutionFactsSnapshot {
            changes: inner
                .changes
                .iter()
                .map(|(id, facts)| {
                    (
                        id.clone(),
                        ChangeExecutionFacts {
                            execution_state: facts.execution_state,
                            current_phase: facts.current_phase,
                            phase_started_at: facts.phase_started_at,
                            last_completed_phase: facts.last_completed_phase,
                            last_completed_at: facts.last_completed_at,
                            apply_commit_oid: facts.apply_commit_oid.clone(),
                        },
                    )
                })
                .collect(),
            activities,
        }
    }

    /// One change's facts without cloning the whole store.
    pub fn change(&self, change_id: &str) -> ChangeExecutionFacts {
        let inner = self.lock();
        inner
            .changes
            .get(change_id)
            .map(|facts| ChangeExecutionFacts {
                execution_state: facts.execution_state,
                current_phase: facts.current_phase,
                phase_started_at: facts.phase_started_at,
                last_completed_phase: facts.last_completed_phase,
                last_completed_at: facts.last_completed_at,
                apply_commit_oid: facts.apply_commit_oid.clone(),
            })
            .unwrap_or_else(ChangeExecutionFacts::unknown)
    }

    /// The retained Apply-completion OID for a change, if this incarnation saw one.
    ///
    /// Empty after a restart by construction, which is the whole reason Apply
    /// commit presence is nullable: a process that never observed the completion
    /// has no typed evidence and must not guess from the repository alone.
    pub fn apply_commit_oid(&self, change_id: &str) -> Option<String> {
        self.lock()
            .changes
            .get(change_id)
            .and_then(|facts| facts.apply_commit_oid.clone())
    }
}

/// Project the reducer's activity onto the closed phase vocabulary.
///
/// The reducer is the sole authority for everything except `Push`. Publication
/// has no activity of its own there — it reuses `Resolving`, because both hold
/// the base-mutating lane — so the typed push episode is what tells the two
/// apart. It overrides only the two activities publication can legitimately be
/// running under; any other activity is a newer reducer transition and wins.
pub fn project_phase(runtime: &ChangeRuntimeState, push_open: bool) -> ExecutionPhase {
    if push_open
        && matches!(
            runtime.activity,
            ActivityState::Idle | ActivityState::Resolving
        )
    {
        return ExecutionPhase::Push;
    }
    match runtime.activity {
        ActivityState::Preparing => ExecutionPhase::Preparing,
        ActivityState::Applying => ExecutionPhase::Apply,
        ActivityState::Accepting => ExecutionPhase::Acceptance,
        ActivityState::Rejecting => ExecutionPhase::RejectionReview,
        ActivityState::Archiving => ExecutionPhase::Archive,
        ActivityState::Resolving => ExecutionPhase::Resolve,
        ActivityState::Idle => ExecutionPhase::None,
    }
}

/// Project the reducer's runtime facts onto the closed execution-state vocabulary.
pub fn project_execution_state(
    runtime: &ChangeRuntimeState,
    phase: ExecutionPhase,
    stop_requested: bool,
) -> ChangeExecutionState {
    match &runtime.terminal {
        TerminalState::Merged | TerminalState::Pushed => return ChangeExecutionState::Completed,
        TerminalState::Rejected(_) | TerminalState::Error(_) => {
            return ChangeExecutionState::Failed
        }
        TerminalState::Stopped => return ChangeExecutionState::Stopped,
        TerminalState::None => {}
    }
    if runtime.dequeued {
        return ChangeExecutionState::Stopped;
    }
    if phase.is_active() {
        return if stop_requested {
            ChangeExecutionState::Stopping
        } else {
            ChangeExecutionState::Active
        };
    }
    if !matches!(runtime.wait_state, WaitState::None) {
        return ChangeExecutionState::Waiting;
    }
    if matches!(runtime.queue_intent, QueueIntent::Queued) {
        return ChangeExecutionState::Queued;
    }
    ChangeExecutionState::Unknown
}

#[cfg(test)]
mod tests;
