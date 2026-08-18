//! Completion subscriptions: one bounded callback per admitted execution.
//!
//! Two ways in, one delivery machine. An *execution-scoped* sink names one
//! admitted episode. A *proposal-scoped* subscription names the proposal an
//! operator names, so it can be registered before any admission exists and binds
//! each new episode of that proposal as the owner opens it.
//!
//! # Why this exists
//!
//! A settled control command proves an owner accepted an intent; it does not
//! prove anything finished. A caller that wants to know when work completes
//! therefore had two bad options: hold `cflx client wait` open for the whole
//! change, or poll. Neither survives the client restarting, and process exit is
//! not a signal either — a TUI stays alive after the work it admitted is done.
//!
//! So the *owner* holds the subscription. It knows the typed transitions
//! (admission, stop, failure, blocked) that no external observer can see, and it
//! is already the process that verifies completion from repository evidence.
//!
//! # What it is not
//!
//! Not workflow state. Under `openspec/CONSTITUTION.md` the next action for a
//! workspace is derived from the workspace alone, so everything here is
//! process-local, discarded on restart, and never read back as a control input:
//!
//! * a registration cannot make a change eligible, ineligible, retried, or archived;
//! * a delivery failure cannot roll back, retry, or re-classify anything;
//! * a delivered event starts, resumes, and messages nobody: Conflux runs the
//!   registered argv and draws no conclusion from what it does;
//! * deleting the whole registry changes no next action for the same workspace.
//!
//! # Truthfulness
//!
//! `completed` is the only event with a proof obligation, and it uses the same
//! repository oracle `cflx client wait` certifies with — a typed terminal state
//! is a *claim worth verifying*, never the answer. A change that merely stopped
//! being tracked settles nothing. Inconclusive repository evidence produces a
//! bounded diagnostic, never a fabricated terminal event.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::client::completion::Verdict;
use crate::orchestration::execution_facts::{
    EpisodeObserver, EpisodeTerminal, EpisodeTransition, EpisodeTransitionKind, ExecutionFactsStore,
};
use crate::web::remote_control_api::dto::{
    ChangeExecutionState, ExecutionEventFile, ExecutionEventType, ExecutionSinkCapability,
    ExecutionSinkSpec, ProposalSubscriptionCapability, EXECUTION_EVENT_SCHEMA_VERSION,
};
use crate::web::remote_control_api::ExecutionContractHandle;

/// Longest accepted callback argv.
///
/// Small on purpose: a sink is "run this one helper", not a place to assemble a
/// command line. A caller that needs more indirection writes a script.
pub const MAX_COMMAND_ARGS: usize = 16;

/// Longest accepted single argv element, in bytes.
pub const MAX_COMMAND_ARG_LEN: usize = 4096;

/// Wall-clock ceiling one callback may run for.
pub const CALLBACK_TIMEOUT: Duration = Duration::from_secs(20);

/// Retained-output ceiling per callback stream.
///
/// Retained, not produced: both streams keep draining past this so a callback
/// can never block on a full pipe, and the excess is discarded as it arrives
/// rather than collected and cut afterwards.
pub const MAX_CALLBACK_OUTPUT_BYTES: usize = 8 * 1024;

/// Default ceiling on the whole graceful-shutdown callback drain.
///
/// One budget for *every* queued or running callback rather than one per
/// callback: with delivery serialized, a per-callback budget would let n slow
/// callbacks hold shutdown for n times as long.
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(40);

/// How long a stream drain may outlive the callback that owned it.
///
/// A callback that leaves a grandchild holding the inherited pipe keeps the
/// stream open after the callback itself is reaped. The owner reports what it
/// retained and stops waiting rather than letting somebody else's orphan hold
/// shutdown open.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Longest evidence string copied into an event file.
const MAX_EVIDENCE_BYTES: usize = 512;

/// How many times inconclusive repository evidence is re-read before the
/// terminal classification gives up and reports a diagnostic.
const VERIFY_ATTEMPTS: usize = 5;

/// Gap between those re-reads.
const VERIFY_RETRY_INTERVAL: Duration = Duration::from_millis(200);

/// Ceiling on one repository verification subprocess round.
const VERIFY_ROUND_BUDGET: Duration = Duration::from_secs(20);

/// Most proposals one atomic subscription request may address.
///
/// Bounded because `set` and `clear` are all-or-nothing over the whole list: an
/// unbounded request would be an unbounded critical section holding the registry
/// lock, and an agent that genuinely needs more than this is describing a whole
/// repository rather than the work it is waiting on.
pub const MAX_PROPOSAL_TARGETS: usize = 64;

/// What this build advertises through `/api/v2/capabilities`.
pub fn capability() -> ExecutionSinkCapability {
    ExecutionSinkCapability {
        available: true,
        max_command_args: MAX_COMMAND_ARGS,
        max_command_arg_len: MAX_COMMAND_ARG_LEN,
        callback_timeout_ms: CALLBACK_TIMEOUT.as_millis() as u64,
        max_callback_output_bytes: MAX_CALLBACK_OUTPUT_BYTES,
    }
}

/// What this build advertises for proposal-scoped subscriptions.
pub fn proposal_capability() -> ProposalSubscriptionCapability {
    ProposalSubscriptionCapability {
        available: true,
        max_targets: MAX_PROPOSAL_TARGETS,
    }
}

/// Why a sink registration was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkRefusal {
    /// This incarnation never admitted an execution with that ID.
    UnknownExecution,
    /// The execution exists but belongs to a different instance or change.
    BindingMismatch {
        /// The change the execution actually belongs to.
        actual_change_id: String,
    },
    /// The presented owner incarnation is not this one.
    ///
    /// Its own variant rather than a [`Self::BindingMismatch`] because a
    /// proposal subscription has no execution to be mismatched *against*: the
    /// only thing that can disagree is which owner the caller thinks it is
    /// talking to, and a caller told "wrong change" there would look for a
    /// change that was never the problem.
    InstanceMismatch,
    /// The argv itself is not acceptable.
    InvalidCommand(String),
}

/// One proposal's subscription and the episode it is currently bound to.
#[derive(Debug, Clone, Default)]
struct ProposalEntry {
    /// The registered callback, when one is.
    sink: Option<ExecutionSinkSpec>,
    /// Latest execution episode this owner observed for the proposal.
    ///
    /// Tracked whether or not a subscription exists, because a subscription
    /// registered *after* an episode settled owes exactly one late delivery,
    /// and it can only name the episode if the owner remembered it.
    latest: Option<String>,
}

impl ProposalEntry {
    /// Whether this entry still carries anything worth remembering.
    fn is_empty(&self) -> bool {
        self.sink.is_none() && self.latest.is_none()
    }
}

/// A read of one proposal's subscription state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalSubscriptionView {
    /// Proposal the subscription is keyed by.
    pub change_id: String,
    /// The registered callback, when one is.
    pub sink: Option<ExecutionSinkSpec>,
    /// Latest bound execution episode, when the proposal has ever been admitted.
    pub execution_id: Option<String>,
    /// Whether a terminal event was delivered for that latest episode.
    pub terminal_dispatched: bool,
    /// Event types delivered for that latest episode, in delivery order.
    pub delivered_events: Vec<ExecutionEventType>,
}

/// One execution's process-local subscription state.
#[derive(Debug, Clone)]
struct Entry {
    change_id: String,
    sink: Option<ExecutionSinkSpec>,
    /// True when this episode's sink was bound from a proposal subscription
    /// rather than registered directly against the execution.
    ///
    /// The distinction is what keeps the two surfaces independent: clearing a
    /// proposal subscription must not detach a callback somebody registered
    /// against this exact execution, and pruning a superseded episode must not
    /// throw away a direct registration.
    proposal_bound: bool,
    /// Typed terminal classification, once the reducer produced one.
    terminal: Option<EpisodeTerminal>,
    /// True once terminal handling started, which is what caps delivery at one
    /// attempt per execution regardless of how many registrations raced.
    terminal_attempted: bool,
    /// True once a terminal event was actually handed to a callback.
    terminal_dispatched: bool,
    /// True while the execution sits inside a blocked attention edge.
    blocked_active: bool,
    /// Event types handed to a callback, in delivery order.
    delivered: Vec<ExecutionEventType>,
    /// True once `owner_stopping` was attempted for this execution.
    stopping_attempted: bool,
}

impl Entry {
    fn new(change_id: String) -> Self {
        Self {
            change_id,
            sink: None,
            proposal_bound: false,
            terminal: None,
            terminal_attempted: false,
            terminal_dispatched: false,
            blocked_active: false,
            delivered: Vec::new(),
            stopping_attempted: false,
        }
    }
}

/// A read of one execution's subscription state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkView {
    /// Change the execution belongs to.
    pub change_id: String,
    /// The attached sink, when one is.
    pub sink: Option<ExecutionSinkSpec>,
    /// Whether a terminal event has been delivered.
    pub terminal_dispatched: bool,
    /// Event types already delivered, in delivery order.
    pub delivered_events: Vec<ExecutionEventType>,
}

/// Work the dispatcher task owns.
#[derive(Debug)]
enum Task {
    /// An episode moved.
    Episode(EpisodeTransition),
    /// A sink was attached and may owe an immediate terminal delivery.
    Registered { execution_id: String },
    /// The owner is shutting down gracefully.
    Stopping(tokio::sync::oneshot::Sender<()>),
}

/// Budgets a test can shorten without asserting on wall-clock latency.
///
/// Injectable because the shutdown ordering these govern — nothing starts after
/// the deadline, every child is reaped before any artifact is removed — is only
/// observable if a test can reach the deadline in bounded time. The assertions
/// are still about ordering and state, never about how long anything took.
#[derive(Debug, Clone, Copy)]
struct Limits {
    callback_timeout: Duration,
    shutdown_deadline: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            callback_timeout: CALLBACK_TIMEOUT,
            shutdown_deadline: SHUTDOWN_DEADLINE,
        }
    }
}

/// The process-local completion-sink registry.
pub struct CompletionSinkRegistry {
    instance_id: String,
    entries: Mutex<HashMap<String, Entry>>,
    /// Proposal-scoped subscriptions, keyed by `change_id`.
    ///
    /// A second map rather than a field on [`Entry`] because a subscription
    /// outlives every episode it binds: it is registered against the thing an
    /// operator names, and an execution ID does not exist yet when the agent
    /// asks to be told. Lock order is always `proposals` before `entries`; no
    /// path takes them the other way round.
    proposals: Mutex<HashMap<String, ProposalEntry>>,
    facts: Arc<ExecutionFactsStore>,
    contract: Arc<ExecutionContractHandle>,
    repo_root: Mutex<Option<PathBuf>>,
    /// Owner-private directory event files are written into.
    ///
    /// An owned path rather than a `TempDir`: a temporary-directory handle makes
    /// registry destruction an implicit cleanup authority, and dropping the
    /// registry is not proof that a callback child was reaped. Removal has to be
    /// a decision this owner takes once, explicitly, and only against a positive
    /// acknowledgement.
    event_dir: Mutex<Option<PathBuf>>,
    tasks: mpsc::UnboundedSender<Task>,
    /// True once graceful shutdown started: admission stops, so no episode
    /// transition and no late registration opens new delivery work.
    stopping: AtomicBool,
    /// Fired when the shutdown deadline expires. Past it nothing starts, no
    /// event directory or artifact is created, and the running callback is
    /// terminated and explicitly reaped.
    cancel: CancellationToken,
    limits: Mutex<Limits>,
    /// Test-only hold between shutdown cancellation and the kill-and-reap it
    /// forces; `None` in every production path.
    reap_gate: Mutex<Option<Arc<ReapGate>>>,
}

/// Test-only synchronization around the kill-and-reap shutdown cancellation
/// forces.
///
/// Injectable because "cleanup happens only after a confirmed reap" is an
/// *ordering* property. Proving it needs a test that can hold the
/// acknowledgement outstanding while a callback is demonstrably still alive,
/// and then observe what the owner did *not* do meanwhile — not one that infers
/// the ordering from how long a wait happened to take.
#[derive(Debug)]
pub struct ReapGate {
    /// A permit appears when the owner reaches the gate holding a live callback.
    reached: tokio::sync::Semaphore,
    /// Notified by the test once it has observed the retained artifacts.
    released: tokio::sync::Notify,
}

impl Default for ReapGate {
    fn default() -> Self {
        Self {
            reached: tokio::sync::Semaphore::new(0),
            released: tokio::sync::Notify::new(),
        }
    }
}

impl ReapGate {
    // Read by the sink tests, which link the library rather than the binary.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Wait until shutdown cancellation has reached a callback that is still
    /// alive and about to be terminated and reaped.
    // Read by the sink tests, which link the library rather than the binary.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn reached(&self) {
        self.reached
            .acquire()
            .await
            .expect("the reap gate is never closed")
            .forget();
    }

    /// Let the owner finish the kill-and-reap it is holding at the gate.
    // Read by the sink tests, which link the library rather than the binary.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn release(&self) {
        self.released.notify_one();
    }

    /// Owner side: announce arrival, then hold until the test releases.
    async fn hold(&self) {
        self.reached.add_permits(1);
        self.released.notified().await;
    }
}

impl std::fmt::Debug for CompletionSinkRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionSinkRegistry")
            .field("instance_id", &self.instance_id)
            .finish_non_exhaustive()
    }
}

/// The registry plus the receiver its dispatcher task drains.
struct Wiring {
    registry: Arc<CompletionSinkRegistry>,
    tasks: mpsc::UnboundedReceiver<Task>,
}

impl CompletionSinkRegistry {
    fn build(
        instance_id: String,
        facts: Arc<ExecutionFactsStore>,
        contract: Arc<ExecutionContractHandle>,
    ) -> Wiring {
        let (tx, rx) = mpsc::unbounded_channel();
        Wiring {
            registry: Arc::new(Self {
                instance_id,
                entries: Mutex::new(HashMap::new()),
                proposals: Mutex::new(HashMap::new()),
                facts,
                contract,
                repo_root: Mutex::new(None),
                event_dir: Mutex::new(None),
                tasks: tx,
                stopping: AtomicBool::new(false),
                cancel: CancellationToken::new(),
                limits: Mutex::new(Limits::default()),
                reap_gate: Mutex::new(None),
            }),
            tasks: rx,
        }
    }

    /// Create the registry, bind it to the facts store, and start its dispatcher.
    ///
    /// Must be called from inside a Tokio runtime: the dispatcher is the only
    /// place a callback is ever spawned, which is what keeps delivery off the
    /// reducer's critical path.
    pub fn start(
        instance_id: String,
        facts: Arc<ExecutionFactsStore>,
        contract: Arc<ExecutionContractHandle>,
    ) -> Arc<Self> {
        let Wiring {
            registry,
            mut tasks,
        } = Self::build(instance_id, facts.clone(), contract);
        facts.bind_episode_observer(registry.clone());
        let dispatcher = registry.clone();
        tokio::spawn(async move {
            while let Some(task) = tasks.recv().await {
                dispatcher.handle(task).await;
            }
        });
        registry
    }

    /// Bind the repository root completion evidence is read from.
    ///
    /// Without it a `completed` classification cannot be certified at all, and
    /// the registry says so in a diagnostic rather than delivering an unproven
    /// success.
    pub fn bind_repo_root(&self, repo_root: PathBuf) {
        *self.lock_repo_root() = Some(repo_root);
    }

    fn lock_repo_root(&self) -> MutexGuard<'_, Option<PathBuf>> {
        self.repo_root
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Entry>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_proposals(&self) -> MutexGuard<'_, HashMap<String, ProposalEntry>> {
        self.proposals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The owner incarnation every binding is checked against.
    // Read by the sink tests; the resources answer with the projection's own
    // instance ID so a response and a snapshot cannot disagree.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// The current closed execution state for one execution's change.
    pub fn execution_state(&self, change_id: &str) -> ChangeExecutionState {
        ChangeExecutionState::from_shared(self.facts.change(change_id).execution_state)
    }

    /// Read one execution's subscription, validating the complete binding.
    ///
    /// Inspection asserts the same `(instance_id, execution_id, change_id)` a
    /// registration does. Not because reading is privileged — all three are
    /// readable through other authenticated resources — but because a partial
    /// binding lets a caller and the owner disagree about *which* execution is
    /// being discussed, and a retry opens a new episode under the same change.
    pub fn view(
        &self,
        execution_id: &str,
        instance_id: &str,
        change_id: &str,
    ) -> Result<SinkView, SinkRefusal> {
        let entries = self.lock();
        let entry = self.resolve(&entries, execution_id, instance_id, change_id)?;
        Ok(SinkView {
            change_id: entry.change_id.clone(),
            sink: entry.sink.clone(),
            terminal_dispatched: entry.terminal_dispatched,
            delivered_events: entry.delivered.clone(),
        })
    }

    fn resolve<'a>(
        &self,
        entries: &'a HashMap<String, Entry>,
        execution_id: &str,
        instance_id: &str,
        change_id: &str,
    ) -> Result<&'a Entry, SinkRefusal> {
        let entry = entries
            .get(execution_id)
            .ok_or(SinkRefusal::UnknownExecution)?;
        if instance_id != self.instance_id || change_id != entry.change_id {
            return Err(SinkRefusal::BindingMismatch {
                actual_change_id: entry.change_id.clone(),
            });
        }
        Ok(entry)
    }

    /// Attach or replace one execution's sink.
    ///
    /// Identical requests are idempotent; a different valid argv replaces the
    /// prior sink atomically. A registration that arrives after the execution
    /// already settled owes exactly one immediate terminal delivery, which is
    /// what closes the race between enqueue settlement and registration.
    pub fn set_sink(
        &self,
        execution_id: &str,
        instance_id: &str,
        change_id: &str,
        spec: ExecutionSinkSpec,
    ) -> Result<SinkView, SinkRefusal> {
        validate_command(&spec.command)?;
        let view = {
            let mut entries = self.lock();
            // Validate against an immutable borrow first so a refusal cannot
            // have mutated anything.
            self.resolve(&entries, execution_id, instance_id, change_id)?;
            let entry = entries
                .get_mut(execution_id)
                .ok_or(SinkRefusal::UnknownExecution)?;
            entry.sink = Some(spec);
            // A direct registration names this exact execution, so it outranks
            // — and is never confused with — a sink the owner bound from a
            // proposal subscription.
            entry.proposal_bound = false;
            SinkView {
                change_id: entry.change_id.clone(),
                sink: entry.sink.clone(),
                terminal_dispatched: entry.terminal_dispatched,
                delivered_events: entry.delivered.clone(),
            }
        };
        // Admission stops at shutdown: the registration is recorded and readable,
        // but it opens no new delivery work in a process that is leaving.
        if !self.stopping.load(Ordering::SeqCst) {
            let _ = self.tasks.send(Task::Registered {
                execution_id: execution_id.to_string(),
            });
        }
        Ok(view)
    }

    /// Detach one execution's sink.
    pub fn clear_sink(
        &self,
        execution_id: &str,
        instance_id: &str,
        change_id: &str,
    ) -> Result<SinkView, SinkRefusal> {
        let mut entries = self.lock();
        self.resolve(&entries, execution_id, instance_id, change_id)?;
        let entry = entries
            .get_mut(execution_id)
            .ok_or(SinkRefusal::UnknownExecution)?;
        entry.sink = None;
        entry.proposal_bound = false;
        Ok(SinkView {
            change_id: entry.change_id.clone(),
            sink: None,
            terminal_dispatched: entry.terminal_dispatched,
            delivered_events: entry.delivered.clone(),
        })
    }

    // ── Proposal-scoped subscriptions ───────────────────────────────────────
    //
    // Registered against the proposal an operator names rather than against an
    // execution ID that does not exist yet. The owner binds each new episode of
    // that proposal to the current subscription, so re-admission after a retry
    // is a *new* delivery rather than a lost one, and dedupe stays keyed by
    // episode so replacing or clearing a subscription can never replay a
    // terminal event this owner already delivered.

    /// Register or replace one proposal's subscription.
    ///
    /// Binding the live episode happens here rather than at delivery time so a
    /// subscription set while work is already running is honored: the episode's
    /// own entry is what the dispatcher reads, and an entry with no sink is
    /// indistinguishable from one nobody asked about.
    ///
    /// The one episode it will not bind is a live one already carrying a sink
    /// registered directly against that execution. The two surfaces are separate
    /// by contract, and a standing rule about a proposal silently replacing a
    /// registration that named one exact episode — which a later `clear` would
    /// then detach — is the interference that separation exists to prevent.
    ///
    /// Registering after the latest episode already settled owes exactly one
    /// late delivery. It is requested unconditionally and *filtered* by
    /// [`Self::handle_terminal`]'s `terminal_attempted` flag, which is what makes
    /// "clear then set does not replay" hold: the flag lives on the episode, so
    /// no subscription generation can reset it.
    pub fn set_proposal_subscription(
        &self,
        change_id: &str,
        instance_id: &str,
        spec: ExecutionSinkSpec,
    ) -> Result<ProposalSubscriptionView, SinkRefusal> {
        validate_command(&spec.command)?;
        if instance_id != self.instance_id {
            return Err(SinkRefusal::InstanceMismatch);
        }
        let (view, bound) = {
            let mut proposals = self.lock_proposals();
            let proposal = proposals.entry(change_id.to_string()).or_default();
            proposal.sink = Some(spec.clone());
            let latest = proposal.latest.clone();

            let mut entries = self.lock();
            let bound = latest.as_ref().filter(|execution_id| {
                match entries.get_mut(execution_id.as_str()) {
                    // A terminal event already handed to a callback closes this
                    // episode for good; the replacement applies to the next one.
                    Some(entry) if entry.terminal_dispatched => false,
                    // A sink registered *directly* against this execution named
                    // this exact episode, and the two surfaces are separate: a
                    // standing proposal rule must not silently replace a
                    // specific registration, and clearing the proposal must not
                    // detach one it never made. The subscription still governs
                    // every future episode of the proposal.
                    Some(entry) if entry.sink.is_some() && !entry.proposal_bound => false,
                    Some(entry) => {
                        entry.sink = Some(spec.clone());
                        entry.proposal_bound = true;
                        true
                    }
                    None => false,
                }
            });
            let bound = bound.cloned();
            (
                Self::project_proposal(change_id, proposals.get(change_id), &entries),
                bound,
            )
        };
        // Admission stops at shutdown: the registration is recorded and readable,
        // but it opens no new delivery work in a process that is leaving.
        if let Some(execution_id) = bound {
            if !self.stopping.load(Ordering::SeqCst) {
                let _ = self.tasks.send(Task::Registered { execution_id });
            }
        }
        Ok(view)
    }

    /// Read one proposal's subscription, validating the owner binding.
    pub fn view_proposal_subscription(
        &self,
        change_id: &str,
        instance_id: &str,
    ) -> Result<ProposalSubscriptionView, SinkRefusal> {
        if instance_id != self.instance_id {
            return Err(SinkRefusal::InstanceMismatch);
        }
        let proposals = self.lock_proposals();
        let entries = self.lock();
        Ok(Self::project_proposal(
            change_id,
            proposals.get(change_id),
            &entries,
        ))
    }

    /// Clear one proposal's subscription.
    ///
    /// Cancels delivery that has not started, and nothing else: a callback
    /// process already running keeps its own bounds and finishes, because
    /// killing it would make "clear" a control action over something the owner
    /// already handed to another program.
    ///
    /// A sink registered directly against the episode is left alone. Clearing
    /// the proposal is not authority over an execution-scoped registration
    /// somebody else made.
    pub fn clear_proposal_subscription(
        &self,
        change_id: &str,
        instance_id: &str,
    ) -> Result<ProposalSubscriptionView, SinkRefusal> {
        if instance_id != self.instance_id {
            return Err(SinkRefusal::InstanceMismatch);
        }
        let mut proposals = self.lock_proposals();
        let mut entries = self.lock();
        if let Some(proposal) = proposals.get_mut(change_id) {
            proposal.sink = None;
            if let Some(entry) = proposal
                .latest
                .as_ref()
                .and_then(|execution_id| entries.get_mut(execution_id.as_str()))
            {
                if entry.proposal_bound {
                    entry.sink = None;
                    entry.proposal_bound = false;
                }
            }
        }
        let view = Self::project_proposal(change_id, proposals.get(change_id), &entries);
        // The delivery history of the latest episode is what a caller reads back
        // to decide whether it still owes follow-up, so the entry is kept while
        // it has one. An entry that never bound an episode and now holds no
        // subscription is nothing at all.
        if proposals
            .get(change_id)
            .is_some_and(ProposalEntry::is_empty)
        {
            proposals.remove(change_id);
        }
        Ok(view)
    }

    /// Project one proposal's registry state into its read model.
    fn project_proposal(
        change_id: &str,
        proposal: Option<&ProposalEntry>,
        entries: &HashMap<String, Entry>,
    ) -> ProposalSubscriptionView {
        let sink = proposal.and_then(|proposal| proposal.sink.clone());
        let execution_id = proposal.and_then(|proposal| proposal.latest.clone());
        let episode = execution_id
            .as_ref()
            .and_then(|execution_id| entries.get(execution_id.as_str()));
        ProposalSubscriptionView {
            change_id: change_id.to_string(),
            sink,
            execution_id,
            terminal_dispatched: episode.is_some_and(|entry| entry.terminal_dispatched),
            delivered_events: episode
                .map(|entry| entry.delivered.clone())
                .unwrap_or_default(),
        }
    }

    /// Bind a newly started episode to its proposal's subscription, if any.
    ///
    /// Also the point where a superseded episode is discarded. The contract
    /// retains at most the latest episode per proposal, and an entry the owner
    /// only created because a subscription bound it has nobody left to answer
    /// for once a newer episode exists. An entry carrying a *direct*
    /// registration is never discarded here: that registration named this exact
    /// execution, and a retry is not permission to forget it.
    fn bind_episode(&self, change_id: &str, execution_id: &str) {
        let mut proposals = self.lock_proposals();
        let mut entries = self.lock();
        let entry = entries
            .entry(execution_id.to_string())
            .or_insert_with(|| Entry::new(change_id.to_string()));
        let proposal = proposals.entry(change_id.to_string()).or_default();
        // Same precedence as `set_proposal_subscription`: a direct registration
        // for this exact execution is more specific than a standing rule about
        // the proposal, so it is not displaced.
        if entry.sink.is_none() || entry.proposal_bound {
            if let Some(spec) = proposal.sink.clone() {
                entry.sink = Some(spec);
                entry.proposal_bound = true;
            }
        }
        let superseded = proposal.latest.replace(execution_id.to_string());
        if let Some(superseded) = superseded {
            if superseded != execution_id {
                let disposable = entries
                    .get(&superseded)
                    .is_some_and(|entry| entry.proposal_bound || entry.sink.is_none());
                if disposable {
                    entries.remove(&superseded);
                }
            }
        }
    }

    /// Attempt `owner_stopping` for every live registration, then return.
    ///
    /// Best effort by construction: a crash cannot run this at all, which is
    /// exactly why an external adapter must treat a vanished owner as
    /// `owner_restarted` rather than as an outcome.
    ///
    /// The ordering is the contract. Admission stops first, so nothing new is
    /// queued. One finite deadline then governs every queued or running callback
    /// together. Reaching it cancels the rest: no further delivery starts, no
    /// event directory or artifact is created, and the callback that is still
    /// running is killed and explicitly reaped. Only once the dispatcher has
    /// acknowledged that are the artifacts removed — a file pulled out from
    /// under a live callback would be the one race this whole path exists to
    /// prevent.
    ///
    /// That acknowledgement has no second deadline of its own, deliberately.
    /// The finite deadline bounds how long a *graceful* callback may run; it
    /// says nothing about whether the child it just cancelled has been reaped.
    /// A secondary timeout that removed artifacts anyway would reintroduce
    /// exactly the live-callback cleanup race the first wait exists to avoid,
    /// and it would do so in the one situation where a callback is provably
    /// still being torn down. Waiting cannot hang instead: cancellation is
    /// kill-and-wait followed by a bounded drain, and a dispatcher that died
    /// drops the sender, which resolves the wait with an error rather than
    /// never.
    ///
    /// Only a *positive* acknowledgement authorizes cleanup. Every other way
    /// this can end — the task channel already closed, the sender dropped before
    /// the deadline, the sender dropped after cancellation — proves only that
    /// the dispatcher task ended. That is not a reap: a child it spawned may
    /// still be running with the artifact open. Those paths retain the
    /// owner-private directory and say so with its path, because a leftover
    /// directory is recoverable and a file pulled out from under a live callback
    /// is not.
    pub async fn owner_stopping(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let (done, mut wait) = tokio::sync::oneshot::channel();
        if self.tasks.send(Task::Stopping(done)).is_err() {
            // The dispatcher is gone and can no longer be asked anything. It is
            // also the only place a callback is ever spawned — but it may have
            // spawned one before it went, and nothing here can tell.
            self.retain_events(
                "the dispatcher was gone before it could be asked to acknowledge a reap",
            );
            return;
        }

        let deadline = self.limits().shutdown_deadline;
        let acknowledged = match tokio::time::timeout(deadline, &mut wait).await {
            Ok(Ok(())) => true,
            Ok(Err(_)) => {
                self.retain_events(
                    "the dispatcher dropped its acknowledgement before the shutdown deadline",
                );
                false
            }
            Err(_) => {
                self.cancel.cancel();
                match wait.await {
                    Ok(()) => true,
                    Err(_) => {
                        self.retain_events(
                            "the dispatcher dropped its acknowledgement after shutdown \
                             cancellation",
                        );
                        false
                    }
                }
            }
        };
        if acknowledged {
            self.remove_events();
        }
    }

    /// Remove the owner-private event directory and everything left in it.
    ///
    /// Every artifact still present belongs to a callback that has already been
    /// reaped: this runs only after the dispatcher acknowledged the reap, and
    /// there is no second path into it. The directory is an owned path rather
    /// than a `TempDir`, so registry destruction cannot remove it either.
    fn remove_events(&self) {
        let Some(dir) = self.lock_event_dir().take() else {
            return;
        };
        if let Err(error) = std::fs::remove_dir_all(&dir) {
            if error.kind() != std::io::ErrorKind::NotFound {
                debug!(
                    event_dir = %dir.display(),
                    error = %error,
                    "the acknowledged owner-private event directory could not be removed"
                );
            }
        }
    }

    /// Keep the event directory, because no reap was ever acknowledged.
    ///
    /// The fail-safe half of the same rule cleanup obeys. The diagnostic carries
    /// the retained directory path and a fixed reason, and nothing else: no
    /// payload, no callback output, no environment value, no token. The slot is
    /// deliberately left populated — there is no state in which something else
    /// later decides to delete it.
    fn retain_events(&self, reason: &'static str) {
        let Some(dir) = self.lock_event_dir().clone() else {
            return;
        };
        warn!(
            event_dir = %dir.display(),
            reason,
            "callback reap was not acknowledged, so the owner-private event directory is retained"
        );
    }

    /// Install the test-only gate held between cancellation and kill-and-reap.
    // Read by the sink tests, which link the library rather than the binary.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_reap_gate(&self, gate: Arc<ReapGate>) {
        *self.lock_reap_gate() = Some(gate);
    }

    /// Shorten the per-callback runtime ceiling.
    ///
    /// For tests that need to reach a bounded terminal state; production uses
    /// [`CALLBACK_TIMEOUT`].
    // Read by the sink tests, which link the library rather than the binary.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_callback_timeout(&self, timeout: Duration) {
        self.lock_limits().callback_timeout = timeout;
    }

    /// Shorten the single global shutdown deadline.
    ///
    /// For tests that need shutdown cancellation to be reachable in bounded
    /// time; production uses [`SHUTDOWN_DEADLINE`].
    // Read by the sink tests, which link the library rather than the binary.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_shutdown_deadline(&self, deadline: Duration) {
        self.lock_limits().shutdown_deadline = deadline;
    }

    fn limits(&self) -> Limits {
        *self.lock_limits()
    }

    fn lock_limits(&self) -> MutexGuard<'_, Limits> {
        self.limits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_event_dir(&self) -> MutexGuard<'_, Option<PathBuf>> {
        self.event_dir
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_reap_gate(&self) -> MutexGuard<'_, Option<Arc<ReapGate>>> {
        self.reap_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn reap_gate(&self) -> Option<Arc<ReapGate>> {
        self.lock_reap_gate().clone()
    }

    // ── Dispatcher ──────────────────────────────────────────────────────────

    async fn handle(&self, task: Task) {
        match task {
            Task::Episode(transition) => self.handle_episode(transition).await,
            Task::Registered { execution_id } => self.handle_terminal(&execution_id).await,
            Task::Stopping(done) => {
                self.handle_stopping().await;
                let _ = done.send(());
            }
        }
    }

    async fn handle_episode(&self, transition: EpisodeTransition) {
        match transition.kind {
            EpisodeTransitionKind::Started => {
                // One call rather than two: creating the episode entry and
                // binding it to the proposal's current subscription have to be
                // the same step, or a terminal transition arriving in between
                // would find an unsubscribed episode and deliver nothing.
                self.bind_episode(&transition.change_id, &transition.execution_id);
            }
            EpisodeTransitionKind::BlockedEntered => {
                let should_deliver = {
                    let mut entries = self.lock();
                    match entries.get_mut(&transition.execution_id) {
                        Some(entry) => {
                            entry.blocked_active = true;
                            entry.sink.as_ref().is_some_and(|sink| sink.notify_blocked)
                                && !entry.terminal_dispatched
                        }
                        None => false,
                    }
                };
                if should_deliver {
                    self.deliver(&transition.execution_id, ExecutionEventType::Blocked, None)
                        .await;
                }
            }
            EpisodeTransitionKind::BlockedLeft => {
                if let Some(entry) = self.lock().get_mut(&transition.execution_id) {
                    entry.blocked_active = false;
                }
            }
            EpisodeTransitionKind::Terminal(terminal) => {
                if let Some(entry) = self.lock().get_mut(&transition.execution_id) {
                    entry.terminal = Some(terminal);
                    entry.blocked_active = false;
                }
                self.handle_terminal(&transition.execution_id).await;
            }
        }
    }

    /// Take the single terminal-delivery attempt one episode is allowed.
    ///
    /// `None` means there is nothing to deliver *or* somebody already took it.
    /// Both are the same answer to the caller, and collapsing them here is what
    /// makes the rule one place rather than a condition every caller repeats.
    ///
    /// Claiming inside the lock is what makes "one terminal delivery per
    /// episode" hold against a registration racing the reducer's own transition,
    /// and keying the flag on the *episode* rather than on the subscription is
    /// what makes replacing, clearing, or clearing-then-setting unable to replay
    /// a terminal event this owner already delivered.
    fn claim_terminal(&self, execution_id: &str) -> Option<(EpisodeTerminal, String)> {
        let mut entries = self.lock();
        let entry = entries.get_mut(execution_id)?;
        let terminal = entry.terminal?;
        if entry.terminal_attempted || entry.sink.is_none() {
            return None;
        }
        entry.terminal_attempted = true;
        Some((terminal, entry.change_id.clone()))
    }

    /// Deliver the terminal event for one execution, at most once, if it can be
    /// told truthfully.
    async fn handle_terminal(&self, execution_id: &str) {
        let Some((terminal, change_id)) = self.claim_terminal(execution_id) else {
            return;
        };

        let (event_type, evidence) = match terminal {
            EpisodeTerminal::Failed => (ExecutionEventType::Failed, None),
            EpisodeTerminal::Stopped => (ExecutionEventType::Stopped, None),
            EpisodeTerminal::Completed => match self.certify(&change_id).await {
                Some(evidence) => (ExecutionEventType::Completed, Some(evidence)),
                None => {
                    warn!(
                        change_id = %change_id,
                        execution_id = %execution_id,
                        "execution reported terminal success but repository evidence did not \
                         prove the owner's terminal mode; no completion event was dispatched"
                    );
                    return;
                }
            },
        };
        self.deliver(execution_id, event_type, evidence).await;
    }

    /// Deliver `owner_stopping` to every live registration, one at a time.
    ///
    /// Serialized like every other delivery, and abandoned the moment the global
    /// deadline cancels: a queued delivery that has not started must not start
    /// after it.
    async fn handle_stopping(&self) {
        let live: Vec<String> = {
            let mut entries = self.lock();
            entries
                .iter_mut()
                .filter(|(_, entry)| {
                    entry.sink.is_some() && !entry.terminal_dispatched && !entry.stopping_attempted
                })
                .map(|(id, entry)| {
                    entry.stopping_attempted = true;
                    id.clone()
                })
                .collect()
        };
        for execution_id in live {
            if self.cancel.is_cancelled() {
                debug!(
                    remaining = %execution_id,
                    "the shutdown deadline passed, so no further owner_stopping delivery starts"
                );
                break;
            }
            self.deliver(&execution_id, ExecutionEventType::OwnerStopping, None)
                .await;
        }
    }

    /// Certify a claimed terminal success from current repository evidence.
    ///
    /// The same oracle `cflx client wait` uses, so an owner-side subscription
    /// and a bounded client wait cannot disagree about what "done" means.
    ///
    /// Gives up the moment shutdown cancels. Cleanup waits for the dispatcher's
    /// reap acknowledgement with no deadline of its own, and this loop runs on
    /// that same dispatcher: re-reading a repository nobody is waiting on any
    /// more would hold the acknowledgement — and therefore shutdown — for
    /// verification rounds whose answer can no longer be delivered.
    async fn certify(&self, change_id: &str) -> Option<String> {
        let repo_root = self.lock_repo_root().clone();
        let Some(repo_root) = repo_root else {
            debug!(
                change_id = %change_id,
                "no repository root is bound, so a claimed completion cannot be certified"
            );
            return None;
        };
        let Some(contract) = self.contract.resolve(Some(change_id)) else {
            debug!(
                change_id = %change_id,
                "this owner published no execution contract, so nothing would prove completion"
            );
            return None;
        };

        for attempt in 0..VERIFY_ATTEMPTS {
            if self.cancel.is_cancelled() {
                debug!(
                    change_id = %change_id,
                    "the shutdown deadline passed, so completion verification stops"
                );
                return None;
            }
            let deadline = tokio::time::Instant::now() + VERIFY_ROUND_BUDGET;
            match crate::client::completion::certify(change_id, &repo_root, &contract, deadline)
                .await
            {
                Verdict::Completed { evidence } => return Some(evidence),
                Verdict::Broken { detail } => {
                    debug!(change_id = %change_id, detail = %detail, "completion evidence is unusable");
                    return None;
                }
                Verdict::Unsupported { detail } => {
                    debug!(change_id = %change_id, detail = %detail, "terminal mode has no repository proof");
                    return None;
                }
                Verdict::NotCompleted { detail } => {
                    debug!(
                        change_id = %change_id,
                        attempt = attempt + 1,
                        detail = %detail,
                        "completion evidence is not yet present"
                    );
                }
                Verdict::DeadlineExpired => {
                    debug!(change_id = %change_id, "repository verification exceeded its budget");
                }
            }
            if attempt + 1 < VERIFY_ATTEMPTS {
                tokio::select! {
                    _ = tokio::time::sleep(VERIFY_RETRY_INTERVAL) => {}
                    _ = self.cancel.cancelled() => {}
                }
            }
        }
        None
    }

    /// Write the event file and run the callback, bounded in every direction.
    async fn deliver(
        &self,
        execution_id: &str,
        event_type: ExecutionEventType,
        evidence: Option<String>,
    ) {
        if self.cancel.is_cancelled() {
            return;
        }
        let Some((change_id, sink)) = ({
            let entries = self.lock();
            entries.get(execution_id).and_then(|entry| {
                entry
                    .sink
                    .clone()
                    .map(|sink| (entry.change_id.clone(), sink))
            })
        }) else {
            return;
        };

        let payload = ExecutionEventFile {
            schema_version: EXECUTION_EVENT_SCHEMA_VERSION,
            event_type,
            instance_id: self.instance_id.clone(),
            execution_id: execution_id.to_string(),
            change_id: change_id.clone(),
            emitted_at: chrono::Utc::now().to_rfc3339(),
            terminal: event_type.is_terminal(),
            terminal_mode: self
                .contract
                .resolve(Some(&change_id))
                .map(|contract| contract.terminal_mode),
            evidence: evidence.map(|evidence| truncate(&evidence, MAX_EVIDENCE_BYTES)),
        };

        let path = match self.write_event(execution_id, event_type, &payload) {
            Ok(path) => path,
            Err(error) => {
                warn!(
                    change_id = %change_id,
                    execution_id = %execution_id,
                    error = %error,
                    "the completion event file could not be written; no callback was started"
                );
                return;
            }
        };

        {
            let mut entries = self.lock();
            if let Some(entry) = entries.get_mut(execution_id) {
                entry.delivered.push(event_type);
                if event_type.is_terminal() {
                    entry.terminal_dispatched = true;
                }
            }
        }

        let report = run_callback(
            &sink.command,
            &path,
            &payload,
            self.limits().callback_timeout,
            &self.cancel,
            self.reap_gate(),
        )
        .await;
        // Removed only now, which is after the child has been reaped — by its own
        // exit, by the timeout, or by shutdown cancellation. A live callback
        // never has its payload taken away from underneath it, and a finished one
        // leaves nothing behind. The owner has not read the file back at any
        // point: it is an output, and a same-UID callback can defeat its
        // permissions, so no owner decision may depend on its contents.
        let _ = std::fs::remove_file(&path);

        let truncated = report.truncated();
        match &report.outcome {
            Ok(()) => debug!(
                change_id = %change_id,
                execution_id = %execution_id,
                event = event_type.as_str(),
                stdout_bytes = report.stdout.total,
                stderr_bytes = report.stderr.total,
                output_truncated = truncated,
                "completion callback finished"
            ),
            // Observability only. Nothing below this line can change a workflow
            // outcome, and nothing retries: a callback that fails twice would
            // still not be evidence about the change.
            Err(detail) => warn!(
                change_id = %change_id,
                execution_id = %execution_id,
                event = event_type.as_str(),
                detail = %detail,
                stdout_bytes = report.stdout.total,
                stderr_bytes = report.stderr.total,
                output_truncated = truncated,
                "completion callback failed"
            ),
        }
    }

    fn write_event(
        &self,
        execution_id: &str,
        event_type: ExecutionEventType,
        payload: &ExecutionEventFile,
    ) -> std::io::Result<PathBuf> {
        if self.cancel.is_cancelled() {
            return Err(std::io::Error::other(
                "the shutdown deadline passed, so no event directory or artifact is created",
            ));
        }
        let dir = {
            let mut slot = self.lock_event_dir();
            match slot.as_ref() {
                Some(dir) => dir.clone(),
                None => {
                    // Still randomized exclusive creation: a predictable path or
                    // a plain `create_dir_all` under a shared `TMPDIR` could be
                    // pre-created or symlinked into by another user. Only the
                    // automatic Drop cleanup is disarmed, by taking ownership of
                    // the path with `keep()`.
                    let created = tempfile::Builder::new().prefix("cflx-events-").tempdir()?;
                    restrict(created.path(), 0o700)?;
                    let path = created.keep();
                    *slot = Some(path.clone());
                    path
                }
            }
        };
        let path = dir.join(format!("{execution_id}-{}.json", event_type.as_str()));
        let body = serde_json::to_vec_pretty(payload)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        write_owner_only(&path, &body)?;
        Ok(path)
    }
}

impl EpisodeObserver for CompletionSinkRegistry {
    fn observe_episode(&self, transition: &EpisodeTransition) {
        // Admission stops at shutdown, so a transition arriving while the owner
        // is leaving opens no new delivery work.
        if self.stopping.load(Ordering::SeqCst) {
            return;
        }
        // Never blocks and never runs a callback inline: the reducer's dispatch
        // boundary must not be able to wait on somebody's shell script.
        let _ = self.tasks.send(Task::Episode(transition.clone()));
    }
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}

/// Write one event payload as an owner-read-only file.
///
/// `0400` inside the `0700` event directory, so an ordinary callback opening
/// `CFLX_EVENT_PATH` for writing or truncation is refused by the file
/// permissions. That is *default* mutation refusal, not an integrity boundary: a
/// callback runs under the owner's own UID and can `chmod` its way past it. What
/// makes that harmless is the other half of the contract — the owner writes the
/// file once and never reads it back, so no owner decision can be changed by
/// editing it.
#[cfg(unix)]
fn write_owner_only(path: &Path, body: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    // Delivery is serialized and an artifact is removed only after its callback
    // is reaped, so any file still at this path is nobody's. Removing it first
    // keeps the create exclusive — and a `0400` file cannot be reopened for
    // writing anyway, not even by the owner that made it.
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(path)?;
    file.write_all(body)
}

#[cfg(not(unix))]
fn write_owner_only(path: &Path, body: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, body)
}

/// Reject an argv that is not a bounded, directly-executable command.
pub fn validate_command(command: &[String]) -> Result<(), SinkRefusal> {
    if command.is_empty() {
        return Err(SinkRefusal::InvalidCommand(
            "a completion sink needs at least a program to run".to_string(),
        ));
    }
    if command.len() > MAX_COMMAND_ARGS {
        return Err(SinkRefusal::InvalidCommand(format!(
            "a completion sink accepts at most {MAX_COMMAND_ARGS} argv elements"
        )));
    }
    for argument in command {
        if argument.len() > MAX_COMMAND_ARG_LEN {
            return Err(SinkRefusal::InvalidCommand(format!(
                "one argv element exceeds {MAX_COMMAND_ARG_LEN} bytes"
            )));
        }
        // NUL cannot cross the exec boundary, and a control byte in argv is
        // never an intentional program argument.
        if argument.bytes().any(|byte| byte < 0x20 && byte != b'\t') {
            return Err(SinkRefusal::InvalidCommand(
                "argv elements may not contain control characters".to_string(),
            ));
        }
    }
    if command[0].trim().is_empty() {
        return Err(SinkRefusal::InvalidCommand(
            "the program name is empty".to_string(),
        ));
    }
    Ok(())
}

/// What one bounded stream drain retained, and what the callback actually wrote.
#[derive(Debug, Default)]
struct Drained {
    /// Retained bytes; never more than the configured limit.
    retained: Vec<u8>,
    /// Total bytes the callback produced, including the discarded excess.
    total: usize,
}

impl Drained {
    fn truncated(&self) -> bool {
        self.total > self.retained.len()
    }

    /// The retained bytes as bounded diagnostic text.
    fn text(&self) -> String {
        let text = String::from_utf8_lossy(&self.retained);
        match self.truncated() {
            true => format!("{text}… ({} bytes total)", self.total),
            false => text.to_string(),
        }
    }
}

/// What one delivery attempt observed, in bounded form.
#[derive(Debug)]
struct CallbackReport {
    /// `Ok` when the callback exited successfully.
    outcome: Result<(), String>,
    stdout: Drained,
    stderr: Drained,
}

impl CallbackReport {
    /// A report for an attempt that never produced a child at all.
    fn unstarted(detail: String) -> Self {
        Self {
            outcome: Err(detail),
            stdout: Drained::default(),
            stderr: Drained::default(),
        }
    }

    fn truncated(&self) -> bool {
        self.stdout.truncated() || self.stderr.truncated()
    }
}

/// Read one stream to its end, retaining at most `limit` bytes.
///
/// The reading does not stop at the limit — that is the point. A drain that
/// stopped would leave the pipe full and the callback blocked in `write()`
/// forever, which is a way of hanging a callback rather than bounding it. So the
/// excess is read and dropped as it arrives, and owner memory stays inside the
/// limit no matter how much the callback produces.
async fn drain<R>(mut stream: R, limit: usize) -> Drained
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut drained = Drained {
        retained: Vec::new(),
        total: 0,
    };
    let mut chunk = [0u8; 8 * 1024];
    loop {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                drained.total = drained.total.saturating_add(read);
                if drained.retained.len() < limit {
                    let room = limit - drained.retained.len();
                    drained.retained.extend_from_slice(&chunk[..read.min(room)]);
                }
            }
        }
    }
    drained
}

/// Collect one drain, without letting an orphan that inherited the pipe hold on.
async fn collect(handle: tokio::task::JoinHandle<Drained>) -> Drained {
    let mut handle = handle;
    match tokio::time::timeout(DRAIN_GRACE, &mut handle).await {
        Ok(Ok(drained)) => drained,
        Ok(Err(_)) => Drained::default(),
        Err(_) => {
            handle.abort();
            Drained::default()
        }
    }
}

/// Run one callback with a bounded runtime and bounded retained output.
///
/// The environment is *replaced*, not extended: the callback receives exactly
/// the five documented variables, so an owner's configured token, provider
/// credentials, and terminal settings cannot reach a third-party helper.
///
/// Both streams are drained concurrently for the whole life of the child, so
/// output volume can never block the callback and can never grow the owner.
/// Reaching the retention limit is a diagnostic, not a delivery failure, and it
/// never terminates the callback. Only the runtime ceiling and shutdown
/// cancellation do that, and both kill *and* explicitly reap: `wait_with_output`
/// cannot be interrupted, and `kill_on_drop` schedules a reap rather than
/// guaranteeing one has happened by the time this returns.
async fn run_callback(
    command: &[String],
    event_path: &Path,
    payload: &ExecutionEventFile,
    timeout: Duration,
    cancel: &CancellationToken,
    reap_gate: Option<Arc<ReapGate>>,
) -> CallbackReport {
    /// How the child stopped running.
    enum Ended {
        Exited(std::process::ExitStatus),
        Broken(String),
        TimedOut,
        Cancelled,
    }

    let mut child = tokio::process::Command::new(&command[0]);
    child
        .args(&command[1..])
        .env_clear()
        .env("CFLX_EVENT_PATH", event_path)
        .env("CFLX_EVENT_TYPE", payload.event_type.as_str())
        .env("CFLX_EXECUTION_ID", &payload.execution_id)
        .env("CFLX_CHANGE_ID", &payload.change_id)
        .env("CFLX_INSTANCE_ID", &payload.instance_id)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut spawned = match child.spawn() {
        Ok(spawned) => spawned,
        Err(error) => return CallbackReport::unstarted(error.to_string()),
    };
    let stdout = spawned.stdout.take();
    let stderr = spawned.stderr.take();
    let stdout = tokio::spawn(async move {
        match stdout {
            Some(stream) => drain(stream, MAX_CALLBACK_OUTPUT_BYTES).await,
            None => Drained::default(),
        }
    });
    let stderr = tokio::spawn(async move {
        match stderr {
            Some(stream) => drain(stream, MAX_CALLBACK_OUTPUT_BYTES).await,
            None => Drained::default(),
        }
    });

    let mut ended = tokio::select! {
        result = spawned.wait() => match result {
            Ok(status) => Ended::Exited(status),
            Err(error) => Ended::Broken(error.to_string()),
        },
        _ = tokio::time::sleep(timeout) => Ended::TimedOut,
        _ = cancel.cancelled() => Ended::Cancelled,
    };
    if matches!(ended, Ended::TimedOut | Ended::Cancelled) {
        if matches!(ended, Ended::Cancelled) {
            if let Some(gate) = &reap_gate {
                // Never installed outside the tests that prove shutdown cleanup
                // waits for this reap rather than for a clock.
                gate.hold().await;
            }
        }
        // `kill` is terminate-and-wait, so the child is reaped before anything
        // downstream removes its event file.
        if let Err(error) = spawned.kill().await {
            ended = Ended::Broken(format!("the callback could not be terminated: {error}"));
        }
    }

    let stdout = collect(stdout).await;
    let stderr = collect(stderr).await;

    let outcome = match ended {
        Ended::Exited(status) if status.success() => Ok(()),
        Ended::Exited(status) => Err(format!(
            "exit {:?}: {}",
            status.code(),
            truncate(&stderr.text(), MAX_CALLBACK_OUTPUT_BYTES)
        )),
        Ended::Broken(error) => Err(error),
        Ended::TimedOut => Err(format!(
            "the callback did not finish within {}ms and was terminated",
            timeout.as_millis()
        )),
        Ended::Cancelled => Err(
            "the owner reached its shutdown deadline, so the callback was terminated".to_string(),
        ),
    };
    CallbackReport {
        outcome,
        stdout,
        stderr,
    }
}

/// Cut a string to a byte ceiling on a character boundary.
fn truncate(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut end = limit;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_argv_must_be_bounded_and_free_of_control_bytes() {
        assert!(validate_command(&["/bin/true".to_string()]).is_ok());
        assert!(matches!(
            validate_command(&[]),
            Err(SinkRefusal::InvalidCommand(_))
        ));
        assert!(matches!(
            validate_command(&["".to_string()]),
            Err(SinkRefusal::InvalidCommand(_))
        ));
        assert!(matches!(
            validate_command(&["/bin/echo".to_string(), "a\nb".to_string()]),
            Err(SinkRefusal::InvalidCommand(_))
        ));
        let too_many: Vec<String> = (0..MAX_COMMAND_ARGS + 1).map(|i| i.to_string()).collect();
        assert!(matches!(
            validate_command(&too_many),
            Err(SinkRefusal::InvalidCommand(_))
        ));
        assert!(matches!(
            validate_command(&["x".repeat(MAX_COMMAND_ARG_LEN + 1)]),
            Err(SinkRefusal::InvalidCommand(_))
        ));
    }

    #[test]
    fn every_terminal_event_type_is_terminal_and_the_others_are_not() {
        assert!(ExecutionEventType::Completed.is_terminal());
        assert!(ExecutionEventType::Failed.is_terminal());
        assert!(ExecutionEventType::Stopped.is_terminal());
        assert!(!ExecutionEventType::Blocked.is_terminal());
        assert!(!ExecutionEventType::OwnerStopping.is_terminal());
    }

    #[test]
    fn truncation_keeps_a_character_boundary() {
        assert_eq!(truncate("abc", 8), "abc");
        assert_eq!(truncate("abcdef", 3), "abc…");
        // A multi-byte character straddling the limit is dropped whole.
        assert_eq!(truncate("aé", 2), "a…");
    }

    /// The retention bound is on what the owner *keeps*, and the draining does
    /// not stop when it is reached — a drain that stopped would leave the pipe
    /// full and the callback blocked in `write()`, which is a way of hanging a
    /// callback rather than bounding it.
    #[tokio::test]
    async fn a_drain_retains_at_most_the_limit_and_still_consumes_the_rest() {
        let produced = vec![b'a'; MAX_CALLBACK_OUTPUT_BYTES * 4 + 7];
        let drained = drain(produced.as_slice(), MAX_CALLBACK_OUTPUT_BYTES).await;

        assert_eq!(
            drained.retained.len(),
            MAX_CALLBACK_OUTPUT_BYTES,
            "owner memory stays inside the configured bound"
        );
        assert_eq!(
            drained.total,
            produced.len(),
            "and every byte was still read, so the writer is never blocked"
        );
        assert!(drained.truncated());
        // The diagnostic says it was cut, and how much there was.
        let text = drained.text();
        assert!(
            text.ends_with(&format!("({} bytes total)", produced.len())),
            "{text}"
        );
    }

    #[tokio::test]
    async fn a_drain_under_the_limit_retains_everything_and_reports_no_truncation() {
        let produced = b"one bounded line\n".to_vec();
        let drained = drain(produced.as_slice(), MAX_CALLBACK_OUTPUT_BYTES).await;
        assert_eq!(drained.retained, produced);
        assert_eq!(drained.total, produced.len());
        assert!(!drained.truncated());
        assert_eq!(drained.text(), "one bounded line\n");
    }

    // ── Proposal-scoped subscriptions ───────────────────────────────────────
    //
    // Unit-scoped by construction: every assertion below is about the registry's
    // own decisions — which episode a subscription binds, and who is allowed to
    // take an episode's single terminal-delivery attempt. None of it writes an
    // event file, spawns a callback, or waits on a clock, because none of those
    // is what the contract is about.

    /// A registry with no dispatcher running, so queued delivery work stays
    /// observable as queued work rather than becoming a subprocess.
    fn proposal_registry() -> (Arc<CompletionSinkRegistry>, mpsc::UnboundedReceiver<Task>) {
        let Wiring { registry, tasks } = CompletionSinkRegistry::build(
            "i-1".to_string(),
            Arc::new(ExecutionFactsStore::new()),
            Arc::new(crate::web::remote_control_api::ExecutionContractHandle::default()),
        );
        (registry, tasks)
    }

    fn spec(program: &str) -> ExecutionSinkSpec {
        ExecutionSinkSpec {
            command: vec![program.to_string()],
            notify_blocked: false,
        }
    }

    /// Announce one episode the way the reducer's own transition does.
    fn start_episode(registry: &CompletionSinkRegistry, change_id: &str, execution_id: &str) {
        registry.bind_episode(change_id, execution_id);
    }

    /// Record the typed terminal classification for one episode.
    fn settle(registry: &CompletionSinkRegistry, execution_id: &str, terminal: EpisodeTerminal) {
        let mut entries = registry.lock();
        let entry = entries
            .get_mut(execution_id)
            .expect("the episode must exist before it settles");
        entry.terminal = Some(terminal);
    }

    /// How many delivery attempts are queued for one episode.
    fn queued_registrations(
        tasks: &mut mpsc::UnboundedReceiver<Task>,
        execution_id: &str,
    ) -> usize {
        let mut count = 0;
        while let Ok(task) = tasks.try_recv() {
            if matches!(&task, Task::Registered { execution_id: queued } if queued == execution_id)
            {
                count += 1;
            }
        }
        count
    }

    #[test]
    fn a_subscription_binds_the_owner_incarnation_it_names() {
        let (registry, _tasks) = proposal_registry();
        assert!(matches!(
            registry.set_proposal_subscription("alpha", "i-2", spec("/bin/true")),
            Err(SinkRefusal::InstanceMismatch)
        ));
        assert!(matches!(
            registry.view_proposal_subscription("alpha", "i-2"),
            Err(SinkRefusal::InstanceMismatch)
        ));
        assert!(matches!(
            registry.clear_proposal_subscription("alpha", "i-2"),
            Err(SinkRefusal::InstanceMismatch)
        ));
        // Nothing was recorded by any of the three refusals.
        let view = registry
            .view_proposal_subscription("alpha", "i-1")
            .expect("this incarnation");
        assert!(view.sink.is_none());
    }

    #[test]
    fn an_unacceptable_argv_is_refused_before_anything_is_recorded() {
        let (registry, _tasks) = proposal_registry();
        let refusal = registry.set_proposal_subscription(
            "alpha",
            "i-1",
            ExecutionSinkSpec {
                command: Vec::new(),
                notify_blocked: false,
            },
        );
        assert!(matches!(refusal, Err(SinkRefusal::InvalidCommand(_))));
        assert!(registry
            .view_proposal_subscription("alpha", "i-1")
            .unwrap()
            .sink
            .is_none());
    }

    /// The case the whole resource exists for: an agent asks to be told about a
    /// proposal that has never been admitted, and the next episode binds to it.
    #[test]
    fn a_subscription_registered_before_admission_binds_the_next_episode() {
        let (registry, mut tasks) = proposal_registry();
        let view = registry
            .set_proposal_subscription("alpha", "i-1", spec("/bin/true"))
            .expect("a pre-admission subscription is legal");
        assert!(view.sink.is_some());
        assert_eq!(view.execution_id, None, "no episode is synthesized");
        assert_eq!(queued_registrations(&mut tasks, "e-1"), 0);

        start_episode(&registry, "alpha", "e-1");
        let view = registry.view_proposal_subscription("alpha", "i-1").unwrap();
        assert_eq!(view.execution_id.as_deref(), Some("e-1"));

        settle(&registry, "e-1", EpisodeTerminal::Completed);
        assert!(
            registry.claim_terminal("e-1").is_some(),
            "the bound episode owes exactly one delivery attempt"
        );
    }

    /// Registering after the latest episode already settled owes one late
    /// delivery, which is what closes the start/registration race.
    #[test]
    fn a_late_subscription_claims_the_retained_terminal_episode_once() {
        let (registry, mut tasks) = proposal_registry();
        start_episode(&registry, "alpha", "e-1");
        settle(&registry, "e-1", EpisodeTerminal::Failed);
        // No subscription yet, so there is nothing to deliver to.
        assert!(registry.claim_terminal("e-1").is_none());

        registry
            .set_proposal_subscription("alpha", "i-1", spec("/bin/true"))
            .expect("a late subscription");
        assert_eq!(
            queued_registrations(&mut tasks, "e-1"),
            1,
            "the late registration asks the dispatcher to look at the settled episode"
        );
        assert!(registry.claim_terminal("e-1").is_some());
        assert!(
            registry.claim_terminal("e-1").is_none(),
            "and only once, however many times it is asked"
        );
    }

    /// Dedupe is keyed by the episode, not by the subscription generation. This
    /// is the property that stops a replacement — or a clear-then-set — from
    /// replaying a terminal event this owner already delivered.
    #[test]
    fn replacing_or_clearing_a_subscription_never_replays_a_delivered_terminal() {
        for replay in ["replace", "clear then set"] {
            let (registry, mut tasks) = proposal_registry();
            registry
                .set_proposal_subscription("alpha", "i-1", spec("/bin/true"))
                .unwrap();
            start_episode(&registry, "alpha", "e-1");
            settle(&registry, "e-1", EpisodeTerminal::Stopped);
            assert!(registry.claim_terminal("e-1").is_some(), "{replay}");

            if replay == "clear then set" {
                registry
                    .clear_proposal_subscription("alpha", "i-1")
                    .unwrap();
            }
            registry
                .set_proposal_subscription("alpha", "i-1", spec("/bin/false"))
                .unwrap();
            // The dispatcher is asked to look, and finds the attempt already taken.
            assert_eq!(queued_registrations(&mut tasks, "e-1"), 1, "{replay}");
            assert!(
                registry.claim_terminal("e-1").is_none(),
                "{replay} must not replay e-1"
            );

            // A *new* episode is independently deliverable under the replacement.
            start_episode(&registry, "alpha", "e-2");
            settle(&registry, "e-2", EpisodeTerminal::Stopped);
            assert!(registry.claim_terminal("e-2").is_some(), "{replay}");
        }
    }

    /// Re-admission after a retry is a distinct episode carrying the replacement
    /// argv, and the proposal subscription survives it.
    #[test]
    fn re_admission_creates_a_distinct_episode_under_the_same_subscription() {
        let (registry, _tasks) = proposal_registry();
        registry
            .set_proposal_subscription("alpha", "i-1", spec("/bin/true"))
            .unwrap();
        start_episode(&registry, "alpha", "e-1");
        settle(&registry, "e-1", EpisodeTerminal::Failed);
        assert!(registry.claim_terminal("e-1").is_some());

        start_episode(&registry, "alpha", "e-2");
        let view = registry.view_proposal_subscription("alpha", "i-1").unwrap();
        assert_eq!(view.execution_id.as_deref(), Some("e-2"));
        assert!(view.sink.is_some(), "the subscription outlives the episode");
        // At most the latest episode is retained per proposal.
        assert!(
            !registry.lock().contains_key("e-1"),
            "a superseded proposal-bound episode is discarded"
        );
        settle(&registry, "e-2", EpisodeTerminal::Failed);
        assert!(
            registry.claim_terminal("e-2").is_some(),
            "e-1's dedupe must not suppress e-2"
        );
    }

    #[test]
    fn clearing_removes_only_the_named_proposal() {
        let (registry, _tasks) = proposal_registry();
        for change_id in ["alpha", "beta", "gamma"] {
            registry
                .set_proposal_subscription(change_id, "i-1", spec("/bin/true"))
                .unwrap();
        }
        registry
            .clear_proposal_subscription("alpha", "i-1")
            .unwrap();
        registry
            .clear_proposal_subscription("gamma", "i-1")
            .unwrap();

        for cleared in ["alpha", "gamma"] {
            let view = registry.view_proposal_subscription(cleared, "i-1").unwrap();
            assert!(view.sink.is_none(), "{cleared}");
        }
        assert!(registry
            .view_proposal_subscription("beta", "i-1")
            .unwrap()
            .sink
            .is_some());
    }

    /// Clearing cancels delivery that has not started. It does so by detaching
    /// the *episode's* binding, which is the same state a not-yet-started
    /// delivery reads.
    #[test]
    fn clearing_cancels_an_unstarted_delivery_for_the_live_episode() {
        let (registry, _tasks) = proposal_registry();
        registry
            .set_proposal_subscription("alpha", "i-1", spec("/bin/true"))
            .unwrap();
        start_episode(&registry, "alpha", "e-1");
        settle(&registry, "e-1", EpisodeTerminal::Completed);

        registry
            .clear_proposal_subscription("alpha", "i-1")
            .unwrap();
        assert!(
            registry.claim_terminal("e-1").is_none(),
            "a cleared subscription has nobody to deliver to"
        );
        // The delivery history of the latest episode is still readable, because
        // that is what a caller reads to decide whether it still owes follow-up.
        let view = registry.view_proposal_subscription("alpha", "i-1").unwrap();
        assert_eq!(view.execution_id.as_deref(), Some("e-1"));
    }

    /// The two surfaces are independent. A proposal clear is not authority over
    /// a callback somebody registered against this exact execution, and a
    /// superseded episode carrying one is not discarded.
    #[test]
    fn a_proposal_subscription_never_disturbs_an_execution_scoped_registration() {
        let (registry, _tasks) = proposal_registry();
        start_episode(&registry, "alpha", "e-1");
        registry
            .set_sink("e-1", "i-1", "alpha", spec("/bin/direct"))
            .expect("a direct registration");

        registry
            .set_proposal_subscription("alpha", "i-1", spec("/bin/proposal"))
            .unwrap();
        let view = registry.view("e-1", "i-1", "alpha").expect("still there");
        assert_eq!(
            view.sink.expect("the direct registration stands").command,
            vec!["/bin/direct".to_string()],
            "a standing proposal rule must not replace a registration that named this episode"
        );

        registry
            .clear_proposal_subscription("alpha", "i-1")
            .unwrap();
        let view = registry.view("e-1", "i-1", "alpha").expect("still there");
        assert_eq!(
            view.sink.expect("the direct registration survives").command,
            vec!["/bin/direct".to_string()],
            "clearing a proposal must not detach an execution-scoped sink"
        );

        // The subscription still governs every *future* episode, and the episode
        // holding a direct registration is not thrown away by a retry.
        registry
            .set_proposal_subscription("alpha", "i-1", spec("/bin/proposal"))
            .unwrap();
        start_episode(&registry, "alpha", "e-2");
        assert!(
            registry.lock().contains_key("e-1"),
            "an episode carrying a direct registration is not pruned by a retry"
        );
        assert_eq!(
            registry
                .lock()
                .get("e-2")
                .unwrap()
                .sink
                .as_ref()
                .unwrap()
                .command,
            vec!["/bin/proposal".to_string()]
        );
    }

    #[test]
    fn a_subscription_replaced_after_delivery_applies_to_the_next_episode_only() {
        let (registry, _tasks) = proposal_registry();
        registry
            .set_proposal_subscription("alpha", "i-1", spec("/bin/first"))
            .unwrap();
        start_episode(&registry, "alpha", "e-1");
        settle(&registry, "e-1", EpisodeTerminal::Completed);
        assert!(registry.claim_terminal("e-1").is_some());
        // Mark the episode delivered, which is what `deliver` records.
        registry.lock().get_mut("e-1").unwrap().terminal_dispatched = true;

        registry
            .set_proposal_subscription("alpha", "i-1", spec("/bin/second"))
            .unwrap();
        assert_eq!(
            registry
                .lock()
                .get("e-1")
                .unwrap()
                .sink
                .as_ref()
                .unwrap()
                .command,
            vec!["/bin/first".to_string()],
            "a closed episode keeps the argv it was delivered with"
        );

        start_episode(&registry, "alpha", "e-2");
        assert_eq!(
            registry
                .lock()
                .get("e-2")
                .unwrap()
                .sink
                .as_ref()
                .unwrap()
                .command,
            vec!["/bin/second".to_string()],
            "the replacement applies to the next episode"
        );
    }

    #[test]
    fn the_published_proposal_capability_matches_the_enforced_bound() {
        let capability = proposal_capability();
        assert!(capability.available);
        assert_eq!(capability.max_targets, MAX_PROPOSAL_TARGETS);
    }

    #[test]
    fn the_published_capability_matches_the_enforced_limits() {
        let capability = capability();
        assert!(capability.available);
        assert_eq!(capability.max_command_args, MAX_COMMAND_ARGS);
        assert_eq!(capability.max_command_arg_len, MAX_COMMAND_ARG_LEN);
        assert_eq!(
            capability.callback_timeout_ms,
            CALLBACK_TIMEOUT.as_millis() as u64
        );
        assert_eq!(
            capability.max_callback_output_bytes,
            MAX_CALLBACK_OUTPUT_BYTES
        );
    }
}
