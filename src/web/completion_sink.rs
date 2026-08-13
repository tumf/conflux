//! Execution-scoped completion sinks: one bounded callback per admitted run.
//!
//! # Why this exists
//!
//! `cflx client enqueue` proves an owner accepted an intent; it does not prove
//! anything finished. A caller that wants autonomous continuation therefore had
//! two bad options: hold `cflx client wait` open for the whole change, or poll.
//! Neither survives the client restarting, and process exit is not a signal
//! either — a TUI stays alive after the work it admitted is done.
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
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::client::completion::Verdict;
use crate::orchestration::execution_facts::{
    EpisodeObserver, EpisodeTerminal, EpisodeTransition, EpisodeTransitionKind, ExecutionFactsStore,
};
use crate::web::remote_control_api::dto::{
    ChangeExecutionState, ExecutionEventFile, ExecutionEventType, ExecutionSinkCapability,
    ExecutionSinkSpec, EXECUTION_EVENT_SCHEMA_VERSION,
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

/// Captured-output ceiling per callback stream.
pub const MAX_CALLBACK_OUTPUT_BYTES: usize = 8 * 1024;

/// Longest evidence string copied into an event file.
const MAX_EVIDENCE_BYTES: usize = 512;

/// How many times inconclusive repository evidence is re-read before the
/// terminal classification gives up and reports a diagnostic.
const VERIFY_ATTEMPTS: usize = 5;

/// Gap between those re-reads.
const VERIFY_RETRY_INTERVAL: Duration = Duration::from_millis(200);

/// Ceiling on one repository verification subprocess round.
const VERIFY_ROUND_BUDGET: Duration = Duration::from_secs(20);

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
    /// The argv itself is not acceptable.
    InvalidCommand(String),
}

/// One execution's process-local subscription state.
#[derive(Debug, Clone)]
struct Entry {
    change_id: String,
    sink: Option<ExecutionSinkSpec>,
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

/// The process-local completion-sink registry.
pub struct CompletionSinkRegistry {
    instance_id: String,
    entries: Mutex<HashMap<String, Entry>>,
    facts: Arc<ExecutionFactsStore>,
    contract: Arc<ExecutionContractHandle>,
    repo_root: Mutex<Option<PathBuf>>,
    /// Owner-private directory event files are written into.
    event_dir: Mutex<Option<Arc<tempfile::TempDir>>>,
    tasks: mpsc::UnboundedSender<Task>,
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
                facts,
                contract,
                repo_root: Mutex::new(None),
                event_dir: Mutex::new(None),
                tasks: tx,
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

    /// Read one execution's subscription, validating the presented binding.
    pub fn view(
        &self,
        execution_id: &str,
        instance_id: Option<&str>,
        change_id: Option<&str>,
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
        instance_id: Option<&str>,
        change_id: Option<&str>,
    ) -> Result<&'a Entry, SinkRefusal> {
        let entry = entries
            .get(execution_id)
            .ok_or(SinkRefusal::UnknownExecution)?;
        if let Some(instance_id) = instance_id {
            if instance_id != self.instance_id {
                return Err(SinkRefusal::BindingMismatch {
                    actual_change_id: entry.change_id.clone(),
                });
            }
        }
        if let Some(change_id) = change_id {
            if change_id != entry.change_id {
                return Err(SinkRefusal::BindingMismatch {
                    actual_change_id: entry.change_id.clone(),
                });
            }
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
            self.resolve(&entries, execution_id, Some(instance_id), Some(change_id))?;
            let entry = entries
                .get_mut(execution_id)
                .ok_or(SinkRefusal::UnknownExecution)?;
            entry.sink = Some(spec);
            SinkView {
                change_id: entry.change_id.clone(),
                sink: entry.sink.clone(),
                terminal_dispatched: entry.terminal_dispatched,
                delivered_events: entry.delivered.clone(),
            }
        };
        let _ = self.tasks.send(Task::Registered {
            execution_id: execution_id.to_string(),
        });
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
        self.resolve(&entries, execution_id, Some(instance_id), Some(change_id))?;
        let entry = entries
            .get_mut(execution_id)
            .ok_or(SinkRefusal::UnknownExecution)?;
        entry.sink = None;
        Ok(SinkView {
            change_id: entry.change_id.clone(),
            sink: None,
            terminal_dispatched: entry.terminal_dispatched,
            delivered_events: entry.delivered.clone(),
        })
    }

    /// Attempt `owner_stopping` for every live registration, then return.
    ///
    /// Best effort by construction: a crash cannot run this at all, which is
    /// exactly why an external adapter must treat a vanished owner as
    /// `owner_restarted` rather than as an outcome.
    pub async fn owner_stopping(&self) {
        let (done, wait) = tokio::sync::oneshot::channel();
        if self.tasks.send(Task::Stopping(done)).is_err() {
            return;
        }
        let _ = tokio::time::timeout(CALLBACK_TIMEOUT * 2, wait).await;
        // Remaining event files belong to callbacks that never reaped; the
        // owner owns their removal.
        self.lock_event_dir().take();
    }

    fn lock_event_dir(&self) -> MutexGuard<'_, Option<Arc<tempfile::TempDir>>> {
        self.event_dir
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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
                self.lock()
                    .entry(transition.execution_id.clone())
                    .or_insert_with(|| Entry::new(transition.change_id.clone()));
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

    /// Deliver the terminal event for one execution, at most once, if it can be
    /// told truthfully.
    async fn handle_terminal(&self, execution_id: &str) {
        let claimed = {
            let mut entries = self.lock();
            let Some(entry) = entries.get_mut(execution_id) else {
                return;
            };
            let Some(terminal) = entry.terminal else {
                return;
            };
            if entry.terminal_attempted || entry.sink.is_none() {
                return;
            }
            // Claiming the attempt inside the lock is what makes "one terminal
            // delivery per execution" hold against a registration racing the
            // reducer's own transition.
            entry.terminal_attempted = true;
            (terminal, entry.change_id.clone())
        };
        let (terminal, change_id) = claimed;

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
            self.deliver(&execution_id, ExecutionEventType::OwnerStopping, None)
                .await;
        }
    }

    /// Certify a claimed terminal success from current repository evidence.
    ///
    /// The same oracle `cflx client wait` uses, so an owner-side subscription
    /// and a bounded client wait cannot disagree about what "done" means.
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
                tokio::time::sleep(VERIFY_RETRY_INTERVAL).await;
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

        let outcome = run_callback(&sink.command, &path, &payload).await;
        // The file is immutable for the callback's lifetime and removed once it
        // is reaped, so a slow callback can never read a rewritten payload and a
        // finished one leaves nothing behind.
        let _ = std::fs::remove_file(&path);

        match outcome {
            Ok(()) => debug!(
                change_id = %change_id,
                execution_id = %execution_id,
                event = event_type.as_str(),
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
        let dir = {
            let mut slot = self.lock_event_dir();
            match slot.as_ref() {
                Some(dir) => dir.clone(),
                None => {
                    let created =
                        Arc::new(tempfile::Builder::new().prefix("cflx-events-").tempdir()?);
                    restrict(created.path(), 0o700)?;
                    *slot = Some(created.clone());
                    created
                }
            }
        };
        let path = dir
            .path()
            .join(format!("{execution_id}-{}.json", event_type.as_str()));
        let body = serde_json::to_vec_pretty(payload)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        std::fs::write(&path, body)?;
        restrict(&path, 0o600)?;
        Ok(path)
    }
}

impl EpisodeObserver for CompletionSinkRegistry {
    fn observe_episode(&self, transition: &EpisodeTransition) {
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

/// Run one callback with a bounded runtime and bounded captured output.
///
/// The environment is *replaced*, not extended: the callback receives exactly
/// the five documented variables, so an owner's configured token, provider
/// credentials, and terminal settings cannot reach a third-party helper.
async fn run_callback(
    command: &[String],
    event_path: &Path,
    payload: &ExecutionEventFile,
) -> Result<(), String> {
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

    let spawned = child.spawn().map_err(|error| error.to_string())?;
    match tokio::time::timeout(CALLBACK_TIMEOUT, spawned.wait_with_output()).await {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(format!(
            "exit {:?}: {}",
            output.status.code(),
            truncate(
                &String::from_utf8_lossy(&output.stderr),
                MAX_CALLBACK_OUTPUT_BYTES
            )
        )),
        Ok(Err(error)) => Err(error.to_string()),
        // Dropping the future kills the child because of `kill_on_drop`, so a
        // hung callback cannot outlive its budget and keep a file handle open.
        Err(_) => Err(format!(
            "the callback did not finish within {}ms",
            CALLBACK_TIMEOUT.as_millis()
        )),
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
