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
    /// Process-local identity of the most recent admitted execution episode.
    ///
    /// `None` until any admission source moved this change into queued or
    /// active work in *this* incarnation. It survives the episode's terminal
    /// settlement so a late subscriber can still address the execution that
    /// just finished, and is replaced — never reused — by the next admission.
    pub execution_id: Option<String>,
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
            execution_id: None,
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

/// The states that mean "this change is admitted work right now".
///
/// Admission — not activity — is what opens an execution episode: a change that
/// is queued behind a slot has already been admitted by whichever source asked
/// for it, and a subscriber that only learned about it once a phase started
/// would miss the window an enqueue caller is actually in.
pub fn is_admitted_execution_state(state: ChangeExecutionState) -> bool {
    matches!(
        state,
        ChangeExecutionState::Queued
            | ChangeExecutionState::Active
            | ChangeExecutionState::Waiting
            | ChangeExecutionState::Stopping
    )
}

/// How one admitted execution episode ended, in the owner's own typed terms.
///
/// Derived from the reducer's terminal state, never from a display string, an
/// error body, or a change disappearing from the snapshot. `Completed` here is a
/// *claim worth verifying*, not a completion: the repository oracle decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeTerminal {
    /// The reducer reached a terminal success for this change.
    Completed,
    /// The reducer reached a terminal failure or rejection.
    Failed,
    /// The episode was stopped or dequeued, including before any active work.
    Stopped,
}

impl EpisodeTerminal {
    /// Stable wire token.
    // Read by the episode-vocabulary assertions; the binary maps the enum onto
    // the API's own event type instead.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// What happened to one execution episode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeTransitionKind {
    /// A non-admitted change became admitted work; the episode ID is new.
    Started,
    /// The open episode entered a typed blocked/waiting condition.
    BlockedEntered,
    /// The open episode left that condition, arming the next attention edge.
    BlockedLeft,
    /// The episode settled; no further transition can belong to this ID.
    Terminal(EpisodeTerminal),
}

/// One published episode transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeTransition {
    /// Change the episode belongs to.
    pub change_id: String,
    /// Process-local episode identity.
    pub execution_id: String,
    /// What happened.
    pub kind: EpisodeTransitionKind,
}

/// A process-local consumer of episode transitions.
///
/// Observability only. An observer cannot refuse, delay, or redirect a
/// transition, and nothing it does is read back as a workflow input — it is
/// called after the store's own lock is released precisely so it can never
/// influence the projection it is describing.
pub trait EpisodeObserver: std::fmt::Debug + Send + Sync {
    /// Absorb one transition. Must not block for long.
    fn observe_episode(&self, transition: &EpisodeTransition);
}

#[derive(Debug, Clone, Default)]
struct ChangeFactsState {
    /// Identity of the most recent admitted episode, retained after it settles.
    execution_id: Option<String>,
    /// True while that episode is still admitted, which is what makes the next
    /// admission a *new* episode rather than a continuation of this one.
    episode_open: bool,
    /// True while the open episode is inside a blocked attention edge.
    blocked_edge: bool,
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
    /// Episode transitions produced by the current absorption, drained and
    /// published *after* the store lock is released so an observer can never
    /// re-enter the store from inside its own mutex.
    pending: Vec<EpisodeTransition>,
}

/// The shared process-local execution-facts store.
#[derive(Debug, Default)]
pub struct ExecutionFactsStore {
    inner: Mutex<Inner>,
    /// Late-bound episode consumer. Unbound is the ordinary case: a build or a
    /// frontend with no completion-sink dispatcher still tracks episodes, it
    /// simply has nobody to tell.
    observer: std::sync::RwLock<Option<std::sync::Arc<dyn EpisodeObserver>>>,
}

impl ExecutionFactsStore {
    /// A store for a fresh process incarnation.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind the process-local episode consumer.
    ///
    /// Idempotent replacement rather than a list: exactly one dispatcher owns
    /// completion sinks in a process, and a second one would double-deliver.
    pub fn bind_episode_observer(&self, observer: std::sync::Arc<dyn EpisodeObserver>) {
        *self
            .observer
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(observer);
    }

    /// Publish drained transitions without holding the store lock.
    fn publish(&self, transitions: Vec<EpisodeTransition>) {
        if transitions.is_empty() {
            return;
        }
        let observer = self
            .observer
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let Some(observer) = observer else {
            return;
        };
        for transition in &transitions {
            observer.observe_episode(transition);
        }
    }

    /// The most recent episode identity for one change, if this incarnation
    /// admitted it at all.
    // A convenience read over the same field `snapshot()` and `change()`
    // publish; the API resources reach it through those.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn execution_id(&self, change_id: &str) -> Option<String> {
        self.lock()
            .changes
            .get(change_id)
            .and_then(|facts| facts.execution_id.clone())
    }

    /// The change one episode identity belongs to, if this incarnation owns it.
    ///
    /// Linear over tracked changes on purpose: the map is bounded by the number
    /// of changes a process tracks, and a second index would be one more thing
    /// to keep consistent with the reducer.
    // The sink registry keeps its own binding, so this exists for assertions
    // that the store's own view agrees with it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn change_of_execution(&self, execution_id: &str) -> Option<String> {
        self.lock()
            .changes
            .iter()
            .find(|(_, facts)| facts.execution_id.as_deref() == Some(execution_id))
            .map(|(id, _)| id.clone())
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
        let transitions = {
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
            std::mem::take(&mut inner.pending)
        };
        self.publish(transitions);
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
        // Split the borrow so one pass can both update a change's facts and
        // append the episode transition that update produced.
        let Inner {
            changes, pending, ..
        } = inner;
        for change_id in state.tracked_change_ids() {
            let Some(runtime) = state.change_runtime(&change_id) else {
                continue;
            };
            let change_key = change_id.clone();
            let facts = changes.entry(change_id).or_default();
            let phase = project_phase(runtime, facts.push_open);
            if phase != facts.current_phase {
                facts.current_phase = phase;
                facts.phase_started_at = phase.is_active().then_some(now);
            }
            let next = project_execution_state(runtime, phase, stop_requested);
            facts.execution_state = next;
            Self::advance_episode(pending, &change_key, facts, next);
        }
    }

    /// Move one change's execution episode in step with its projected state.
    ///
    /// The rule is deliberately narrow: admission opens an episode, leaving
    /// admission settles it, and nothing else creates identity. A change that
    /// simply stops being tracked never settles here — disappearance proves
    /// nothing, and inventing a terminal for it is exactly the lie the
    /// completion contract exists to prevent.
    fn advance_episode(
        pending: &mut Vec<EpisodeTransition>,
        change_id: &str,
        facts: &mut ChangeFactsState,
        next: ChangeExecutionState,
    ) {
        let admitted = is_admitted_execution_state(next);

        if admitted && !facts.episode_open {
            let execution_id = crate::ids::new_hex_id();
            facts.execution_id = Some(execution_id.clone());
            facts.episode_open = true;
            facts.blocked_edge = false;
            pending.push(EpisodeTransition {
                change_id: change_id.to_string(),
                execution_id,
                kind: EpisodeTransitionKind::Started,
            });
        }

        let Some(execution_id) = facts.execution_id.clone() else {
            return;
        };

        if !facts.episode_open {
            return;
        }

        if admitted {
            // Attention is edge-triggered: an unchanged waiting state publishes
            // nothing, while leaving and re-entering it arms a new edge.
            let blocked = matches!(next, ChangeExecutionState::Waiting);
            if blocked != facts.blocked_edge {
                facts.blocked_edge = blocked;
                pending.push(EpisodeTransition {
                    change_id: change_id.to_string(),
                    execution_id,
                    kind: if blocked {
                        EpisodeTransitionKind::BlockedEntered
                    } else {
                        EpisodeTransitionKind::BlockedLeft
                    },
                });
            }
            return;
        }

        let terminal = match next {
            ChangeExecutionState::Completed => EpisodeTerminal::Completed,
            ChangeExecutionState::Failed => EpisodeTerminal::Failed,
            // `Stopped` is settled stop/dequeue removal. `Unknown` reaches here
            // only by leaving admission without a typed terminal — a revoked
            // queue intent — which is the same fact from the caller's side.
            ChangeExecutionState::Stopped | ChangeExecutionState::Unknown => {
                EpisodeTerminal::Stopped
            }
            // Unreachable: every remaining variant is an admitted state.
            _ => return,
        };
        facts.episode_open = false;
        facts.blocked_edge = false;
        pending.push(EpisodeTransition {
            change_id: change_id.to_string(),
            execution_id,
            kind: EpisodeTransitionKind::Terminal(terminal),
        });
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
                            execution_id: facts.execution_id.clone(),
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
                execution_id: facts.execution_id.clone(),
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
    // A dependency wait is a dispatch exclusion applied to admitted queue
    // intent: no execution episode has started, and the retained intent is what
    // will dispatch the change once the dependency resolves. It therefore stays
    // `queued` here, and the `blocked` display plus the structured dependency
    // blocker are what explain *why* the slot is empty. Every other wait state
    // holds an episode that already began, so it stays `waiting`.
    if matches!(runtime.wait_state, WaitState::DependencyBlocked)
        && matches!(runtime.queue_intent, QueueIntent::Queued)
    {
        return ChangeExecutionState::Queued;
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
