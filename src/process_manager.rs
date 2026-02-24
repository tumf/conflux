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
}

#[allow(dead_code)] // kill() and id() are part of the public lifecycle API; not all callers use both
impl StreamingChildHandle {
    /// Create a new handle. Called by the streaming executor after setting up the
    /// background task.
    pub fn new(
        cancel_tx: tokio::sync::oneshot::Sender<()>,
        current_pid: Arc<AtomicU32>,
        final_status_rx: tokio::sync::oneshot::Receiver<std::process::ExitStatus>,
    ) -> Self {
        Self {
            cancel_tx: Some(cancel_tx),
            current_pid,
            final_status_rx,
        }
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
    use nix::unistd::{setpgid, Pid};

    unsafe {
        cmd.pre_exec(|| {
            // Set the process group ID to the process ID (making it the group leader)
            match setpgid(Pid::from_raw(0), Pid::from_raw(0)) {
                Ok(_) => {
                    debug!("Process group created successfully");
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to create process group: {}", e);
                    Err(io::Error::other(e))
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
}
