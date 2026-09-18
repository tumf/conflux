//! Bounded argv-only JSON subprocess boundary for purpose-scoped judge commands.
//!
//! A judge is a *non-authoritative* observer: it receives one complete request
//! object on stdin and returns one response object on stdout. Nothing here can
//! change a workflow decision — the only outputs are a strictly validated
//! response or a bounded failure category.
//!
//! This module deliberately does not reuse [`crate::ai_command_runner`]. An
//! agent runner's shell templates, retries, stream-json textification, output
//! callbacks, command queue, and multi-hour timeouts are all wrong for a
//! bounded machine protocol:
//!
//! - argv is executed directly, with no shell and no placeholder expansion;
//! - the request is written to stdin exactly once and stdin is then closed;
//! - stdout and stderr are drained concurrently under a byte limit, so a chatty
//!   command can never deadlock the write side;
//! - the invocation has one bounded timeout and one explicit cancellation path;
//! - no retry is ever performed, so a failing judge costs exactly one call.
//!
//! The compatibility baseline for the request/response/exit-code contract is
//! `jev-cli v0.4.1`, tag commit `50488f35d705051b5d621d1d10e69d1503783e7d`
//! (<https://github.com/tumf/jev-cli/releases/tag/v0.4.1>). Conflux never
//! inspects `jev --version` and accepts any command conforming to it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::process_manager::{configure_process_group, ManagedChild};

/// Judgment purpose of the only judge command this change consumes.
pub const PARALLEL_DEPENDENCY_PURPOSE: &str = "parallel_dependency";

/// Fixed grace between TERM and KILL when a judge has to be cleaned up.
///
/// Deliberately short and non-configurable: a judge is already known to be
/// non-authoritative at this point, so the only thing left to do is stop it
/// without leaking a process.
pub const JUDGE_CLEANUP_GRACE: Duration = Duration::from_millis(100);

/// Bounded window allowed for reaping a judge after KILL.
const JUDGE_REAP_WINDOW: Duration = Duration::from_secs(2);

/// Chunk size used by the bounded output drains.
const DRAIN_CHUNK_BYTES: usize = 8 * 1024;

/// Bounded, redaction-safe outcome category for one judge invocation.
///
/// Every variant is a *shadow* failure: none of them can fail, block, reorder,
/// or delay the authoritative analysis path. The tokens are stable so they can
/// appear in diagnostics without carrying any request or response content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgeFailureCategory {
    /// The command could not be started at all (missing executable, permissions).
    Spawn,
    /// The serialized request exceeded the configured input limit; no spawn happened.
    InputTooLarge,
    /// The invocation exceeded its bounded timeout.
    Timeout,
    /// The caller cancelled the invocation because the authoritative path finished.
    Cancelled,
    /// The command exited non-zero without a more specific classification.
    NonzeroExit,
    /// The command reported a CLI/input error (baseline exit code 2).
    InvalidRequest,
    /// The command reported a credential/authentication error (baseline exit code 3).
    Authentication,
    /// The command reported a transient API/network error (baseline exit code 4).
    Transient,
    /// Collected stdout or stderr exceeded the configured output limit.
    OutputTooLarge,
    /// stdout was not exactly one JSON value followed by whitespace.
    InvalidJson,
    /// stdout parsed as JSON but violated the expected schema, types, or ranges.
    SchemaMismatch,
    /// A local I/O failure occurred while talking to the child.
    Io,
}

impl JudgeFailureCategory {
    /// Short stable token for diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::InputTooLarge => "input_too_large",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::NonzeroExit => "nonzero_exit",
            Self::InvalidRequest => "invalid_request",
            Self::Authentication => "authentication",
            Self::Transient => "transient",
            Self::OutputTooLarge => "output_too_large",
            Self::InvalidJson => "invalid_json",
            Self::SchemaMismatch => "schema_mismatch",
            Self::Io => "io",
        }
    }

    /// Classify a non-zero exit status against the baseline exit-code contract.
    ///
    /// The mapping is internal observability only: every variant stays a
    /// non-authoritative shadow failure.
    fn from_exit_code(code: Option<i32>) -> Self {
        match code {
            Some(2) => Self::InvalidRequest,
            Some(3) => Self::Authentication,
            Some(4) => Self::Transient,
            _ => Self::NonzeroExit,
        }
    }
}

/// One bounded judge invocation definition.
///
/// `argv` is executed directly. There is no template, no `{placeholder}`, and
/// no shell, so nothing in a proposal or change ID can become a command word.
#[derive(Debug, Clone)]
pub struct JudgeCommandSpec {
    /// Judge argv; the first element is the executable.
    pub argv: Vec<String>,
    /// Working directory for the child (the repository root).
    pub working_dir: PathBuf,
    /// Extra environment for the child, on top of the inherited process environment.
    ///
    /// Conflux adds no provider-specific variable here: credentials belong to
    /// the command.
    pub envs: HashMap<String, String>,
    /// Bounded wall-clock timeout for the invocation.
    pub timeout: Duration,
    /// Bounded byte limit applied independently to collected stdout and stderr.
    pub max_output_bytes: usize,
}

/// Raw bounded stdout of a completed judge invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeCommandOutput {
    /// Complete stdout, guaranteed to be within the configured output limit.
    pub stdout: String,
    /// Measured invocation duration.
    pub duration: Duration,
}

/// A bounded judge failure with the duration already measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgeCommandFailure {
    pub category: JudgeFailureCategory,
    pub duration: Duration,
}

/// Result of one bounded judge invocation.
pub type JudgeCommandResult = std::result::Result<JudgeCommandOutput, JudgeCommandFailure>;

/// What a caller got back when it stopped racing a judge against its own work.
pub enum JudgeCollection {
    /// The judge finished on its own before the caller's deadline.
    Completed(JudgeCommandResult),
    /// The judge was still running and its complete ownership was transferred to
    /// a detached cleanup task.
    ///
    /// The join handle is returned for tests that need to observe cleanup; the
    /// production caller drops it, which detaches the task exactly as intended.
    Cancelled(JoinHandle<JudgeCommandResult>),
}

/// Owner of one running judge invocation.
///
/// The invocation runs in its own task from the start, so it makes progress
/// concurrently with the caller's authoritative work and needs no polling.
/// Dropping the handle without collecting is itself a cancellation: the dropped
/// sender closes the channel, which the invocation task observes exactly like an
/// explicit cancel. `kill_on_drop` on the child is the further backstop for
/// runtime shutdown.
pub struct JudgeHandle {
    task: JoinHandle<JudgeCommandResult>,
    cancel: oneshot::Sender<()>,
}

impl JudgeHandle {
    /// Whether the invocation has already finished.
    // Part of the handle's lifecycle surface; read by boundary coverage rather
    // than by the binary, which only ever collects or cancels.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    /// Stop racing the judge.
    ///
    /// Returns the already-available result when the judge finished on its own.
    /// Otherwise it signals cancellation and hands the complete child,
    /// process-group, and output-drain ownership to the still-running task,
    /// which performs TERM → fixed grace → KILL → reap on its own. The caller
    /// never awaits that work, so an unavailable or slow judge cannot add
    /// latency to the authoritative path.
    pub async fn collect_or_cancel(self) -> JudgeCollection {
        let Self { task, cancel } = self;

        if task.is_finished() {
            // An already-finished task yields immediately; this await never blocks.
            let result = join_result(task.await);
            drop(cancel);
            return JudgeCollection::Completed(result);
        }

        let _ = cancel.send(());
        JudgeCollection::Cancelled(task)
    }

    /// Await the judge's own outcome without ever cancelling it.
    ///
    /// Only the invocation's own bounded timeout ends it. The cancel sender is
    /// deliberately held alive across the await so the task is not stopped by a
    /// dropped channel.
    // The analyzer never waits for a judge on purpose — this is how the
    // invocation's own timeout, exit-code, and output contracts are exercised.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn wait_for_completion(self) -> JudgeCommandResult {
        let Self { task, cancel } = self;
        let result = join_result(task.await);
        drop(cancel);
        result
    }
}

fn join_result(
    joined: std::result::Result<JudgeCommandResult, tokio::task::JoinError>,
) -> JudgeCommandResult {
    joined.unwrap_or(Err(JudgeCommandFailure {
        category: JudgeFailureCategory::Io,
        duration: Duration::ZERO,
    }))
}

/// Start one bounded judge invocation.
///
/// The request is written to the child's stdin verbatim; the caller has already
/// enforced the configured input limit, which is why no size check happens here.
pub fn spawn_judge(spec: JudgeCommandSpec, request: String) -> JudgeHandle {
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let task = tokio::spawn(run_judge(spec, request, cancel_rx));
    JudgeHandle {
        task,
        cancel: cancel_tx,
    }
}

/// Signal/reap surface used by the bounded cleanup sequence.
///
/// Kept behind a trait so the ordering decisions — TERM first, one fixed grace,
/// KILL only when the grace expired, always reap — are verifiable with an
/// in-memory double instead of real signals and wall-clock waits.
#[async_trait::async_trait]
pub(crate) trait JudgeProcessControl: Send {
    /// Send TERM to the judge's process group. Returns whether it was delivered.
    fn signal_term(&mut self) -> bool;
    /// Send KILL to the judge's process group. Returns whether it was delivered.
    async fn signal_kill(&mut self) -> bool;
    /// Wait up to `within` for the child to exit and be reaped.
    async fn reap_within(&mut self, within: Duration) -> bool;
}

/// What a cleanup sequence actually did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JudgeCleanupReport {
    pub term_sent: bool,
    pub force_killed: bool,
    pub reaped: bool,
}

/// Run the mandatory cleanup sequence for a judge that must stop now.
///
/// TERM immediately, then exactly one [`JUDGE_CLEANUP_GRACE`] window, then KILL
/// if the group still lives, and a reap either way. The sequence never
/// concludes anything from elapsed time alone: only an observed exit counts as
/// reaped.
pub(crate) async fn run_judge_cleanup<C>(control: &mut C) -> JudgeCleanupReport
where
    C: JudgeProcessControl + ?Sized,
{
    let term_sent = control.signal_term();
    if control.reap_within(JUDGE_CLEANUP_GRACE).await {
        return JudgeCleanupReport {
            term_sent,
            force_killed: false,
            reaped: true,
        };
    }

    let force_killed = control.signal_kill().await;
    let reaped = control.reap_within(JUDGE_REAP_WINDOW).await;
    JudgeCleanupReport {
        term_sent,
        force_killed,
        reaped,
    }
}

/// Real process-group control over a spawned judge child.
struct ManagedJudgeChild {
    child: ManagedChild,
}

#[async_trait::async_trait]
impl JudgeProcessControl for ManagedJudgeChild {
    fn signal_term(&mut self) -> bool {
        self.child.terminate().is_ok()
    }

    async fn signal_kill(&mut self) -> bool {
        self.child.force_kill().await.is_ok()
    }

    async fn reap_within(&mut self, within: Duration) -> bool {
        matches!(
            tokio::time::timeout(within, self.child.wait()).await,
            Ok(Ok(_))
        )
    }
}

/// Outcome of one guarded step of the invocation.
enum Guarded<T> {
    Done(T),
    TimedOut,
    Cancelled,
}

/// Await `fut`, but never past the invocation deadline or an explicit cancel.
///
/// Cancellation is biased ahead of both the deadline and the work: once the
/// authoritative path has finished, there is nothing left for the judge to win.
/// A closed channel counts as cancellation, so dropping the handle stops the
/// invocation too.
async fn guarded<T, F>(
    fut: F,
    deadline: tokio::time::Instant,
    cancel: &mut oneshot::Receiver<()>,
) -> Guarded<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        biased;
        _ = &mut *cancel => Guarded::Cancelled,
        _ = tokio::time::sleep_until(deadline) => Guarded::TimedOut,
        value = fut => Guarded::Done(value),
    }
}

/// Bounded read of one child pipe.
struct DrainedStream {
    bytes: Vec<u8>,
    overflowed: bool,
}

/// Read a child pipe until EOF or until the byte limit is exceeded.
///
/// Reading (rather than ignoring) the stream is what keeps the child from
/// blocking on a full pipe while Conflux is writing its request. Once the limit
/// is exceeded the read stops and the pipe is dropped, which is also the signal
/// the caller uses to terminate the invocation rather than keep collecting.
async fn drain_bounded<R>(reader: Option<R>, limit: usize) -> DrainedStream
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut collected = Vec::new();
    let Some(mut reader) = reader else {
        return DrainedStream {
            bytes: collected,
            overflowed: false,
        };
    };

    let mut chunk = vec![0u8; DRAIN_CHUNK_BYTES];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => {
                return DrainedStream {
                    bytes: collected,
                    overflowed: false,
                }
            }
            Ok(read) => {
                collected.extend_from_slice(&chunk[..read]);
                if collected.len() > limit {
                    return DrainedStream {
                        bytes: collected,
                        overflowed: true,
                    };
                }
            }
            Err(_) => {
                return DrainedStream {
                    bytes: collected,
                    overflowed: false,
                }
            }
        }
    }
}

async fn run_judge(
    spec: JudgeCommandSpec,
    request: String,
    mut cancel: oneshot::Receiver<()>,
) -> JudgeCommandResult {
    let started = std::time::Instant::now();
    let deadline = tokio::time::Instant::now() + spec.timeout;
    let fail = |category: JudgeFailureCategory| {
        Err(JudgeCommandFailure {
            category,
            duration: started.elapsed(),
        })
    };

    let Some((executable, arguments)) = spec.argv.split_first() else {
        // Configuration validation rejects an empty argv; this keeps the runner
        // safe even when it is driven directly.
        return fail(JudgeFailureCategory::Spawn);
    };

    let mut command = tokio::process::Command::new(executable);
    command
        .args(arguments)
        .current_dir(&spec.working_dir)
        .envs(&spec.envs)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Runtime shutdown must not be able to orphan a judge.
        .kill_on_drop(true);
    configure_process_group(&mut command);

    let mut spawned = match command.spawn() {
        Ok(child) => child,
        Err(_) => return fail(JudgeFailureCategory::Spawn),
    };

    let stdin = spawned.stdin.take();
    let stdout = spawned.stdout.take();
    let stderr = spawned.stderr.take();
    let child = match ManagedChild::new(spawned) {
        Ok(child) => child,
        Err(_) => return fail(JudgeFailureCategory::Io),
    };
    let mut control = ManagedJudgeChild { child };

    // Held as `Option`s so a drain is joined or reaped exactly once: awaiting a
    // `JoinHandle` that already completed is a panic, not a no-op.
    let mut stdout_task = Some(tokio::spawn(drain_bounded(stdout, spec.max_output_bytes)));
    let mut stderr_task = Some(tokio::spawn(drain_bounded(stderr, spec.max_output_bytes)));

    // Exact request delivery, then an explicit stdin close so the command can
    // read to EOF.
    let write = async {
        let mut stdin = stdin.ok_or(())?;
        stdin.write_all(request.as_bytes()).await.map_err(|_| ())?;
        stdin.flush().await.map_err(|_| ())?;
        drop(stdin);
        Ok::<(), ()>(())
    };

    // Cleanup is mandatory on every non-success path: the child's process group
    // is stopped and reaped, and any still-running drain is aborted and reaped
    // with its partial output discarded.
    macro_rules! stop {
        ($category:expr) => {{
            run_judge_cleanup(&mut control).await;
            for drain in [stdout_task.take(), stderr_task.take()] {
                if let Some(drain) = drain {
                    drain.abort();
                    let _ = drain.await;
                }
            }
            return fail($category);
        }};
    }

    match guarded(write, deadline, &mut cancel).await {
        Guarded::Done(Ok(())) => {}
        Guarded::Done(Err(())) => stop!(JudgeFailureCategory::Io),
        Guarded::TimedOut => stop!(JudgeFailureCategory::Timeout),
        Guarded::Cancelled => stop!(JudgeFailureCategory::Cancelled),
    }

    let joined = {
        let out = stdout_task.as_mut().expect("stdout drain is live");
        let err = stderr_task.as_mut().expect("stderr drain is live");
        guarded(async { tokio::join!(out, err) }, deadline, &mut cancel).await
    };
    let (stdout_drain, stderr_drain) = match joined {
        Guarded::Done(drained) => {
            // Both drains are finished; drop the handles so the cleanup path can
            // never await them a second time.
            stdout_task.take();
            stderr_task.take();
            drained
        }
        Guarded::TimedOut => stop!(JudgeFailureCategory::Timeout),
        Guarded::Cancelled => stop!(JudgeFailureCategory::Cancelled),
    };
    let (Ok(stdout_drain), Ok(stderr_drain)) = (stdout_drain, stderr_drain) else {
        stop!(JudgeFailureCategory::Io)
    };

    // Oversized output is rejected rather than truncated, and the command is
    // stopped immediately instead of being allowed to keep writing.
    if stdout_drain.overflowed || stderr_drain.overflowed {
        stop!(JudgeFailureCategory::OutputTooLarge)
    }

    let status = match guarded(control.child.wait(), deadline, &mut cancel).await {
        Guarded::Done(Ok(status)) => status,
        Guarded::Done(Err(_)) => stop!(JudgeFailureCategory::Io),
        Guarded::TimedOut => stop!(JudgeFailureCategory::Timeout),
        Guarded::Cancelled => stop!(JudgeFailureCategory::Cancelled),
    };

    if !status.success() {
        return fail(JudgeFailureCategory::from_exit_code(status.code()));
    }

    // stderr is bounded and discarded here: it is never parsed as an answer and
    // never logged verbatim.
    let Ok(stdout) = String::from_utf8(stdout_drain.bytes) else {
        return fail(JudgeFailureCategory::InvalidJson);
    };

    Ok(JudgeCommandOutput {
        stdout,
        duration: started.elapsed(),
    })
}

// ── System One wire contract ───────────────────────────────────────────────

/// Schema version of the Conflux-supplied `state` payload.
pub const JUDGE_STATE_SCHEMA_VERSION: u32 = 1;

/// Question type of every judgment Conflux asks for.
pub const NOUL_QUESTION_TYPE: &str = "noul";

/// One complete System One request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SystemOneRequest {
    pub state: JudgeRequestState,
    pub model: String,
    /// Question map keyed by deterministic index-derived IDs.
    ///
    /// A [`BTreeMap`] keeps serialization order stable so the same working set
    /// always produces byte-identical request JSON.
    pub questions: BTreeMap<String, NoulQuestion>,
}

/// Structured working-set state carried with the request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JudgeRequestState {
    pub schema_version: u32,
    pub queued_changes: Vec<JudgeChangeNode>,
    pub in_flight_changes: Vec<JudgeChangeNode>,
}

/// One change in the request's working set.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JudgeChangeNode {
    /// Trusted integer index; the only thing question IDs are derived from.
    pub index: usize,
    /// Change ID, carried as a value and never as an object key.
    pub id: String,
    /// Complete `proposal.md` text.
    pub proposal: String,
    /// Dependencies parsed from proposal metadata, which stay authoritative
    /// outside the judge.
    pub metadata_dependencies: Vec<String>,
    pub priority: Option<String>,
    pub references: Vec<String>,
}

/// One Noul question.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoulQuestion {
    #[serde(rename = "type")]
    pub question_type: String,
    pub instructions: String,
    pub criteria: NoulCriteria,
}

/// Optional Noul true/false criteria from the System One question contract.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoulCriteria {
    #[serde(rename = "true")]
    pub when_true: String,
    #[serde(rename = "false")]
    pub when_false: String,
}

/// Token usage reported by the command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgeUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// A strictly validated System One response.
#[derive(Debug, Clone, PartialEq)]
pub struct JudgeResponse {
    /// Model identity, already proven equal to the requested model.
    pub model: String,
    /// Noul probability per question ID.
    pub values: BTreeMap<String, f64>,
    pub usage: JudgeUsage,
}

/// Parse and validate one System One response.
///
/// Every rule below is a hard rejection, because a partially understood judge
/// answer is worth less than no answer at all:
///
/// - stdout contains exactly one JSON value, followed only by whitespace;
/// - the top level is an object with a string `model`, an object `answers`, and
///   an object `usage`;
/// - the returned model equals the requested model;
/// - the answer IDs equal the expected ID set exactly — no missing, no extra;
/// - every answer has `type: "noul"` and a finite `noul` value in `[0, 1]`;
/// - `usage.input_tokens` and `usage.output_tokens` are non-negative integers.
///
/// Noul carries no separate confidence field, so none is read. Duplicate
/// object-key rejection is deliberately not attempted: normal JSON decoding
/// keeps the last occurrence, and the exact expected answer-ID check already
/// makes a semantically duplicated answer set impossible to accept.
pub fn parse_system_one_response(
    stdout: &str,
    expected_model: &str,
    expected_ids: &BTreeSet<String>,
) -> std::result::Result<JudgeResponse, JudgeFailureCategory> {
    let mut stream = serde_json::Deserializer::from_str(stdout).into_iter::<serde_json::Value>();
    let Some(Ok(value)) = stream.next() else {
        return Err(JudgeFailureCategory::InvalidJson);
    };
    // Exactly one value: anything other than trailing whitespace — a second
    // JSON value, prose, or a stray fence — is a rejection.
    if !stdout[stream.byte_offset()..].trim().is_empty() {
        return Err(JudgeFailureCategory::InvalidJson);
    }

    let object = value
        .as_object()
        .ok_or(JudgeFailureCategory::SchemaMismatch)?;

    let model = object
        .get("model")
        .and_then(serde_json::Value::as_str)
        .ok_or(JudgeFailureCategory::SchemaMismatch)?;
    if model != expected_model {
        return Err(JudgeFailureCategory::SchemaMismatch);
    }

    let usage_object = object
        .get("usage")
        .and_then(serde_json::Value::as_object)
        .ok_or(JudgeFailureCategory::SchemaMismatch)?;
    let mut usage_counters = [0u64; 2];
    for (slot, field) in ["input_tokens", "output_tokens"].iter().enumerate() {
        usage_counters[slot] = usage_object
            .get(*field)
            .and_then(serde_json::Value::as_u64)
            .ok_or(JudgeFailureCategory::SchemaMismatch)?;
    }

    let answers = object
        .get("answers")
        .and_then(serde_json::Value::as_object)
        .ok_or(JudgeFailureCategory::SchemaMismatch)?;
    if answers.len() != expected_ids.len() {
        return Err(JudgeFailureCategory::SchemaMismatch);
    }

    let mut values = BTreeMap::new();
    for id in expected_ids {
        let answer = answers
            .get(id)
            .and_then(serde_json::Value::as_object)
            .ok_or(JudgeFailureCategory::SchemaMismatch)?;
        if answer.get("type").and_then(serde_json::Value::as_str) != Some(NOUL_QUESTION_TYPE) {
            return Err(JudgeFailureCategory::SchemaMismatch);
        }
        let noul = answer
            .get(NOUL_QUESTION_TYPE)
            .and_then(serde_json::Value::as_f64)
            .ok_or(JudgeFailureCategory::SchemaMismatch)?;
        if !noul.is_finite() || !(0.0..=1.0).contains(&noul) {
            return Err(JudgeFailureCategory::SchemaMismatch);
        }
        values.insert(id.clone(), noul);
    }

    Ok(JudgeResponse {
        model: model.to_string(),
        values,
        usage: JudgeUsage {
            input_tokens: usage_counters[0],
            output_tokens: usage_counters[1],
        },
    })
}

#[cfg(test)]
mod cleanup_sequence_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// In-memory process double: no signals, no real child, no wall clock.
    struct FakeControl {
        /// Whether the double reports an exit within the TERM grace window.
        exits_on_term: bool,
        /// Whether the double reports an exit after KILL.
        exits_on_kill: bool,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl FakeControl {
        fn new(exits_on_term: bool, exits_on_kill: bool) -> (Self, Arc<Mutex<Vec<String>>>) {
            let calls = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    exits_on_term,
                    exits_on_kill,
                    calls: calls.clone(),
                },
                calls,
            )
        }

        fn record(&self, call: &str) {
            self.calls.lock().expect("call log").push(call.to_string());
        }
    }

    #[async_trait::async_trait]
    impl JudgeProcessControl for FakeControl {
        fn signal_term(&mut self) -> bool {
            self.record("term");
            true
        }

        async fn signal_kill(&mut self) -> bool {
            self.record("kill");
            true
        }

        async fn reap_within(&mut self, within: Duration) -> bool {
            self.record(&format!("reap_within:{}ms", within.as_millis()));
            if within == JUDGE_CLEANUP_GRACE {
                self.exits_on_term
            } else {
                self.exits_on_kill
            }
        }
    }

    #[tokio::test]
    async fn cooperative_judge_is_reaped_without_a_kill() {
        let (mut control, calls) = FakeControl::new(true, true);

        let report = run_judge_cleanup(&mut control).await;

        assert_eq!(
            report,
            JudgeCleanupReport {
                term_sent: true,
                force_killed: false,
                reaped: true,
            }
        );
        assert_eq!(
            *calls.lock().expect("call log"),
            vec!["term".to_string(), "reap_within:100ms".to_string()],
            "a judge that exits inside the grace window must never be killed"
        );
    }

    #[tokio::test]
    async fn stubborn_judge_is_killed_after_the_fixed_grace_and_reaped() {
        let (mut control, calls) = FakeControl::new(false, true);

        let report = run_judge_cleanup(&mut control).await;

        assert_eq!(
            report,
            JudgeCleanupReport {
                term_sent: true,
                force_killed: true,
                reaped: true,
            }
        );
        let calls = calls.lock().expect("call log").clone();
        assert_eq!(
            calls,
            vec![
                "term".to_string(),
                "reap_within:100ms".to_string(),
                "kill".to_string(),
                format!("reap_within:{}ms", JUDGE_REAP_WINDOW.as_millis()),
            ],
            "TERM must precede exactly one grace window, then KILL, then a reap"
        );
    }

    #[tokio::test]
    async fn unreapable_judge_is_reported_rather_than_assumed_gone() {
        let (mut control, _calls) = FakeControl::new(false, false);

        let report = run_judge_cleanup(&mut control).await;

        assert_eq!(
            report,
            JudgeCleanupReport {
                term_sent: true,
                force_killed: true,
                reaped: false,
            },
            "elapsed time alone never proves a reap"
        );
    }

    #[test]
    fn cleanup_grace_is_the_fixed_contract_value() {
        assert_eq!(JUDGE_CLEANUP_GRACE, Duration::from_millis(100));
    }
}

#[cfg(test)]
mod failure_category_tests {
    use super::*;

    #[test]
    fn baseline_exit_codes_map_to_dedicated_categories() {
        assert_eq!(
            JudgeFailureCategory::from_exit_code(Some(2)),
            JudgeFailureCategory::InvalidRequest
        );
        assert_eq!(
            JudgeFailureCategory::from_exit_code(Some(3)),
            JudgeFailureCategory::Authentication
        );
        assert_eq!(
            JudgeFailureCategory::from_exit_code(Some(4)),
            JudgeFailureCategory::Transient
        );
        assert_eq!(
            JudgeFailureCategory::from_exit_code(Some(1)),
            JudgeFailureCategory::NonzeroExit
        );
        assert_eq!(
            JudgeFailureCategory::from_exit_code(None),
            JudgeFailureCategory::NonzeroExit
        );
    }

    #[test]
    fn category_tokens_are_stable_and_content_free() {
        for (category, token) in [
            (JudgeFailureCategory::Spawn, "spawn"),
            (JudgeFailureCategory::InputTooLarge, "input_too_large"),
            (JudgeFailureCategory::Timeout, "timeout"),
            (JudgeFailureCategory::Cancelled, "cancelled"),
            (JudgeFailureCategory::NonzeroExit, "nonzero_exit"),
            (JudgeFailureCategory::InvalidRequest, "invalid_request"),
            (JudgeFailureCategory::Authentication, "authentication"),
            (JudgeFailureCategory::Transient, "transient"),
            (JudgeFailureCategory::OutputTooLarge, "output_too_large"),
            (JudgeFailureCategory::InvalidJson, "invalid_json"),
            (JudgeFailureCategory::SchemaMismatch, "schema_mismatch"),
            (JudgeFailureCategory::Io, "io"),
        ] {
            assert_eq!(category.as_str(), token);
        }
    }
}

#[cfg(test)]
mod response_parser_tests {
    use super::*;

    /// Fixtures are checked in and derived from the immutable `jev-cli v0.4.1`
    /// tag commit `50488f35d705051b5d621d1d10e69d1503783e7d`, never from
    /// repository `main` or live API output.
    const VALID_COMPACT: &str =
        include_str!("../tests/fixtures/judge_command/response_valid_compact.json");
    const VALID_PRETTY: &str =
        include_str!("../tests/fixtures/judge_command/response_valid_pretty.json");
    const REQUEST_BASELINE: &str =
        include_str!("../tests/fixtures/judge_command/request_baseline.json");
    const ERROR_STDERR: &str =
        include_str!("../tests/fixtures/judge_command/error_stderr_invalid_request.json");

    fn expected_ids() -> BTreeSet<String> {
        ["q_0000_0001".to_string(), "q_0001_0000".to_string()]
            .into_iter()
            .collect()
    }

    fn single_id() -> BTreeSet<String> {
        ["q_0000_0001".to_string()].into_iter().collect()
    }

    #[test]
    fn baseline_compact_response_is_accepted() {
        let parsed = parse_system_one_response(VALID_COMPACT, "jev-1.13.0", &expected_ids())
            .expect("the baseline compact response must parse");

        assert_eq!(parsed.model, "jev-1.13.0");
        assert_eq!(parsed.values.get("q_0000_0001"), Some(&0.93));
        assert_eq!(parsed.values.get("q_0001_0000"), Some(&0.02));
        assert_eq!(
            parsed.usage,
            JudgeUsage {
                input_tokens: 123,
                output_tokens: 8,
            }
        );
    }

    #[test]
    fn baseline_pretty_response_is_accepted() {
        let parsed = parse_system_one_response(VALID_PRETTY, "jev-1.13.0", &single_id())
            .expect("the baseline pretty response must parse");

        assert_eq!(parsed.values.get("q_0000_0001"), Some(&0.5));
    }

    #[test]
    fn baseline_request_fixture_matches_the_serialized_request_shape() {
        // The request fixture is what a compatible command must accept on stdin:
        // structured state, a pinned model, and Noul questions with optional
        // true/false criteria.
        let value: serde_json::Value =
            serde_json::from_str(REQUEST_BASELINE).expect("request fixture must be valid JSON");

        assert_eq!(value["model"], "jev-1.13.0");
        assert_eq!(value["state"]["schema_version"], 1);
        assert_eq!(value["questions"]["q_0000_0001"]["type"], "noul");
        assert!(value["questions"]["q_0000_0001"]["criteria"]["true"]
            .as_str()
            .is_some_and(|text| !text.is_empty()));
        assert!(value["questions"]["q_0000_0001"]["criteria"]["false"]
            .as_str()
            .is_some_and(|text| !text.is_empty()));

        let request: SystemOneRequest = SystemOneRequest {
            state: JudgeRequestState {
                schema_version: JUDGE_STATE_SCHEMA_VERSION,
                queued_changes: vec![JudgeChangeNode {
                    index: 0,
                    id: "change-a".to_string(),
                    proposal: "complete proposal.md text".to_string(),
                    metadata_dependencies: Vec::new(),
                    priority: Some("medium".to_string()),
                    references: Vec::new(),
                }],
                in_flight_changes: vec![JudgeChangeNode {
                    index: 1,
                    id: "change-b".to_string(),
                    proposal: "complete proposal.md text".to_string(),
                    metadata_dependencies: Vec::new(),
                    priority: None,
                    references: Vec::new(),
                }],
            },
            model: "jev-1.13.0".to_string(),
            questions: BTreeMap::from([(
                "q_0000_0001".to_string(),
                NoulQuestion {
                    question_type: NOUL_QUESTION_TYPE.to_string(),
                    instructions: value["questions"]["q_0000_0001"]["instructions"]
                        .as_str()
                        .expect("fixture instructions")
                        .to_string(),
                    criteria: NoulCriteria {
                        when_true: value["questions"]["q_0000_0001"]["criteria"]["true"]
                            .as_str()
                            .expect("fixture true criteria")
                            .to_string(),
                        when_false: value["questions"]["q_0000_0001"]["criteria"]["false"]
                            .as_str()
                            .expect("fixture false criteria")
                            .to_string(),
                    },
                },
            )]),
        };

        assert_eq!(
            serde_json::to_value(&request).expect("request must serialize"),
            value,
            "the serialized request type must stay byte-compatible with the baseline fixture"
        );
    }

    #[test]
    fn baseline_structured_stderr_is_never_an_answer_source() {
        // Structured errors go to stderr with a non-zero exit; treating that
        // payload as stdout must not produce a judgment.
        assert_eq!(
            parse_system_one_response(ERROR_STDERR, "jev-1.13.0", &single_id()),
            Err(JudgeFailureCategory::SchemaMismatch)
        );
    }

    #[test]
    fn malformed_and_out_of_contract_responses_are_rejected() {
        let cases: Vec<(&str, &str, JudgeFailureCategory)> = vec![
            ("empty stdout", "", JudgeFailureCategory::InvalidJson),
            (
                "prose before JSON",
                r#"Here you go: {"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.9}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::InvalidJson,
            ),
            (
                "trailing non-whitespace",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.9}},"usage":{"input_tokens":1,"output_tokens":1}} done"#,
                JudgeFailureCategory::InvalidJson,
            ),
            (
                "two top-level values",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.9}},"usage":{"input_tokens":1,"output_tokens":1}}{"model":"jev-1.13.0"}"#,
                JudgeFailureCategory::InvalidJson,
            ),
            (
                "truncated JSON",
                r#"{"model":"jev-1.13.0","answers":{"#,
                JudgeFailureCategory::InvalidJson,
            ),
            (
                "non-object root",
                r#"[{"model":"jev-1.13.0"}]"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "wrong model",
                r#"{"model":"jev-9.9.9","answers":{"q_0000_0001":{"type":"noul","noul":0.9}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "missing model",
                r#"{"answers":{"q_0000_0001":{"type":"noul","noul":0.9}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "missing answer id",
                r#"{"model":"jev-1.13.0","answers":{},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "extra answer id",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.9},"q_9999_9999":{"type":"noul","noul":0.1}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "renamed answer id",
                r#"{"model":"jev-1.13.0","answers":{"change-a::change-b":{"type":"noul","noul":0.9}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "wrong answer type",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"choice","noul":0.9}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "answer value is a string",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":"0.9"}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "negative noul",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":-0.1}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "noul greater than one",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":1.0001}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                // An overflowing exponent is refused by JSON decoding itself,
                // before the range check ever sees a value.
                "non-finite noul",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":1e999}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::InvalidJson,
            ),
            (
                "NaN literal",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":NaN}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
                JudgeFailureCategory::InvalidJson,
            ),
            (
                "negative usage",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.9}},"usage":{"input_tokens":-1,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "fractional usage",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.9}},"usage":{"input_tokens":1.5,"output_tokens":1}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
            (
                "missing usage",
                r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.9}}}"#,
                JudgeFailureCategory::SchemaMismatch,
            ),
        ];

        for (label, stdout, expected) in cases {
            assert_eq!(
                parse_system_one_response(stdout, "jev-1.13.0", &single_id()),
                Err(expected),
                "{label} must be rejected as {expected:?}"
            );
        }
    }

    #[test]
    fn boundary_noul_values_are_accepted() {
        for value in ["0", "0.0", "1", "1.0"] {
            let stdout = format!(
                r#"{{"model":"jev-1.13.0","answers":{{"q_0000_0001":{{"type":"noul","noul":{value}}}}},"usage":{{"input_tokens":0,"output_tokens":0}}}}"#
            );
            assert!(
                parse_system_one_response(&stdout, "jev-1.13.0", &single_id()).is_ok(),
                "noul {value} is inside the closed [0,1] range"
            );
        }
    }

    #[test]
    fn no_confidence_field_is_required_or_read() {
        // TypeSafe Noul has no separate confidence field, so a response that
        // omits one is fully valid and one that invents one changes nothing.
        let without = r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.7}},"usage":{"input_tokens":1,"output_tokens":1}}"#;
        let with = r#"{"model":"jev-1.13.0","answers":{"q_0000_0001":{"type":"noul","noul":0.7,"confidence":0.1}},"usage":{"input_tokens":1,"output_tokens":1}}"#;

        let parsed_without = parse_system_one_response(without, "jev-1.13.0", &single_id())
            .expect("a response without a confidence field must be valid");
        let parsed_with = parse_system_one_response(with, "jev-1.13.0", &single_id())
            .expect("an extra field inside an answer is ignored");

        assert_eq!(parsed_without.values, parsed_with.values);
    }
}

#[cfg(test)]
mod operator_documentation_tests {
    //! The operator-facing contract is part of this boundary, so it is asserted
    //! rather than assumed. The guide is read at test time from the repository
    //! (not embedded) so the published crate does not have to carry `docs/`.

    fn config_guide() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/guides/CONFIG.md");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{} must be readable: {err}", path.display()))
    }

    #[test]
    fn config_guide_documents_the_product_neutral_judge_command() {
        let guide = config_guide();

        for required in [
            // Key, purpose, and the tracked example entry.
            "`judge_commands`",
            "parallel_dependency",
            r#""command": ["jev", "run", "-"]"#,
            r#""model": "jev-1.13.0""#,
            "`max_input_bytes`",
            "`max_output_bytes`",
            "`yes_threshold`",
            // Compatibility baseline, as an immutable release URL and commit.
            "https://github.com/tumf/jev-cli/releases/tag/v0.4.1",
            "50488f35d705051b5d621d1d10e69d1503783e7d",
            "not\nrepository `main`",
            // Protocol and exit-code contract.
            "stdin",
            "stdout",
            "exit `2`",
            // Concrete-model requirement.
            "-latest",
            // Credential ownership and request sizing.
            "TYPESAFE_API_KEY",
            "Conflux passes no\nprovider API key",
            "max_input_bytes`. Conflux then",
            // Shadow-only and fallback guarantees.
            "non-authoritative observer",
            "100 ms",
            "`blocked`, `stalled`, or `failed`",
            "atomic per purpose",
        ] {
            assert!(
                guide.contains(required),
                "docs/guides/CONFIG.md must document {required:?}"
            );
        }
    }

    #[test]
    fn tracked_example_entry_parses_as_a_valid_judge_configuration() {
        // The documented example is exactly what an operator copies, so it has
        // to load and validate rather than merely read well.
        let parsed: crate::config::OrchestratorConfig = serde_json::from_str(
            r#"{"judge_commands": {"parallel_dependency": {
                   "command": ["jev", "run", "-"],
                   "model": "jev-1.13.0",
                   "timeout_ms": 30000,
                   "max_input_bytes": 1048576,
                   "max_output_bytes": 1048576,
                   "yes_threshold": 0.85
                 }}}"#,
        )
        .expect("the documented example must parse");

        let judge = parsed
            .get_parallel_dependency_judge()
            .expect("the documented example configures the purpose");
        assert!(judge.validate().is_ok());
        assert_eq!(judge.command, vec!["jev", "run", "-"]);
        assert_eq!(judge.timeout_ms(), 30_000);
        assert_eq!(judge.yes_threshold(), 0.85);
    }

    #[test]
    fn compatibility_fixtures_are_tracked_with_their_immutable_baseline() {
        let readme = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/judge_command/README.md"),
        )
        .expect("fixture README must be tracked");

        assert!(readme.contains("50488f35d705051b5d621d1d10e69d1503783e7d"));
        assert!(readme.contains("https://github.com/tumf/jev-cli/releases/tag/v0.4.1"));
        assert!(
            readme.contains("not repository `main`") || readme.contains("not\nrepository `main`"),
            "the fixture baseline must state that repository `main` is not a baseline"
        );
    }
}

#[cfg(all(test, unix))]
mod judge_process_tests {
    //! Causal fake-executable coverage for the process boundary.
    //!
    //! These exercise a real child process on purpose: spawn failure, exit-code
    //! classification, exact stdin delivery, bounded drains, timeout,
    //! cancellation, and reaping are properties of the boundary itself, so the
    //! decision logic they surround is what the in-memory
    //! [`super::cleanup_sequence_tests`] doubles cover.

    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct FakeJudge {
        _dir: tempfile::TempDir,
        path: PathBuf,
        request_sink: PathBuf,
    }

    impl FakeJudge {
        /// Write an executable shell script whose body is the fake judge.
        ///
        /// `$REQUEST_SINK` is where the script must copy its stdin so a test can
        /// prove the exact request arrived.
        fn new(body: &str) -> Self {
            let dir = tempfile::TempDir::new().expect("fake judge dir");
            let path = dir.path().join("fake-judge");
            let request_sink = dir.path().join("request.json");
            let script = format!(
                "#!/bin/sh\nREQUEST_SINK='{}'\n{body}\n",
                request_sink.display()
            );
            std::fs::write(&path, script).expect("write fake judge");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod fake judge");
            Self {
                _dir: dir,
                path,
                request_sink,
            }
        }

        fn spec(&self, timeout: Duration, max_output_bytes: usize) -> JudgeCommandSpec {
            JudgeCommandSpec {
                argv: vec![self.path.display().to_string()],
                working_dir: self._dir.path().to_path_buf(),
                envs: HashMap::new(),
                timeout,
                max_output_bytes,
            }
        }

        fn captured_request(&self) -> String {
            std::fs::read_to_string(&self.request_sink).unwrap_or_default()
        }
    }

    fn category(result: &JudgeCommandResult) -> Option<JudgeFailureCategory> {
        result.as_ref().err().map(|failure| failure.category)
    }

    /// Wait for the judge's own completion, then collect it the way the analyzer
    /// does once its authoritative work is done.
    async fn collect_after_completion(handle: JudgeHandle) -> JudgeCollection {
        while !handle.is_finished() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        handle.collect_or_cancel().await
    }

    #[tokio::test]
    async fn successful_judge_returns_bounded_stdout_and_receives_the_exact_request() {
        let judge = FakeJudge::new(
            r#"cat > "$REQUEST_SINK"
printf '%s' '{"model":"m","answers":{},"usage":{"input_tokens":1,"output_tokens":1}}'"#,
        );
        let request = r#"{"state":{"schema_version":1},"model":"m","questions":{}}"#.to_string();

        let handle = spawn_judge(judge.spec(Duration::from_secs(5), 4096), request.clone());
        let result = match collect_after_completion(handle).await {
            JudgeCollection::Completed(result) => result,
            JudgeCollection::Cancelled(_) => panic!("a completed judge must not be cancelled"),
        };

        let output = result.expect("a zero-exit judge must succeed");
        assert_eq!(
            output.stdout,
            r#"{"model":"m","answers":{},"usage":{"input_tokens":1,"output_tokens":1}}"#
        );
        assert_eq!(
            judge.captured_request(),
            request,
            "stdin must carry the request bytes exactly, with nothing appended"
        );
    }

    #[tokio::test]
    async fn missing_executable_is_a_spawn_failure() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let spec = JudgeCommandSpec {
            argv: vec![dir.path().join("not-installed").display().to_string()],
            working_dir: dir.path().to_path_buf(),
            envs: HashMap::new(),
            timeout: Duration::from_secs(5),
            max_output_bytes: 4096,
        };

        let result = spawn_judge(spec, "{}".to_string())
            .wait_for_completion()
            .await;

        assert_eq!(category(&result), Some(JudgeFailureCategory::Spawn));
    }

    #[tokio::test]
    async fn empty_argv_cannot_spawn() {
        let spec = JudgeCommandSpec {
            argv: Vec::new(),
            working_dir: std::env::temp_dir(),
            envs: HashMap::new(),
            timeout: Duration::from_secs(5),
            max_output_bytes: 4096,
        };

        let result = spawn_judge(spec, "{}".to_string())
            .wait_for_completion()
            .await;

        assert_eq!(category(&result), Some(JudgeFailureCategory::Spawn));
    }

    #[tokio::test]
    async fn baseline_exit_codes_are_classified_without_touching_the_caller() {
        for (code, expected) in [
            (1, JudgeFailureCategory::NonzeroExit),
            (2, JudgeFailureCategory::InvalidRequest),
            (3, JudgeFailureCategory::Authentication),
            (4, JudgeFailureCategory::Transient),
        ] {
            let judge = FakeJudge::new(&format!(
                r#"cat > "$REQUEST_SINK"
echo 'structured error' >&2
exit {code}"#
            ));

            let result = spawn_judge(judge.spec(Duration::from_secs(5), 4096), "{}".to_string())
                .wait_for_completion()
                .await;

            assert_eq!(
                category(&result),
                Some(expected),
                "exit {code} must classify as {expected:?}"
            );
        }
    }

    #[tokio::test]
    async fn oversized_stdout_is_rejected_rather_than_truncated() {
        let judge = FakeJudge::new(
            r#"cat > "$REQUEST_SINK"
i=0
while [ $i -lt 200 ]; do printf '0123456789'; i=$((i+1)); done"#,
        );

        let result = spawn_judge(judge.spec(Duration::from_secs(5), 16), "{}".to_string())
            .wait_for_completion()
            .await;

        assert_eq!(
            category(&result),
            Some(JudgeFailureCategory::OutputTooLarge)
        );
    }

    #[tokio::test]
    async fn oversized_stderr_is_rejected_too() {
        let judge = FakeJudge::new(
            r#"cat > "$REQUEST_SINK"
i=0
while [ $i -lt 200 ]; do printf '0123456789' >&2; i=$((i+1)); done
printf '%s' '{}'"#,
        );

        let result = spawn_judge(judge.spec(Duration::from_secs(5), 16), "{}".to_string())
            .wait_for_completion()
            .await;

        assert_eq!(
            category(&result),
            Some(JudgeFailureCategory::OutputTooLarge)
        );
    }

    #[tokio::test]
    async fn bounded_stderr_never_contaminates_stdout() {
        let judge = FakeJudge::new(
            r#"cat > "$REQUEST_SINK"
echo 'diagnostic that must never be parsed' >&2
printf '%s' 'RESPONSE'"#,
        );

        let output = spawn_judge(judge.spec(Duration::from_secs(5), 4096), "{}".to_string())
            .wait_for_completion()
            .await
            .expect("stderr output does not fail a zero-exit judge");

        assert_eq!(output.stdout, "RESPONSE");
    }

    #[tokio::test]
    async fn timeout_terminates_and_reaps_the_judge() {
        let judge = FakeJudge::new(
            r#"cat > "$REQUEST_SINK"
sleep 30"#,
        );

        let result = spawn_judge(
            judge.spec(Duration::from_millis(50), 4096),
            "{}".to_string(),
        )
        .wait_for_completion()
        .await;

        assert_eq!(category(&result), Some(JudgeFailureCategory::Timeout));
    }

    #[tokio::test]
    async fn cancellation_releases_the_caller_before_cleanup_finishes() {
        let judge = FakeJudge::new(
            r#"cat > "$REQUEST_SINK"
sleep 30"#,
        );
        // A 10-minute timeout the caller must never wait for.
        let handle = spawn_judge(judge.spec(Duration::from_secs(600), 4096), "{}".to_string());

        // Let the child actually start before cancelling, so cleanup has a real
        // process group to stop.
        while judge.captured_request().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let cleanup = match handle.collect_or_cancel().await {
            JudgeCollection::Cancelled(cleanup) => cleanup,
            JudgeCollection::Completed(_) => panic!("a sleeping judge cannot have completed"),
        };

        // The caller is already released here; awaiting the detached task is a
        // test-only observation of cleanup.
        let result = join_result(cleanup.await);
        assert_eq!(category(&result), Some(JudgeFailureCategory::Cancelled));
    }

    #[tokio::test]
    async fn a_term_ignoring_judge_is_killed_after_the_grace_and_reaped() {
        let judge = FakeJudge::new(
            r#"cat > "$REQUEST_SINK"
trap '' TERM
sleep 30"#,
        );
        let handle = spawn_judge(judge.spec(Duration::from_secs(600), 4096), "{}".to_string());
        while judge.captured_request().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let cleanup = match handle.collect_or_cancel().await {
            JudgeCollection::Cancelled(cleanup) => cleanup,
            JudgeCollection::Completed(_) => panic!("a sleeping judge cannot have completed"),
        };

        // Reaching this point at all is the assertion: a TERM-ignoring child is
        // escalated to KILL after the fixed grace and reaped, so the detached
        // task completes instead of hanging for the invocation timeout.
        let result = join_result(cleanup.await);
        assert_eq!(category(&result), Some(JudgeFailureCategory::Cancelled));
    }

    #[tokio::test]
    async fn dropping_the_handle_stops_the_judge() {
        let judge = FakeJudge::new(
            r#"cat > "$REQUEST_SINK"
sleep 30"#,
        );
        let handle = spawn_judge(judge.spec(Duration::from_secs(600), 4096), "{}".to_string());
        while judge.captured_request().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        drop(handle);

        // The invocation task observes the closed cancel channel and cleans up;
        // nothing is left for the caller to await.
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
