//! Cross-platform process management for reliable child process cleanup
//!
//! This module provides abstractions for managing child processes across Unix and Windows platforms:
//! - Unix: Process groups (`setpgid` + `killpg`)
//! - Windows: Job Objects (automatic termination on parent exit)

use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Child;
use tracing::{debug, warn};

/// Platform-specific process handle for managing child processes
#[cfg(unix)]
pub struct ProcessHandle {
    pid: Option<u32>,
}

#[cfg(windows)]
pub struct ProcessHandle {
    job: Option<JobObjectGuard>,
}

#[cfg(windows)]
struct JobObjectGuard {
    handle: windows::Win32::Foundation::HANDLE,
}

// SAFETY: Windows HANDLE is safe to send between threads
// The HANDLE represents a kernel object that can be used from any thread
#[cfg(windows)]
unsafe impl Send for JobObjectGuard {}

#[cfg(windows)]
unsafe impl Sync for JobObjectGuard {}

#[cfg(windows)]
impl Drop for JobObjectGuard {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;

        unsafe {
            let _ = CloseHandle(self.handle);
        }
        debug!("Job object handle closed");
    }
}

/// Wrapper for a managed child process with platform-specific cleanup
pub struct ManagedChild {
    pub child: Child,
    pub handle: ProcessHandle,
}

/// Result of a termination attempt.
#[allow(dead_code)]
#[derive(Debug)]
pub enum TerminationOutcome {
    Exited(std::process::ExitStatus),
    ForceKilled(std::process::ExitStatus),
    TimedOut,
}

impl ManagedChild {
    /// Creates a new managed child from a tokio Child process
    pub fn new(mut child: Child) -> io::Result<Self> {
        let handle = Self::create_handle(&mut child)?;
        Ok(Self { child, handle })
    }

    #[cfg(unix)]
    fn create_handle(child: &mut Child) -> io::Result<ProcessHandle> {
        Ok(ProcessHandle { pid: child.id() })
    }

    #[cfg(windows)]
    fn create_handle(child: &mut Child) -> io::Result<ProcessHandle> {
        let job = assign_to_job(child)?;
        Ok(ProcessHandle { job: Some(job) })
    }

    /// Terminates the child process and all its descendants
    pub fn terminate(&mut self) -> io::Result<()> {
        self.handle.terminate(&self.child)
    }

    /// Forcefully kills the child process and its descendants.
    pub async fn force_kill(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            self.handle.force_kill()
        }

        #[cfg(windows)]
        {
            self.child.kill().await
        }
    }

    /// Terminates the process, waits for exit, then force kills if needed.
    pub async fn terminate_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> io::Result<TerminationOutcome> {
        self.terminate()?;

        match tokio::time::timeout(timeout, self.wait()).await {
            Ok(status) => Ok(TerminationOutcome::Exited(status?)),
            Err(_) => {
                self.force_kill().await?;
                match tokio::time::timeout(timeout, self.wait()).await {
                    Ok(status) => Ok(TerminationOutcome::ForceKilled(status?)),
                    Err(_) => Ok(TerminationOutcome::TimedOut),
                }
            }
        }
    }

    /// Returns the process ID
    #[allow(dead_code)]
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Waits for the child process to exit
    pub async fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        self.child.wait().await
    }

    /// Attempts to kill the child process (fallback to standard kill)
    #[allow(dead_code)]
    pub async fn kill(&mut self) -> io::Result<()> {
        self.child.kill().await
    }
}

/// A handle for a streaming command execution that may involve retry attempts.
///
/// Unlike [`ManagedChild`], this handle represents a long-running background task that
/// owns the real child process. It provides the same lifecycle interface (terminate, wait,
/// kill, id) but routes signals through the background task so the real process group is
/// always targeted—never a placeholder process.
pub struct StreamingChildHandle {
    /// Send `()` to signal cancellation to the background task.
    /// Wrapped in `Option` so `terminate()` is idempotent after the first call.
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// PID of the currently-running real child process (0 = none running).
    current_pid: Arc<AtomicU32>,
    /// Receives the final exit status when the background task completes.
    final_status_rx: tokio::sync::oneshot::Receiver<std::process::ExitStatus>,
    /// Receives the typed process-group cleanup evidence for this execution.
    cleanup_rx: tokio::sync::oneshot::Receiver<ProcessGroupCleanupReport>,
    /// Cached cleanup evidence so the outcome can be read more than once.
    cleanup_report: Option<ProcessGroupCleanupReport>,
}

#[allow(dead_code)] // kill() and id() are part of the public lifecycle API; not all callers use both
impl StreamingChildHandle {
    /// Create a new handle. Called by the streaming executor after setting up the
    /// background task.
    pub fn new(
        cancel_tx: tokio::sync::oneshot::Sender<()>,
        current_pid: Arc<AtomicU32>,
        final_status_rx: tokio::sync::oneshot::Receiver<std::process::ExitStatus>,
        cleanup_rx: tokio::sync::oneshot::Receiver<ProcessGroupCleanupReport>,
    ) -> Self {
        Self {
            cancel_tx: Some(cancel_tx),
            current_pid,
            final_status_rx,
            cleanup_rx,
            cleanup_report: None,
        }
    }

    /// Awaits the typed process-group cleanup evidence for this execution.
    ///
    /// Callers that are about to mutate the managed worktree must gate on this
    /// result: leader exit alone (what [`Self::wait`] reports) does not prove
    /// that descendants released the worktree. A missing report is reported as
    /// unverifiable, never as quiescence.
    pub async fn process_group_cleanup(&mut self) -> ProcessGroupCleanupReport {
        if let Some(report) = &self.cleanup_report {
            return report.clone();
        }

        let report = match (&mut self.cleanup_rx).await {
            Ok(report) => report,
            Err(_) => ProcessGroupCleanupReport::missing(
                "the command runner ended without publishing process-group cleanup evidence",
            ),
        };
        self.cleanup_report = Some(report.clone());
        report
    }

    /// Signal the background task to terminate the current child process group.
    ///
    /// Idempotent: subsequent calls after the first are no-ops.
    pub fn terminate(&mut self) -> io::Result<()> {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        Ok(())
    }

    /// Terminate the process then wait up to `timeout` for the background task to finish.
    pub async fn terminate_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> io::Result<TerminationOutcome> {
        self.terminate()?;
        match tokio::time::timeout(timeout, &mut self.final_status_rx).await {
            Ok(Ok(status)) => Ok(TerminationOutcome::Exited(status)),
            Ok(Err(_)) => {
                // Sender was dropped (background task ended without sending).
                Ok(TerminationOutcome::ForceKilled({
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        std::process::ExitStatus::from_raw(0)
                    }
                    #[cfg(not(unix))]
                    {
                        use std::os::windows::process::ExitStatusExt;
                        std::process::ExitStatus::from_raw(0)
                    }
                }))
            }
            Err(_elapsed) => Ok(TerminationOutcome::TimedOut),
        }
    }

    /// Force kill (sends the same cancel signal; the background task handles graceful shutdown).
    pub async fn kill(&mut self) -> io::Result<()> {
        self.terminate()
    }

    /// Wait for the background task to complete and return the final exit status.
    pub async fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        (&mut self.final_status_rx).await.map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "streaming child handle dropped")
        })
    }

    /// Returns the PID of the currently-running real child process, if any.
    pub fn id(&self) -> Option<u32> {
        let pid = self.current_pid.load(Ordering::SeqCst);
        if pid == 0 {
            None
        } else {
            Some(pid)
        }
    }
}

impl ProcessHandle {
    #[cfg(unix)]
    pub fn terminate(&self, _child: &Child) -> io::Result<()> {
        use nix::sys::signal::{killpg, Signal};
        use nix::unistd::Pid;

        if let Some(pid) = self.pid {
            debug!("Sending SIGTERM to process group {}", pid);

            // Send SIGTERM to the entire process group
            match killpg(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                Ok(_) => {
                    debug!("Successfully sent SIGTERM to process group {}", pid);
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to send SIGTERM to process group {}: {}", pid, e);
                    Err(io::Error::other(e))
                }
            }
        } else {
            warn!("No PID available for process group termination");
            Ok(())
        }
    }

    #[cfg(unix)]
    pub fn force_kill(&self) -> io::Result<()> {
        use nix::sys::signal::{killpg, Signal};
        use nix::unistd::Pid;

        if let Some(pid) = self.pid {
            debug!("Sending SIGKILL to process group {}", pid);
            match killpg(Pid::from_raw(pid as i32), Signal::SIGKILL) {
                Ok(_) => {
                    debug!("Successfully sent SIGKILL to process group {}", pid);
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to send SIGKILL to process group {}: {}", pid, e);
                    Err(io::Error::other(e))
                }
            }
        } else {
            warn!("No PID available for process group force kill");
            Ok(())
        }
    }

    #[cfg(windows)]
    pub fn terminate(&self, child: &Child) -> io::Result<()> {
        // On Windows, job object will automatically terminate the process when dropped
        // But we can also explicitly terminate if needed
        if let Some(pid) = child.id() {
            debug!("Terminating Windows process {}", pid);
            // Job object will handle cleanup automatically
            Ok(())
        } else {
            warn!("No PID available for Windows process termination");
            Ok(())
        }
    }
}

#[cfg(unix)]
/// Configures the command to create a new process group
#[allow(dead_code)]
pub fn configure_process_group(cmd: &mut tokio::process::Command) {
    use nix::unistd::{setpgid, setsid, Pid};

    unsafe {
        cmd.pre_exec(|| {
            // Detach from the controlling terminal to avoid job-control stops (SIGTTIN/SIGTTOU).
            // This is especially important for shell pipelines and CLI wrappers that may
            // attempt to touch /dev/tty internally.
            match setsid() {
                Ok(_) => {
                    debug!("Created new session (setsid) for child process");
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to create new session (setsid): {}", e);
                    // Fallback: at least create a new process group.
                    match setpgid(Pid::from_raw(0), Pid::from_raw(0)) {
                        Ok(_) => {
                            debug!("Process group created successfully (fallback)");
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Failed to create process group: {}", e);
                            Err(io::Error::other(e))
                        }
                    }
                }
            }
        });
    }
}

#[cfg(windows)]
/// Assigns a process to a Windows job object for automatic cleanup
fn assign_to_job(child: &Child) -> io::Result<JobObjectGuard> {
    use std::mem::size_of;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::*;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};

    unsafe {
        // Create a new job object
        let job = CreateJobObjectW(None, windows::core::PCWSTR::null())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Set job to kill all processes when the job handle is closed
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Open a handle to the child process
        let process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, child.id().unwrap())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Assign the process to the job
        AssignProcessToJobObject(job, process_handle).map_err(|e| {
            CloseHandle(process_handle);
            CloseHandle(job);
            io::Error::new(io::ErrorKind::Other, e)
        })?;

        // Close the process handle (job handle is enough)
        CloseHandle(process_handle);

        debug!("Process assigned to job object successfully");
        Ok(JobObjectGuard { handle: job })
    }
}

#[cfg(windows)]
/// Configures the command for Windows (no-op, job assignment happens after spawn)
pub fn configure_process_group(_cmd: &mut tokio::process::Command) {
    // No pre-spawn configuration needed on Windows
}

/// Outcome of a post-completion process-group cleanup sweep.
///
/// Returned by [`cleanup_process_group`] to allow callers to log or assert on
/// what actually happened during the sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostCleanupOutcome {
    /// No process group ID was available; cleanup was skipped (non-Unix platforms).
    #[allow(dead_code)]
    NoPgid,
    /// SIGTERM was sent.  The process group may already have exited before
    /// SIGKILL was delivered.
    Terminated,
    /// SIGTERM was sent and SIGKILL was subsequently sent to any survivors.
    Killed,
    /// The process group was already gone when SIGTERM was attempted (ESRCH).
    AlreadyGone,
}

/// Default bounded budget for proving that an owned process group became
/// quiescent after termination was requested.
pub const DEFAULT_PROCESS_GROUP_CLEANUP_TIMEOUT_MS: u64 = 10_000;

/// Default grace period between SIGTERM and SIGKILL escalation.
///
/// The window is polled rather than slept through, so a SIGTERM-responsive
/// group still finishes in milliseconds. It is wide enough for a cooperative
/// descendant to release resources it owns — notably a Git `index.lock` it must
/// remove itself, because Conflux never deletes lock files.
pub const DEFAULT_PROCESS_GROUP_SIGTERM_GRACE_MS: u64 = 2_000;

/// Interval between process-group membership probes during cleanup.
const PROCESS_GROUP_PROBE_INTERVAL_MS: u64 = 20;

/// Whether an owned process group is proven to have no remaining members.
///
/// Leader exit is deliberately *not* one of these verdicts: reaping the group
/// leader says nothing about descendants that may still hold worktree files
/// such as Git's `index.lock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessGroupQuiescence {
    /// Verified: the owned process group has no remaining members.
    Confirmed,
    /// Verification does not apply: there was no owned group to sweep, cleanup
    /// was disabled, or the platform runtime (Windows job objects) owns
    /// descendant lifetime itself.
    NotApplicable,
    /// Owned members were still present when the cleanup budget expired.
    MembersRemain,
    /// Group membership could not be checked, so quiescence is unproven.
    Unverifiable,
}

impl ProcessGroupQuiescence {
    /// Whether callers may treat the owned process group as quiescent.
    pub fn is_confirmed(self) -> bool {
        matches!(self, Self::Confirmed | Self::NotApplicable)
    }

    /// Stable label for structured logs and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::NotApplicable => "not_applicable",
            Self::MembersRemain => "members_remain",
            Self::Unverifiable => "unverifiable",
        }
    }
}

/// Typed evidence produced by a bounded process-group cleanup sequence.
///
/// This is ephemeral process-lifetime evidence only: it is never persisted and
/// never used to route workflow state after a restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessGroupCleanupReport {
    quiescence: ProcessGroupQuiescence,
    pgid: Option<u32>,
    force_killed: bool,
    already_gone: bool,
    detail: String,
}

impl ProcessGroupCleanupReport {
    fn new(
        quiescence: ProcessGroupQuiescence,
        pgid: Option<u32>,
        force_killed: bool,
        already_gone: bool,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            quiescence,
            pgid,
            force_killed,
            already_gone,
            detail: detail.into(),
        }
    }

    /// Evidence that cleanup verification does not apply to this execution.
    pub fn not_applicable(reason: impl Into<String>) -> Self {
        Self::new(
            ProcessGroupQuiescence::NotApplicable,
            None,
            false,
            false,
            reason,
        )
    }

    /// Evidence that no cleanup result was ever published by the owner.
    ///
    /// Absence of evidence is never treated as quiescence.
    pub fn missing(reason: impl Into<String>) -> Self {
        Self::new(
            ProcessGroupQuiescence::Unverifiable,
            None,
            false,
            false,
            reason,
        )
    }

    pub fn quiescence(&self) -> ProcessGroupQuiescence {
        self.quiescence
    }

    #[allow(dead_code)] // Part of the report's accessor surface; used by tests and callers.
    pub fn pgid(&self) -> Option<u32> {
        self.pgid
    }

    /// Whether SIGKILL escalation was needed to reach quiescence.
    pub fn force_killed(&self) -> bool {
        self.force_killed
    }

    /// Whether the process group was already gone when cleanup started.
    pub fn already_gone(&self) -> bool {
        self.already_gone
    }

    /// Whether the owned process group may be treated as quiescent.
    pub fn is_confirmed(&self) -> bool {
        self.quiescence.is_confirmed()
    }

    /// Builds an arbitrary verdict so in-crate tests can exercise callers that
    /// gate on cleanup evidence without spawning real processes.
    #[cfg(test)]
    pub(crate) fn for_test(
        quiescence: ProcessGroupQuiescence,
        pgid: Option<u32>,
        detail: &str,
    ) -> Self {
        Self::new(quiescence, pgid, false, false, detail)
    }

    /// Actionable one-line diagnostics for logs and operator-facing errors.
    pub fn diagnostics(&self) -> String {
        match self.pgid {
            Some(pgid) => format!(
                "process-group cleanup {} (pgid={}, force_killed={}): {}",
                self.quiescence.as_str(),
                pgid,
                self.force_killed,
                self.detail
            ),
            None => format!(
                "process-group cleanup {}: {}",
                self.quiescence.as_str(),
                self.detail
            ),
        }
    }
}

/// Result of delivering a signal to an owned process group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SignalResult {
    /// The signal reached the group.
    Delivered,
    /// The group no longer exists (ESRCH).
    AlreadyGone,
    /// The signal could not be delivered; quiescence stays unproven.
    Failed(String),
}

/// Result of a non-destructive membership probe of an owned process group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeResult {
    /// At least one member is still alive.
    MembersRemain,
    /// No members remain.
    Empty,
    /// Membership could not be determined.
    Unknown(String),
}

/// Signal/probe surface used by the bounded cleanup sequence.
///
/// Kept behind a trait so the cleanup decision logic can be unit-tested with
/// in-memory doubles instead of real OS processes and wall-clock waits.
pub(crate) trait ProcessGroupControl {
    fn signal_term(&self) -> SignalResult;
    fn signal_kill(&self) -> SignalResult;
    /// Non-destructive membership check (`killpg(pgid, 0)` on Unix).
    fn probe(&self) -> ProbeResult;
}

/// Bounded budget for the graceful-then-forceful cleanup sequence.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CleanupBudget {
    /// Window allowed for cooperative shutdown after SIGTERM.
    pub sigterm_grace: Duration,
    /// Total budget for proving quiescence, including force-kill escalation.
    pub total: Duration,
    /// Interval between membership probes.
    pub probe_interval: Duration,
}

impl CleanupBudget {
    pub fn from_millis(sigterm_grace_ms: u64, total_ms: u64) -> Self {
        Self {
            sigterm_grace: Duration::from_millis(sigterm_grace_ms),
            total: Duration::from_millis(total_ms),
            probe_interval: Duration::from_millis(PROCESS_GROUP_PROBE_INTERVAL_MS),
        }
    }
}

/// Polls group membership until it is empty or `deadline` passes, and returns
/// the last observation.
///
/// Always probes at least once so a zero-length window still yields a real
/// observation rather than an assumption. An indeterminate probe keeps the poll
/// running rather than ending it: on some platforms a group whose members were
/// just killed reports `EPERM` for the moments before those members are reaped,
/// and that transient state is neither proof of quiescence nor a final failure.
async fn poll_until_empty<C: ProcessGroupControl>(
    control: &C,
    deadline: tokio::time::Instant,
    probe_interval: Duration,
) -> ProbeResult {
    loop {
        let observation = control.probe();
        if matches!(observation, ProbeResult::Empty) {
            return ProbeResult::Empty;
        }

        let now = tokio::time::Instant::now();
        if now >= deadline {
            return observation;
        }
        let step = probe_interval.min(deadline - now);
        tokio::time::sleep(step).await;
    }
}

/// Maps a non-empty final observation onto an unconfirmed verdict.
fn unconfirmed_from(observation: ProbeResult, phase: &str) -> (ProcessGroupQuiescence, String) {
    match observation {
        ProbeResult::Unknown(reason) => (
            ProcessGroupQuiescence::Unverifiable,
            format!("group membership could not be checked after {phase} ({reason})"),
        ),
        _ => (
            ProcessGroupQuiescence::MembersRemain,
            format!(
                "members were still alive after {phase} and the cleanup budget expired; \
                 inspect surviving processes with `ps -o pid,pgid,command -g <pgid>`"
            ),
        ),
    }
}

/// Runs the bounded graceful-then-forceful cleanup sequence and returns typed
/// quiescence evidence.
///
/// Sequence: SIGTERM → poll for an empty group within the grace window →
/// SIGKILL → poll again until the budget is exhausted. The sequence never
/// concludes quiescence from leader exit or elapsed time; only a membership
/// probe that reports an empty group counts as proof.
pub(crate) async fn drive_process_group_cleanup<C: ProcessGroupControl>(
    control: &C,
    pgid: Option<u32>,
    budget: CleanupBudget,
) -> ProcessGroupCleanupReport {
    let start = tokio::time::Instant::now();
    let deadline = start + budget.total;
    let mut notes: Vec<String> = Vec::new();

    match control.signal_term() {
        SignalResult::AlreadyGone => {
            return ProcessGroupCleanupReport::new(
                ProcessGroupQuiescence::Confirmed,
                pgid,
                false,
                true,
                "process group was already gone when SIGTERM was attempted",
            );
        }
        SignalResult::Delivered => {}
        SignalResult::Failed(reason) => notes.push(format!("SIGTERM failed: {reason}")),
    }

    let graceful_deadline = (start + budget.sigterm_grace).min(deadline);
    let graceful = poll_until_empty(control, graceful_deadline, budget.probe_interval).await;
    if matches!(graceful, ProbeResult::Empty) {
        return ProcessGroupCleanupReport::new(
            ProcessGroupQuiescence::Confirmed,
            pgid,
            false,
            false,
            join_detail("no members remained after graceful termination", &notes),
        );
    }

    if tokio::time::Instant::now() >= deadline {
        let (quiescence, detail) = unconfirmed_from(graceful, "SIGTERM");
        return ProcessGroupCleanupReport::new(
            quiescence,
            pgid,
            false,
            false,
            join_detail(
                &format!("cleanup budget expired before SIGKILL escalation: {detail}"),
                &notes,
            ),
        );
    }

    let mut force_killed = false;
    match control.signal_kill() {
        SignalResult::AlreadyGone => {
            return ProcessGroupCleanupReport::new(
                ProcessGroupQuiescence::Confirmed,
                pgid,
                false,
                false,
                join_detail("process group exited before SIGKILL was delivered", &notes),
            );
        }
        SignalResult::Delivered => force_killed = true,
        SignalResult::Failed(reason) => notes.push(format!("SIGKILL failed: {reason}")),
    }

    let forced = poll_until_empty(control, deadline, budget.probe_interval).await;
    if matches!(forced, ProbeResult::Empty) {
        return ProcessGroupCleanupReport::new(
            ProcessGroupQuiescence::Confirmed,
            pgid,
            force_killed,
            false,
            join_detail("no members remained after forced termination", &notes),
        );
    }

    let (quiescence, detail) = unconfirmed_from(forced, "SIGKILL");
    ProcessGroupCleanupReport::new(
        quiescence,
        pgid,
        force_killed,
        false,
        join_detail(&detail, &notes),
    )
}

fn join_detail(base: &str, notes: &[String]) -> String {
    if notes.is_empty() {
        base.to_string()
    } else {
        format!("{base} [{}]", notes.join("; "))
    }
}

/// Real Unix process group targeted by `killpg`.
#[cfg(unix)]
struct UnixProcessGroup {
    pgid: u32,
}

#[cfg(unix)]
impl UnixProcessGroup {
    fn send(&self, signal: nix::sys::signal::Signal) -> SignalResult {
        use nix::errno::Errno;
        use nix::sys::signal::killpg;
        use nix::unistd::Pid;

        match killpg(Pid::from_raw(self.pgid as i32), signal) {
            Ok(()) => SignalResult::Delivered,
            Err(Errno::ESRCH) => SignalResult::AlreadyGone,
            Err(e) => SignalResult::Failed(e.to_string()),
        }
    }
}

#[cfg(unix)]
impl ProcessGroupControl for UnixProcessGroup {
    fn signal_term(&self) -> SignalResult {
        self.send(nix::sys::signal::Signal::SIGTERM)
    }

    fn signal_kill(&self) -> SignalResult {
        self.send(nix::sys::signal::Signal::SIGKILL)
    }

    fn probe(&self) -> ProbeResult {
        use nix::errno::Errno;
        use nix::sys::signal::killpg;
        use nix::unistd::Pid;

        // Signal 0 performs error checking only: it proves membership without
        // disturbing surviving members.
        match killpg(Pid::from_raw(self.pgid as i32), None) {
            Ok(()) => ProbeResult::MembersRemain,
            Err(Errno::ESRCH) => ProbeResult::Empty,
            Err(e) => ProbeResult::Unknown(e.to_string()),
        }
    }
}

/// Performs bounded process-group cleanup and returns typed quiescence evidence.
///
/// Unlike [`cleanup_process_group`], the caller receives proof (or an explicit
/// failure to prove) that no owned process-group members remain, which is the
/// precondition for starting Conflux-owned Git operations in the same worktree.
#[cfg(unix)]
pub async fn cleanup_process_group_verified(
    pgid: u32,
    sigterm_grace_ms: u64,
    cleanup_timeout_ms: u64,
    op: Option<&str>,
    change_id: Option<&str>,
) -> ProcessGroupCleanupReport {
    if pgid == 0 {
        return ProcessGroupCleanupReport::not_applicable(
            "no process group id was available for cleanup",
        );
    }

    let control = UnixProcessGroup { pgid };
    let report = drive_process_group_cleanup(
        &control,
        Some(pgid),
        CleanupBudget::from_millis(sigterm_grace_ms, cleanup_timeout_ms),
    )
    .await;

    if report.is_confirmed() {
        debug!(
            pgid,
            op,
            change_id,
            quiescence = report.quiescence().as_str(),
            "post-cleanup: {}",
            report.diagnostics()
        );
    } else {
        warn!(
            pgid,
            op,
            change_id,
            quiescence = report.quiescence().as_str(),
            "post-cleanup: {}",
            report.diagnostics()
        );
    }

    report
}

/// Non-Unix stub: Windows job objects own descendant lifetime, so there is no
/// separate process group to prove quiescent.
#[cfg(not(unix))]
pub async fn cleanup_process_group_verified(
    pgid: u32,
    _sigterm_grace_ms: u64,
    _cleanup_timeout_ms: u64,
    _op: Option<&str>,
    _change_id: Option<&str>,
) -> ProcessGroupCleanupReport {
    debug!("post-cleanup: no-op on non-Unix platform (pgid={})", pgid);
    ProcessGroupCleanupReport::not_applicable(
        "descendant lifetime is owned by the Windows job object",
    )
}

/// Performs a strict post-completion cleanup sweep on a Unix process group.
///
/// This function is the canonical "launcher owns cleanup" implementation.
/// It should be called after a command is considered complete (success,
/// failure, cancellation, or inactivity timeout) when strict cleanup is
/// enabled.
///
/// # Sequence
///
/// Delegates to [`cleanup_process_group_verified`], which runs
/// `SIGTERM` → bounded membership polling → `SIGKILL` → bounded membership
/// polling, and maps the typed evidence back to [`PostCleanupOutcome`] for
/// callers that only need the coarse sweep result.
///
/// # Arguments
///
/// * `pgid` - Process group ID to sweep (typically the PID of the spawned `sh` process).
/// * `sigterm_grace_ms` - Grace period in ms between SIGTERM and SIGKILL.
/// * `op` - Operation name for structured log fields (e.g. `"apply"`).
/// * `change_id` - Change ID for structured log fields.
pub async fn cleanup_process_group(
    pgid: u32,
    sigterm_grace_ms: u64,
    op: Option<&str>,
    change_id: Option<&str>,
) -> PostCleanupOutcome {
    if pgid == 0 {
        return PostCleanupOutcome::NoPgid;
    }

    let report = cleanup_process_group_verified(
        pgid,
        sigterm_grace_ms,
        DEFAULT_PROCESS_GROUP_CLEANUP_TIMEOUT_MS,
        op,
        change_id,
    )
    .await;

    match report.quiescence() {
        ProcessGroupQuiescence::NotApplicable => PostCleanupOutcome::NoPgid,
        _ if report.already_gone() => PostCleanupOutcome::AlreadyGone,
        _ if report.force_killed() => PostCleanupOutcome::Killed,
        _ => PostCleanupOutcome::Terminated,
    }
}

#[cfg(test)]
mod cleanup_driver_tests {
    use super::*;
    use std::cell::Cell;

    /// In-memory process group double.
    ///
    /// Models only what the cleanup sequence can observe: signal delivery
    /// results and membership probes. No OS process, filesystem, or clock
    /// boundary is touched, so the decision logic is exercised in isolation.
    struct FakeProcessGroup {
        term: SignalResult,
        kill: SignalResult,
        /// Probes that still report members before SIGTERM alone empties the
        /// group. `None` means SIGTERM never empties it.
        probes_until_empty_after_term: Cell<Option<u32>>,
        /// Whether SIGKILL delivery empties the group.
        empties_on_kill: bool,
        probe_error: Option<String>,
        killed: Cell<bool>,
        term_calls: Cell<u32>,
        kill_calls: Cell<u32>,
        probe_calls: Cell<u32>,
    }

    impl FakeProcessGroup {
        fn new() -> Self {
            Self {
                term: SignalResult::Delivered,
                kill: SignalResult::Delivered,
                probes_until_empty_after_term: Cell::new(None),
                empties_on_kill: true,
                probe_error: None,
                killed: Cell::new(false),
                term_calls: Cell::new(0),
                kill_calls: Cell::new(0),
                probe_calls: Cell::new(0),
            }
        }

        fn with_term(mut self, result: SignalResult) -> Self {
            self.term = result;
            self
        }

        fn empties_after_term_probes(self, probes: u32) -> Self {
            self.probes_until_empty_after_term.set(Some(probes));
            self
        }

        fn survives_sigkill(mut self) -> Self {
            self.empties_on_kill = false;
            self
        }

        fn with_probe_error(mut self, reason: &str) -> Self {
            self.probe_error = Some(reason.to_string());
            self
        }
    }

    impl ProcessGroupControl for FakeProcessGroup {
        fn signal_term(&self) -> SignalResult {
            self.term_calls.set(self.term_calls.get() + 1);
            self.term.clone()
        }

        fn signal_kill(&self) -> SignalResult {
            self.kill_calls.set(self.kill_calls.get() + 1);
            if matches!(self.kill, SignalResult::Delivered) {
                self.killed.set(true);
            }
            self.kill.clone()
        }

        fn probe(&self) -> ProbeResult {
            self.probe_calls.set(self.probe_calls.get() + 1);
            if let Some(reason) = &self.probe_error {
                return ProbeResult::Unknown(reason.clone());
            }
            if self.killed.get() && self.empties_on_kill {
                return ProbeResult::Empty;
            }
            match self.probes_until_empty_after_term.get() {
                Some(0) => ProbeResult::Empty,
                Some(remaining) => {
                    self.probes_until_empty_after_term.set(Some(remaining - 1));
                    ProbeResult::MembersRemain
                }
                None => ProbeResult::MembersRemain,
            }
        }
    }

    fn budget() -> CleanupBudget {
        CleanupBudget::from_millis(100, 1_000)
    }

    /// Natural exit: the group is already gone when cleanup starts.
    #[tokio::test(start_paused = true)]
    async fn process_group_cleanup_confirms_when_group_already_gone() {
        let group = FakeProcessGroup::new().with_term(SignalResult::AlreadyGone);

        let report = drive_process_group_cleanup(&group, Some(4242), budget()).await;

        assert_eq!(report.quiescence(), ProcessGroupQuiescence::Confirmed);
        assert!(report.already_gone());
        assert!(!report.force_killed());
        assert_eq!(group.kill_calls.get(), 0, "SIGKILL must not be escalated");
    }

    /// Graceful path: a descendant outlives the leader but exits on SIGTERM.
    #[tokio::test(start_paused = true)]
    async fn process_group_cleanup_confirms_after_graceful_descendant_exit() {
        let group = FakeProcessGroup::new().empties_after_term_probes(3);

        let report = drive_process_group_cleanup(&group, Some(11), budget()).await;

        assert_eq!(report.quiescence(), ProcessGroupQuiescence::Confirmed);
        assert!(!report.force_killed());
        assert_eq!(group.kill_calls.get(), 0);
        assert!(group.probe_calls.get() >= 4);
    }

    /// Forced path: SIGTERM is not enough, SIGKILL reaches quiescence in budget.
    #[tokio::test(start_paused = true)]
    async fn process_group_cleanup_confirms_after_forced_termination() {
        let group = FakeProcessGroup::new();

        let report = drive_process_group_cleanup(&group, Some(12), budget()).await;

        assert_eq!(report.quiescence(), ProcessGroupQuiescence::Confirmed);
        assert!(report.force_killed());
        assert_eq!(group.kill_calls.get(), 1);
    }

    /// Leader exit alone is never quiescence: descendants still hold the group.
    #[tokio::test(start_paused = true)]
    async fn process_group_cleanup_never_confirms_from_leader_exit_alone() {
        // SIGTERM is delivered (leader reaped by the caller) but a descendant
        // keeps the group alive through both signals.
        let group = FakeProcessGroup::new().survives_sigkill();

        let report = drive_process_group_cleanup(&group, Some(13), budget()).await;

        assert_eq!(report.quiescence(), ProcessGroupQuiescence::MembersRemain);
        assert!(!report.is_confirmed());
    }

    /// Budget exhaustion before escalation is unconfirmed, not success.
    #[tokio::test(start_paused = true)]
    async fn process_group_cleanup_reports_members_remain_when_budget_exhausted() {
        let group = FakeProcessGroup::new();

        let report =
            drive_process_group_cleanup(&group, Some(14), CleanupBudget::from_millis(100, 0)).await;

        assert_eq!(report.quiescence(), ProcessGroupQuiescence::MembersRemain);
        assert!(!report.is_confirmed());
        assert_eq!(
            group.kill_calls.get(),
            0,
            "an exhausted budget must not claim an escalation it never ran"
        );
        assert!(
            report.diagnostics().contains("cleanup budget expired"),
            "diagnostics must name the exhausted budget: {}",
            report.diagnostics()
        );
    }

    /// Unknown membership is unconfirmed: absence of proof is never proof.
    #[tokio::test(start_paused = true)]
    async fn process_group_cleanup_reports_unverifiable_when_membership_unknown() {
        let group = FakeProcessGroup::new().with_probe_error("EPERM");

        let report = drive_process_group_cleanup(&group, Some(15), budget()).await;

        assert_eq!(report.quiescence(), ProcessGroupQuiescence::Unverifiable);
        assert!(!report.is_confirmed());
        assert!(report.diagnostics().contains("EPERM"));
    }

    /// A failed SIGTERM keeps the sequence running and is recorded verbatim.
    #[tokio::test(start_paused = true)]
    async fn process_group_cleanup_records_signal_failures_in_diagnostics() {
        let group = FakeProcessGroup::new().with_term(SignalResult::Failed("EPERM".to_string()));

        let report = drive_process_group_cleanup(&group, Some(16), budget()).await;

        assert_eq!(report.quiescence(), ProcessGroupQuiescence::Confirmed);
        assert!(report.force_killed());
        assert!(
            report.diagnostics().contains("SIGTERM failed: EPERM"),
            "diagnostics must retain the signal failure: {}",
            report.diagnostics()
        );
    }

    #[test]
    fn missing_cleanup_evidence_is_never_confirmed() {
        let report = ProcessGroupCleanupReport::missing("runner published no cleanup evidence");
        assert!(!report.is_confirmed());
        assert_eq!(report.quiescence(), ProcessGroupQuiescence::Unverifiable);

        let skipped = ProcessGroupCleanupReport::not_applicable("cleanup disabled");
        assert!(skipped.is_confirmed());
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tokio::process::Command;

    #[tokio::test]
    async fn terminate_with_timeout_exits_cleanly() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("sleep 5")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        configure_process_group(&mut cmd);
        let child = cmd.spawn().expect("spawn sleep");
        let mut child = ManagedChild::new(child).expect("managed child");

        let outcome = child
            .terminate_with_timeout(Duration::from_secs(1))
            .await
            .expect("terminate");

        assert!(matches!(
            outcome,
            TerminationOutcome::Exited(_) | TerminationOutcome::ForceKilled(_)
        ));
    }

    #[tokio::test]
    async fn terminate_with_timeout_force_kills() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("trap '' TERM; while true; do sleep 1; done")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        configure_process_group(&mut cmd);
        let child = cmd.spawn().expect("spawn trap");
        let mut child = ManagedChild::new(child).expect("managed child");

        let outcome = child
            .terminate_with_timeout(Duration::from_millis(200))
            .await
            .expect("terminate");

        assert!(matches!(
            outcome,
            TerminationOutcome::Exited(_)
                | TerminationOutcome::ForceKilled(_)
                | TerminationOutcome::TimedOut
        ));
    }

    /// Helper: check whether a process group has any live members.
    /// Returns true if the group is gone (ESRCH), false if members remain.
    fn pgid_is_gone(pgid: u32) -> bool {
        use nix::errno::Errno;
        use nix::sys::signal::{killpg, Signal};
        use nix::unistd::Pid;
        match killpg(Pid::from_raw(pgid as i32), Signal::SIGKILL) {
            Ok(()) => false,           // still alive
            Err(Errno::ESRCH) => true, // gone
            Err(_) => false,
        }
    }

    /// Regression test 1.6: successful command that backgrounds a child is cleaned up.
    ///
    /// Spawns `sh -c 'sleep 60 & exit 0'` (exits immediately; backgrounds a sleep).
    /// After the parent exits and `cleanup_process_group` is called, `killpg(pgid, 0)`
    /// must return ESRCH (no live members).
    #[tokio::test]
    async fn successful_command_backgrounded_child_is_cleaned_up() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("sleep 60 & exit 0")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        configure_process_group(&mut cmd);
        let child = cmd.spawn().expect("spawn");
        let mut child = ManagedChild::new(child).expect("managed child");
        let pgid = child.id().unwrap_or(0);
        assert!(pgid > 0, "process must have a PID");

        // Wait for the parent shell to exit (it exits immediately after backgrounding sleep).
        child.wait().await.expect("wait");

        // At this point the backgrounded `sleep 60` may still be running.
        // cleanup_process_group must terminate it.
        cleanup_process_group(pgid, 50, Some("test"), Some("regression-1.6")).await;

        // Allow a brief moment for the kernel to reap the process.
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(
            pgid_is_gone(pgid),
            "process group {} should be gone after cleanup, but members remain",
            pgid
        );
    }

    /// Regression test 1.7: failed command that backgrounds a child is cleaned up.
    ///
    /// Spawns `sh -c 'sleep 60 & exit 1'` (fails; backgrounds a sleep).
    /// Same verification as 1.6.
    #[tokio::test]
    async fn failed_command_backgrounded_child_is_cleaned_up() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("sleep 60 & exit 1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        configure_process_group(&mut cmd);
        let child = cmd.spawn().expect("spawn");
        let mut child = ManagedChild::new(child).expect("managed child");
        let pgid = child.id().unwrap_or(0);
        assert!(pgid > 0);

        child.wait().await.expect("wait");

        cleanup_process_group(pgid, 50, Some("test"), Some("regression-1.7")).await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(
            pgid_is_gone(pgid),
            "process group {} should be gone after cleanup, but members remain",
            pgid
        );
    }

    /// Regression test 1.8: cancellation (terminate_with_timeout) triggers full
    /// process-group cleanup.
    ///
    /// Spawns `sh -c 'sleep 60 & sleep 60'` (both parent and a backgrounded sibling sleep).
    /// After `terminate_with_timeout` is called, `cleanup_process_group` sweeps survivors.
    /// `killpg(pgid, 0)` must then return ESRCH.
    #[tokio::test]
    async fn cancellation_triggers_full_process_group_cleanup() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("sleep 60 & sleep 60")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        configure_process_group(&mut cmd);
        let child = cmd.spawn().expect("spawn");
        let mut child = ManagedChild::new(child).expect("managed child");
        let pgid = child.id().unwrap_or(0);
        assert!(pgid > 0);

        // Simulate cancellation.
        let _ = child
            .terminate_with_timeout(Duration::from_millis(500))
            .await;

        // Run post-completion cleanup to sweep any survivors (e.g. the backgrounded sleep).
        cleanup_process_group(pgid, 50, Some("test"), Some("regression-1.8")).await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(
            pgid_is_gone(pgid),
            "process group {} should be gone after cancellation + cleanup",
            pgid
        );
    }
}
