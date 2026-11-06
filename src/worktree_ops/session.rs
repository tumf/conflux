//! Owner-managed configured-command worktree sessions.
//!
//! Two pieces live here, and both are frontend independent:
//!
//! * [`SessionProvisioner`] — the TUI `+` key's provisioning sequence (create a
//!   `ws-session-*` worktree from base `HEAD`, validate the cwd, run
//!   `.wt/setup`, clean up on setup failure, revalidate) over a seam whose
//!   production implementation calls the same VCS functions the TUI calls. The
//!   TUI handler is not rewired onto it; [`PLUS_STEP_ORDER`] is the expected
//!   call sequence both the TUI tests and the provisioner tests assert, which
//!   is what keeps the two from drifting.
//! * [`WorktreeSessionRegistry`] — process-local sessions of the configured
//!   `worktree_command`. Each session is one
//!   `execute_streaming_with_retry_observed` invocation on a runner bound to
//!   the registry's own [`RunCommandScope`]: the
//!   configured retry policy retries *inside* the session, a stop goes through
//!   the runner's own cancellation handle (never an external `killpg`, which the
//!   retry policy would read as a crash and relaunch), and owner shutdown closes
//!   the scope under a bounded deadline.
//!
//! Nothing here is persisted. The registry holds running sessions until they
//! finish and at most [`MAX_RETAINED_FINISHED`] finished ones; restart forgets
//! everything, and no scheduling, resume, acceptance, or archive decision reads
//! it. Output is retained in memory only, never written to a file, never sent
//! to the log ring, the event stream, or the snapshot.
//!
//! Provenance is typed. The output tail holds only what the child wrote; the
//! runner's own facts — retries, launch failures, limits, cleanup, and the
//! reaped leader status — arrive as [`RunnerNotice`]s on a separate channel,
//! so `attempt`, `signal`, and `runner_notices` can never be forged by a line
//! the child prints.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tracing::warn;

use crate::ai_command_runner::{
    AiCommandRunner, OutputLine, RetryCause, RunCommandScope, RunnerNotice,
};
use crate::process_manager::{CommandTermination, StreamingChildHandle};

use super::service::{SessionProbe, WorktreeBackend};

/// Sessions that may run at once, reservations included.
pub const MAX_RUNNING_SESSIONS: usize = 4;

/// Finished sessions retained for observation, oldest evicted first.
pub const MAX_RETAINED_FINISHED: usize = 32;

/// Retained output tail per session, in bytes of sanitized line text.
pub const MAX_OUTPUT_BYTES: usize = 65_536;

/// Retained runner notices per session, oldest dropped first.
pub const MAX_RUNNER_NOTICES: usize = 32;

/// Bounded deadline owner shutdown gives the registry scope.
pub const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(10);

/// Operation label the runner logs a session's spawn under.
pub const SESSION_OPERATION: &str = "worktree_session";

/// Branch prefix of a provisioned session worktree, exactly as the TUI `+` key.
pub const SESSION_BRANCH_PREFIX: &str = "ws-session";

/// Attempts the branch-name generator gets, exactly as the TUI `+` key.
pub const SESSION_BRANCH_ATTEMPTS: u32 = 10;

/// Stable refusal reasons published in the capability block.
pub const SESSION_REASONS: [&str; 5] = [
    "worktree_busy",
    "capacity",
    "cwd_invalid",
    "setup_failed",
    "unavailable",
];

// ============================================================================
// Provisioner
// ============================================================================

/// Names of the provisioning steps, as recorded by a test seam.
// Read by the order-equivalence tests; the binary crate sees no caller.
#[allow(dead_code)]
pub mod step {
    /// `check_git_repo`.
    pub const CHECK_GIT_REPO: &str = "check_git_repo";
    /// `generate_unique_branch_name`.
    pub const GENERATE_BRANCH: &str = "generate_unique_branch_name";
    /// `worktree_add`.
    pub const WORKTREE_ADD: &str = "worktree_add";
    /// `validate_worktree_command_cwd`.
    pub const VALIDATE_CWD: &str = "validate_worktree_command_cwd";
    /// `run_worktree_setup`.
    pub const RUN_SETUP: &str = "run_worktree_setup";
    /// `worktree_remove_after_setup_failure`.
    pub const CLEANUP: &str = "worktree_remove_after_setup_failure";
}

/// The TUI `+` key's seam calls for a successful provisioning, in order.
///
/// Both `src/tui/key_handlers.rs` and the provisioner tests assert against this
/// one list: the TUI handler is not rewired onto the provisioner, so this is
/// what keeps the two paths from drifting.
#[allow(dead_code)] // Read by the order-equivalence tests.
pub const PLUS_STEP_ORDER: [&str; 7] = [
    step::CHECK_GIT_REPO,
    step::GENERATE_BRANCH,
    step::WORKTREE_ADD,
    step::VALIDATE_CWD,
    step::RUN_SETUP,
    step::VALIDATE_CWD,
    step::VALIDATE_CWD,
];

/// The TUI `+` key's seam calls when `.wt/setup` fails, in order.
#[allow(dead_code)] // Read by the order-equivalence tests.
pub const PLUS_SETUP_FAILURE_STEP_ORDER: [&str; 6] = [
    step::CHECK_GIT_REPO,
    step::GENERATE_BRANCH,
    step::WORKTREE_ADD,
    step::VALIDATE_CWD,
    step::RUN_SETUP,
    step::CLEANUP,
];

/// Every Git/hook effect provisioning performs, in the TUI `+` key's shape.
///
/// Errors are plain strings: they are diagnostics for the operator, and the
/// caller redacts absolute paths before anything reaches a remote client.
#[async_trait]
pub trait SessionProvisionRuntime: Send + Sync {
    /// Whether `repo_root` is a Git repository.
    async fn check_git_repo(&self, repo_root: &Path) -> Result<bool, String>;
    /// A branch name with `prefix` that does not exist yet.
    async fn generate_unique_branch_name(
        &self,
        repo_root: &Path,
        prefix: &str,
        max_attempts: u32,
    ) -> Result<String, String>;
    /// Add a worktree at `worktree_path` on a new `branch_name` from `base_commit`.
    async fn worktree_add(
        &self,
        repo_root: &Path,
        worktree_path: &str,
        branch_name: &str,
        base_commit: &str,
    ) -> Result<(), String>;
    /// Whether `worktree_path` is a safe cwd for the configured command.
    async fn validate_worktree_command_cwd(
        &self,
        repo_root: &Path,
        worktree_path: &Path,
    ) -> Result<(), String>;
    /// Run `.wt/setup`; `Ok(true)` when a script existed and ran.
    async fn run_worktree_setup(
        &self,
        repo_root: &Path,
        worktree_path: &Path,
    ) -> Result<bool, String>;
    /// Remove a worktree whose setup failed: forced, teardown skipped, branch kept.
    async fn worktree_remove_after_setup_failure(
        &self,
        repo_root: &Path,
        worktree_path: &str,
    ) -> Result<(), String>;
}

/// Production seam: the shared worktree backend for the two repository
/// mutations, and the same `crate::vcs::git::commands` functions the TUI's
/// `ProductionWorktreePlusRuntime` calls for everything else.
pub struct GitSessionProvisionRuntime {
    backend: Arc<dyn WorktreeBackend>,
}

impl GitSessionProvisionRuntime {
    /// Wire the seam to the shared backend.
    pub fn new(backend: Arc<dyn WorktreeBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl SessionProvisionRuntime for GitSessionProvisionRuntime {
    async fn check_git_repo(&self, repo_root: &Path) -> Result<bool, String> {
        crate::vcs::git::commands::check_git_repo(repo_root)
            .await
            .map_err(|e| e.to_string())
    }

    async fn generate_unique_branch_name(
        &self,
        repo_root: &Path,
        prefix: &str,
        max_attempts: u32,
    ) -> Result<String, String> {
        crate::vcs::git::commands::generate_unique_branch_name(repo_root, prefix, max_attempts)
            .await
            .map_err(|e| e.to_string())
    }

    async fn worktree_add(
        &self,
        _repo_root: &Path,
        worktree_path: &str,
        branch_name: &str,
        base_commit: &str,
    ) -> Result<(), String> {
        self.backend
            .create_without_setup(Path::new(worktree_path), branch_name, base_commit)
            .await
            .map_err(|e| e.to_string())
    }

    async fn validate_worktree_command_cwd(
        &self,
        repo_root: &Path,
        worktree_path: &Path,
    ) -> Result<(), String> {
        crate::vcs::git::commands::validate_worktree_command_cwd(repo_root, worktree_path)
            .await
            .map_err(|e| e.to_string())
    }

    async fn run_worktree_setup(
        &self,
        repo_root: &Path,
        worktree_path: &Path,
    ) -> Result<bool, String> {
        crate::vcs::git::commands::run_worktree_setup(repo_root, worktree_path)
            .await
            .map(|report| report.ran)
            .map_err(|e| e.to_string())
    }

    async fn worktree_remove_after_setup_failure(
        &self,
        _repo_root: &Path,
        worktree_path: &str,
    ) -> Result<(), String> {
        self.backend
            .remove_after_setup_failure(Path::new(worktree_path))
            .await
            .map_err(|e| e.to_string())
    }
}

/// Which of the three cwd validations refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CwdPhase {
    /// Immediately after `worktree_add`.
    AfterCreate,
    /// Immediately after `.wt/setup`.
    AfterSetup,
    /// Immediately before the command is launched.
    BeforeLaunch,
}

impl CwdPhase {
    /// Stable token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AfterCreate => "after_create",
            Self::AfterSetup => "after_setup",
            Self::BeforeLaunch => "before_launch",
        }
    }
}

/// What `.wt/setup` did for a provisioned session worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupFacts {
    /// A `.wt/setup` script existed and ran.
    pub ran: bool,
    /// Setup did not fail (also true when no script existed).
    pub succeeded: bool,
}

/// What the setup-failure cleanup did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupOutcome {
    /// The worktree directory and registration are gone. The branch is kept.
    Removed,
    /// The worktree could not be removed and is still present.
    Retained {
        /// Why removal failed. Absolute paths are redacted by the caller.
        reason: String,
    },
}

/// A provisioned worktree, ready for launch.
#[derive(Debug, Clone)]
pub struct ProvisionedWorktree {
    /// Absolute path. Internal; never serialized.
    pub path: PathBuf,
    /// The new `ws-session-*` branch.
    pub branch: String,
    /// The expanded configured command. Internal; never serialized.
    pub command: String,
    /// What setup did.
    pub setup: SetupFacts,
    /// The cwd validations that passed, in order.
    #[allow(dead_code)] // Asserted by the provisioner tests.
    pub validations: Vec<CwdPhase>,
}

/// Why provisioning stopped before a launch.
#[derive(Debug, Clone)]
pub enum ProvisionError {
    /// Not a Git repository, or no `worktree_command` configured.
    Unavailable(String),
    /// A step before or at worktree creation failed; nothing was created.
    Internal(String),
    /// A cwd validation refused. The created worktree is left exactly as the
    /// TUI leaves it in the same case.
    CwdInvalid {
        /// Which validation refused.
        phase: CwdPhase,
        /// Diagnostic.
        detail: String,
        /// Absolute path of the created worktree.
        path: PathBuf,
        /// Its branch.
        branch: String,
        /// What setup did, when it had run.
        setup: Option<SetupFacts>,
    },
    /// `.wt/setup` failed and the cleanup ran.
    SetupFailed {
        /// Setup diagnostic.
        detail: String,
        /// Cleanup outcome.
        cleanup: CleanupOutcome,
        /// Absolute path of the created worktree.
        path: PathBuf,
        /// Its retained branch.
        branch: String,
    },
}

/// The TUI `+` provisioning sequence over a seam.
pub struct SessionProvisioner<'a> {
    runtime: &'a dyn SessionProvisionRuntime,
    repo_root: &'a Path,
    worktree_base_dir: &'a Path,
    template: Option<&'a str>,
}

impl<'a> SessionProvisioner<'a> {
    /// Bind the sequence to one repository and one configured template.
    pub fn new(
        runtime: &'a dyn SessionProvisionRuntime,
        repo_root: &'a Path,
        worktree_base_dir: &'a Path,
        template: Option<&'a str>,
    ) -> Self {
        Self {
            runtime,
            repo_root,
            worktree_base_dir,
            template,
        }
    }

    /// Provision a session worktree in the TUI `+` key's exact order.
    pub async fn provision(&self) -> Result<ProvisionedWorktree, ProvisionError> {
        let Some(template) = self.template else {
            return Err(ProvisionError::Unavailable(
                "worktree_command is not configured".to_string(),
            ));
        };

        // 1. Git repository check (`should_trigger_worktree_command`).
        let is_git_repo = self
            .runtime
            .check_git_repo(self.repo_root)
            .await
            .map_err(|e| ProvisionError::Unavailable(format!("failed to check git repo: {e}")))?;
        if !is_git_repo {
            return Err(ProvisionError::Unavailable(
                "the project is not a Git repository".to_string(),
            ));
        }

        // 2. Base directory and the timestamped `proposal-<ms>` path.
        std::fs::create_dir_all(self.worktree_base_dir).map_err(|e| {
            ProvisionError::Internal(format!("failed to prepare worktree base dir: {e}"))
        })?;
        let path = build_session_worktree_path(self.worktree_base_dir);
        let path_str = path
            .to_str()
            .ok_or_else(|| ProvisionError::Internal("failed to resolve worktree path".into()))?
            .to_string();
        let repo_root_str = self
            .repo_root
            .to_str()
            .ok_or_else(|| ProvisionError::Internal("failed to resolve repo root path".into()))?;

        // 3. Server-generated branch.
        let branch = self
            .runtime
            .generate_unique_branch_name(
                self.repo_root,
                SESSION_BRANCH_PREFIX,
                SESSION_BRANCH_ATTEMPTS,
            )
            .await
            .map_err(|e| {
                ProvisionError::Internal(format!("failed to generate unique branch name: {e}"))
            })?;

        // 4. Worktree at base `HEAD`.
        self.runtime
            .worktree_add(self.repo_root, &path_str, &branch, "HEAD")
            .await
            .map_err(|e| ProvisionError::Internal(format!("failed to create worktree: {e}")))?;

        let mut validations = Vec::with_capacity(3);

        // 5. Validate after create.
        self.validate(&path, &branch, CwdPhase::AfterCreate, None)
            .await?;
        validations.push(CwdPhase::AfterCreate);

        // 6. Setup is fatal here, exactly as for the TUI `+` key, with cleanup.
        let ran = match self.runtime.run_worktree_setup(self.repo_root, &path).await {
            Ok(ran) => ran,
            Err(detail) => {
                let cleanup = match self
                    .runtime
                    .worktree_remove_after_setup_failure(self.repo_root, &path_str)
                    .await
                {
                    Ok(()) => CleanupOutcome::Removed,
                    Err(reason) => CleanupOutcome::Retained { reason },
                };
                return Err(ProvisionError::SetupFailed {
                    detail,
                    cleanup,
                    path,
                    branch,
                });
            }
        };
        let setup = SetupFacts {
            ran,
            succeeded: true,
        };

        // 7-8. Validate after setup and immediately before launch.
        self.validate(&path, &branch, CwdPhase::AfterSetup, Some(setup))
            .await?;
        validations.push(CwdPhase::AfterSetup);
        self.validate(&path, &branch, CwdPhase::BeforeLaunch, Some(setup))
            .await?;
        validations.push(CwdPhase::BeforeLaunch);

        // 9. Expand server-side.
        let command = crate::config::OrchestratorConfig::expand_worktree_command(
            template,
            &path_str,
            repo_root_str,
        );

        Ok(ProvisionedWorktree {
            path,
            branch,
            command,
            setup,
            validations,
        })
    }

    async fn validate(
        &self,
        path: &Path,
        branch: &str,
        phase: CwdPhase,
        setup: Option<SetupFacts>,
    ) -> Result<(), ProvisionError> {
        self.runtime
            .validate_worktree_command_cwd(self.repo_root, path)
            .await
            .map_err(|detail| ProvisionError::CwdInvalid {
                phase,
                detail,
                path: path.to_path_buf(),
                branch: branch.to_string(),
                setup,
            })
    }
}

/// The TUI `+` key's timestamped worktree path, shared rather than copied.
pub fn build_session_worktree_path(base_dir: &Path) -> PathBuf {
    crate::tui::worktrees::build_worktree_path(base_dir)
}

// ============================================================================
// Session registry
// ============================================================================

/// Whether a session runs in an existing worktree or a provisioned one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// `run_worktree_command` in an existing worktree.
    Existing,
    /// `create_session_worktree` in a fresh `ws-session-*` worktree.
    Session,
}

impl SessionKind {
    /// Stable token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Existing => "existing",
            Self::Session => "session",
        }
    }
}

/// Closed session lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// The runner invocation has not settled.
    Running,
    /// The final attempt exited 0.
    Succeeded,
    /// Anything else that was not a stop, including inactivity and
    /// runtime-limit terminations.
    Failed,
    /// Stopped through the runner's cancellation path or owner shutdown.
    Stopped,
}

impl SessionState {
    /// Stable token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// Which stream an output line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

impl OutputStream {
    /// Stable token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// One retained output line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedLine {
    /// Source stream.
    pub stream: OutputStream,
    /// `sanitize_detail`-ed text: control sequences stripped, at most 8 KiB,
    /// not redacted.
    pub text: String,
}

/// Bounded newest-first-dropped output tail.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutputTail {
    /// Retained lines, oldest first.
    pub lines: Vec<RetainedLine>,
    /// Bytes of line text retained.
    pub retained_bytes: usize,
    /// Bytes of line text dropped to stay within [`MAX_OUTPUT_BYTES`].
    pub truncated_bytes: u64,
}

/// The ring itself: whole lines only, never split.
#[derive(Debug, Default)]
struct OutputRing {
    lines: VecDeque<RetainedLine>,
    retained_bytes: usize,
    truncated_bytes: u64,
}

impl OutputRing {
    fn push(&mut self, stream: OutputStream, raw: &str) {
        let text = crate::events::sanitize_detail(raw);
        self.retained_bytes += text.len();
        self.lines.push_back(RetainedLine { stream, text });
        while self.retained_bytes > MAX_OUTPUT_BYTES {
            let Some(dropped) = self.lines.pop_front() else {
                break;
            };
            self.retained_bytes -= dropped.text.len();
            self.truncated_bytes += dropped.text.len() as u64;
        }
    }

    fn tail(&self) -> OutputTail {
        OutputTail {
            lines: self.lines.iter().cloned().collect(),
            retained_bytes: self.retained_bytes,
            truncated_bytes: self.truncated_bytes,
        }
    }
}

/// One runner-generated fact retained for a session.
///
/// Never derived from child output. `detail` carries numbers and fixed tokens
/// only — never the template, the expanded command, the environment, or a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerNoticeRecord {
    /// Stable kind token.
    pub kind: &'static str,
    /// The attempt the notice concerns, when it concerns one.
    pub attempt: Option<u32>,
    /// `sanitize_detail`-ed facts, when the kind has any.
    pub detail: Option<String>,
}

/// Retained runner notices, oldest dropped first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunnerNotices {
    /// Retained notices, oldest first, at most [`MAX_RUNNER_NOTICES`].
    pub notices: Vec<RunnerNoticeRecord>,
    /// Notices dropped to stay within [`MAX_RUNNER_NOTICES`].
    pub dropped: u64,
}

impl RunnerNotices {
    fn push(&mut self, record: RunnerNoticeRecord) {
        self.notices.push(record);
        while self.notices.len() > MAX_RUNNER_NOTICES {
            self.notices.remove(0);
            self.dropped = self.dropped.saturating_add(1);
        }
    }
}

/// The final leader fact the runner reported for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaderFact {
    /// The leader was reaped with this status.
    Reaped {
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
    /// The leader was not reaped; its status is unknown.
    NotReaped,
}

/// A coherent read of one session.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    /// Random 128-bit hexadecimal session ID.
    pub session_id: String,
    /// Existing or provisioned worktree.
    pub kind: SessionKind,
    /// Opaque worktree ID, when one was allocated.
    pub worktree_id: Option<String>,
    /// Repository-relative display path.
    pub path: String,
    /// Checked-out branch.
    pub branch: String,
    /// Lifecycle state.
    pub state: SessionState,
    /// Runner attempts started inside this one session: `1` at launch, then
    /// the attempt of each later successful spawn the runner reported. A retry
    /// merely scheduled does not count.
    pub attempt: u32,
    /// Launch instant.
    pub started_at: DateTime<Utc>,
    /// Settlement instant.
    pub finished_at: Option<DateTime<Utc>>,
    /// Exit code of the final attempt, when it exited on its own.
    pub exit_code: Option<i32>,
    /// The signal that terminated the leader as reaped; `None` when it exited
    /// on its own or when its status is unknown.
    ///
    /// For a stopped (or runtime-limited) session this is the reaped leader's
    /// real signal — `SIGTERM`, or `SIGKILL` after the runner's escalation —
    /// and both this and `exit_code` are `None` when the leader was not reaped.
    pub signal: Option<String>,
    /// A stop or owner shutdown was requested.
    pub stop_requested: bool,
    /// Setup facts of a provisioned session.
    pub setup: Option<SetupFacts>,
    /// Setup-failure cleanup facts. Always `None` for a launched session.
    pub cleanup: Option<CleanupOutcome>,
    /// Whether the runner proved the process group quiescent; `None` while running.
    pub process_group_confirmed: Option<bool>,
    /// Bounded sanitized output tail: only what the child wrote.
    pub output: OutputTail,
    /// Bounded runner-generated notices.
    pub runner_notices: RunnerNotices,
}

/// Everything a launch needs. `path` and `command` are internal.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    /// Existing or provisioned worktree.
    pub kind: SessionKind,
    /// Opaque worktree ID.
    pub worktree_id: Option<String>,
    /// Repository-relative display path.
    pub display_path: String,
    /// Branch.
    pub branch: String,
    /// Absolute cwd. Never serialized.
    pub path: PathBuf,
    /// Expanded configured command. Never serialized.
    pub command: String,
    /// Setup facts of a provisioned session.
    pub setup: Option<SetupFacts>,
}

/// Why a reservation or launch was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRefusal {
    /// A session is running (or reserved) for that worktree.
    WorktreeBusy,
    /// [`MAX_RUNNING_SESSIONS`] are already running or reserved.
    Capacity,
    /// The owner is shutting down and admits nothing new.
    Closed,
    /// The runner refused to start the invocation.
    LaunchFailed(String),
}

/// What a stop did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopOutcome {
    /// The session was running when the stop arrived.
    pub was_running: bool,
    /// The runner cancelled it and proved the process group quiescent.
    pub terminated: bool,
    /// State after settlement.
    pub state_after: SessionState,
}

/// One state event a session publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEvent {
    /// `worktree_session_started` or `worktree_session_finished`.
    pub event_type: &'static str,
    /// Session ID.
    pub session_id: String,
    /// Opaque worktree ID.
    pub worktree_id: Option<String>,
    /// State at the event.
    pub state: SessionState,
    /// Exit code at the event.
    pub exit_code: Option<i32>,
}

/// Event type of a launched session.
pub const EVENT_SESSION_STARTED: &str = "worktree_session_started";
/// Event type of a settled session.
pub const EVENT_SESSION_FINISHED: &str = "worktree_session_finished";

/// Where session state events go. Output never travels here.
pub trait SessionEventSink: Send + Sync {
    /// Publish one event.
    fn publish(&self, event: &SessionEvent);
}

struct Entry {
    snapshot: SessionSnapshot,
    /// Canonical cwd. Internal; never serialized.
    path: PathBuf,
    output: OutputRing,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// The last leader fact the runner reported.
    leader: Option<LeaderFact>,
}

#[derive(Default)]
struct Inner {
    closed: bool,
    /// Insertion order, oldest first.
    entries: Vec<Entry>,
    reservations: Vec<(u64, Option<PathBuf>)>,
    next_reservation: u64,
}

impl Inner {
    fn running(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.snapshot.state == SessionState::Running)
            .count()
    }

    fn busy(&self, path: &Path) -> bool {
        self.entries
            .iter()
            .any(|e| e.snapshot.state == SessionState::Running && e.path == path)
            || self
                .reservations
                .iter()
                .any(|(_, reserved)| reserved.as_deref() == Some(path))
    }

    fn entry_mut(&mut self, session_id: &str) -> Option<&mut Entry> {
        self.entries
            .iter_mut()
            .find(|e| e.snapshot.session_id == session_id)
    }

    fn snapshot(entry: &Entry) -> SessionSnapshot {
        let mut snapshot = entry.snapshot.clone();
        snapshot.output = entry.output.tail();
        snapshot
    }

    /// Evict finished sessions beyond [`MAX_RETAINED_FINISHED`], oldest
    /// *launched* first. Running sessions are never evicted.
    fn evict_finished(&mut self) {
        let mut finished = self
            .entries
            .iter()
            .filter(|e| e.snapshot.state != SessionState::Running)
            .count();
        while finished > MAX_RETAINED_FINISHED {
            if let Some(index) = self
                .entries
                .iter()
                .position(|e| e.snapshot.state != SessionState::Running)
            {
                self.entries.remove(index);
            }
            finished -= 1;
        }
    }
}

/// A held running slot, released on drop unless a launch converted it.
pub struct SessionReservation {
    registry: Weak<WorktreeSessionRegistry>,
    id: u64,
    converted: bool,
}

impl SessionReservation {
    /// Bind the reservation to the provisioned path once it exists.
    pub fn bind_path(&self, path: &Path) {
        if let Some(registry) = self.registry.upgrade() {
            let canonical = canonical(path);
            let mut inner = registry.lock();
            if let Some(slot) = inner.reservations.iter_mut().find(|(id, _)| *id == self.id) {
                slot.1 = Some(canonical);
            }
        }
    }
}

impl Drop for SessionReservation {
    fn drop(&mut self) {
        if self.converted {
            return;
        }
        if let Some(registry) = self.registry.upgrade() {
            registry
                .lock()
                .reservations
                .retain(|(id, _)| *id != self.id);
        }
    }
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Process-local configured-command sessions.
pub struct WorktreeSessionRegistry {
    inner: Mutex<Inner>,
    scope: RunCommandScope,
    runner: AiCommandRunner,
    events: RwLock<Option<Arc<dyn SessionEventSink>>>,
    changed: tokio::sync::Notify,
    launches: AtomicU64,
}

impl WorktreeSessionRegistry {
    /// Own a fresh scope and bind `runner` to it.
    ///
    /// `runner` is the production runner clone the TUI uses, so stagger, retry
    /// policy, and process-group handling are the TUI's own.
    pub fn new(mut runner: AiCommandRunner) -> Arc<Self> {
        let scope = RunCommandScope::new();
        runner.set_run_command_scope(scope.clone());
        Arc::new(Self {
            inner: Mutex::new(Inner::default()),
            scope,
            runner,
            events: RwLock::new(None),
            changed: tokio::sync::Notify::new(),
            launches: AtomicU64::new(0),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The scope every session runner is bound to.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn scope(&self) -> &RunCommandScope {
        &self.scope
    }

    /// Route state events to an observer.
    pub fn set_event_sink(&self, sink: Arc<dyn SessionEventSink>) {
        *self
            .events
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(sink);
    }

    fn publish(&self, event: SessionEvent) {
        let sink = self
            .events
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(sink) = sink {
            sink.publish(&event);
        }
    }

    /// True once owner shutdown closed admission.
    pub fn is_closed(&self) -> bool {
        self.lock().closed
    }

    /// Runner invocations this registry has started.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn launch_count(&self) -> u64 {
        self.launches.load(Ordering::SeqCst)
    }

    /// Running sessions.
    pub fn running_count(&self) -> usize {
        self.lock().running()
    }

    /// Reserve a running slot before any Git or process effect.
    ///
    /// `path` is the existing worktree a launch will target; a provisioned
    /// session reserves with `None` and binds its path once it exists. The
    /// worktree-busy and capacity checks share one lock with the launch that
    /// converts the reservation, so two concurrent requests cannot both pass.
    pub fn reserve(
        self: &Arc<Self>,
        path: Option<&Path>,
    ) -> Result<SessionReservation, SessionRefusal> {
        let canonical_path = path.map(canonical);
        let mut inner = self.lock();
        if inner.closed {
            return Err(SessionRefusal::Closed);
        }
        if let Some(path) = canonical_path.as_deref() {
            if inner.busy(path) {
                return Err(SessionRefusal::WorktreeBusy);
            }
        }
        if inner.running() + inner.reservations.len() >= MAX_RUNNING_SESSIONS {
            return Err(SessionRefusal::Capacity);
        }
        inner.next_reservation += 1;
        let id = inner.next_reservation;
        inner.reservations.push((id, canonical_path));
        Ok(SessionReservation {
            registry: Arc::downgrade(self),
            id,
            converted: false,
        })
    }

    /// Convert a reservation into a running session and start the runner.
    pub async fn launch(
        self: &Arc<Self>,
        mut reservation: SessionReservation,
        spec: LaunchSpec,
    ) -> Result<SessionSnapshot, SessionRefusal> {
        let session_id = crate::ids::new_hex_id();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        {
            let mut inner = self.lock();
            if inner.closed {
                return Err(SessionRefusal::Closed);
            }
            inner.reservations.retain(|(id, _)| *id != reservation.id);
            reservation.converted = true;
            inner.entries.push(Entry {
                snapshot: SessionSnapshot {
                    session_id: session_id.clone(),
                    kind: spec.kind,
                    worktree_id: spec.worktree_id.clone(),
                    path: spec.display_path.clone(),
                    branch: spec.branch.clone(),
                    state: SessionState::Running,
                    attempt: 1,
                    started_at: Utc::now(),
                    finished_at: None,
                    exit_code: None,
                    signal: None,
                    stop_requested: false,
                    setup: spec.setup,
                    cleanup: None,
                    process_group_confirmed: None,
                    output: OutputTail::default(),
                    runner_notices: RunnerNotices::default(),
                },
                path: canonical(&spec.path),
                output: OutputRing::default(),
                stop_tx: Some(stop_tx),
                leader: None,
            });
        }

        let launched = self
            .runner
            .execute_streaming_with_retry_observed(
                &spec.command,
                Some(&spec.path),
                Some(SESSION_OPERATION),
                None,
            )
            .await;
        let (handle, rx, notices) = match launched {
            Ok(launched) => launched,
            Err(error) => {
                let detail = error.to_string();
                let mut inner = self.lock();
                inner
                    .entries
                    .retain(|e| e.snapshot.session_id != session_id);
                drop(inner);
                self.changed.notify_waiters();
                return Err(SessionRefusal::LaunchFailed(detail));
            }
        };
        self.launches.fetch_add(1, Ordering::SeqCst);

        self.publish(SessionEvent {
            event_type: EVENT_SESSION_STARTED,
            session_id: session_id.clone(),
            worktree_id: spec.worktree_id.clone(),
            state: SessionState::Running,
            exit_code: None,
        });

        let registry = self.clone();
        let supervised = session_id.clone();
        tokio::spawn(async move {
            registry
                .supervise(supervised, handle, rx, notices, stop_rx)
                .await;
        });

        Ok(self
            .get(&session_id)
            .expect("a just-launched session is running and never evicted"))
    }

    /// Drain output and notices, forward a stop to the runner, then settle
    /// from the runner's own exit, termination, cleanup, and leader evidence.
    async fn supervise(
        self: Arc<Self>,
        session_id: String,
        mut handle: StreamingChildHandle,
        mut rx: tokio::sync::mpsc::Receiver<OutputLine>,
        mut notices: tokio::sync::mpsc::UnboundedReceiver<RunnerNotice>,
        mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        let mut stop_pending = true;
        let mut notices_open = true;
        loop {
            tokio::select! {
                line = rx.recv() => match line {
                    Some(line) => self.record_output(&session_id, line),
                    None => break,
                },
                notice = notices.recv(), if notices_open => match notice {
                    Some(notice) => self.record_notice(&session_id, notice),
                    None => notices_open = false,
                },
                requested = &mut stop_rx, if stop_pending => {
                    stop_pending = false;
                    if requested.is_ok() {
                        // The runner's own cancellation path: SIGTERM to the
                        // group, SIGKILL after 5 s, verified cleanup, and no
                        // further attempt.
                        let _ = handle.terminate();
                    }
                }
            }
        }

        // The runner closes the notice channel only when its task ends, after
        // its last notice, so classification never races a late leader fact.
        while let Some(notice) = notices.recv().await {
            self.record_notice(&session_id, notice);
        }

        let status = handle.wait().await;
        let termination = handle.termination().await;
        let cleanup = handle.process_group_cleanup().await;
        let confirmed = cleanup.is_confirmed();
        if !confirmed {
            warn!(
                session_id = %session_id,
                "worktree session process-group cleanup unconfirmed: {}",
                cleanup.diagnostics()
            );
        }

        let event = {
            let mut inner = self.lock();
            let Some(entry) = inner.entry_mut(&session_id) else {
                return;
            };
            let stop_requested = entry.snapshot.stop_requested;
            let (state, exit_code, signal) =
                classify(&status, termination, stop_requested, entry.leader);
            entry.snapshot.state = state;
            entry.snapshot.exit_code = exit_code;
            entry.snapshot.signal = signal;
            entry.snapshot.finished_at = Some(Utc::now());
            entry.snapshot.process_group_confirmed = Some(confirmed);
            entry.stop_tx = None;
            let event = SessionEvent {
                event_type: EVENT_SESSION_FINISHED,
                session_id: session_id.clone(),
                worktree_id: entry.snapshot.worktree_id.clone(),
                state,
                exit_code,
            };
            inner.evict_finished();
            event
        };
        self.changed.notify_waiters();
        self.publish(event);
    }

    fn record_output(&self, session_id: &str, line: OutputLine) {
        let mut inner = self.lock();
        let Some(entry) = inner.entry_mut(session_id) else {
            return;
        };
        // Every line on this channel is child data: the runner publishes its
        // own facts on the notice channel, so nothing here changes a typed
        // member.
        let (stream, text) = match &line {
            OutputLine::Stdout(text) => (OutputStream::Stdout, text),
            OutputLine::Stderr(text) => (OutputStream::Stderr, text),
        };
        entry.output.push(stream, text);
    }

    fn record_notice(&self, session_id: &str, notice: RunnerNotice) {
        let mut inner = self.lock();
        let Some(entry) = inner.entry_mut(session_id) else {
            return;
        };
        apply_notice(entry, notice);
    }

    /// Every retained session, newest first.
    pub fn list(&self) -> Vec<SessionSnapshot> {
        let inner = self.lock();
        inner.entries.iter().rev().map(Inner::snapshot).collect()
    }

    /// One session.
    pub fn get(&self, session_id: &str) -> Option<SessionSnapshot> {
        let inner = self.lock();
        inner
            .entries
            .iter()
            .find(|e| e.snapshot.session_id == session_id)
            .map(Inner::snapshot)
    }

    /// Stop one session through the runner's cancellation handle and wait for
    /// the reap and the cleanup report. `None` when the ID is unknown.
    pub async fn stop(&self, session_id: &str) -> Option<StopOutcome> {
        {
            let mut inner = self.lock();
            let entry = inner.entry_mut(session_id)?;
            if entry.snapshot.state != SessionState::Running {
                return Some(StopOutcome {
                    was_running: false,
                    terminated: false,
                    state_after: entry.snapshot.state,
                });
            }
            entry.snapshot.stop_requested = true;
            if let Some(tx) = entry.stop_tx.take() {
                let _ = tx.send(());
            }
        }

        let finished = self.wait_settled(Some(session_id)).await;
        let (state_after, confirmed) = finished.unwrap_or((SessionState::Stopped, Some(false)));
        Some(StopOutcome {
            was_running: true,
            terminated: state_after == SessionState::Stopped && confirmed == Some(true),
            state_after,
        })
    }

    /// Wait until `session_id` (or, with `None`, every session) is no longer running.
    async fn wait_settled(&self, session_id: Option<&str>) -> Option<(SessionState, Option<bool>)> {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let inner = self.lock();
                match session_id {
                    Some(id) => match inner.entries.iter().find(|e| e.snapshot.session_id == id) {
                        None => return None,
                        Some(entry) if entry.snapshot.state != SessionState::Running => {
                            return Some((
                                entry.snapshot.state,
                                entry.snapshot.process_group_confirmed,
                            ))
                        }
                        Some(_) => {}
                    },
                    None if inner.running() == 0 => return None,
                    None => {}
                }
            }
            notified.await;
        }
    }

    /// Owner shutdown: refuse new sessions, close the scope under `deadline`
    /// (which cancels every running session through the runner's own path and
    /// admits no further attempt), then wait for the sessions to settle.
    pub async fn shutdown(
        &self,
        deadline: Duration,
    ) -> crate::ai_command_runner::RunCommandScopeCleanup {
        let started = tokio::time::Instant::now();
        {
            let mut inner = self.lock();
            inner.closed = true;
            for entry in inner.entries.iter_mut() {
                if entry.snapshot.state == SessionState::Running {
                    entry.snapshot.stop_requested = true;
                }
            }
        }
        let cleanup = self.scope.shutdown(deadline).await;
        let remaining = deadline.saturating_sub(started.elapsed());
        let _ = tokio::time::timeout(remaining, self.wait_settled(None)).await;
        if !cleanup.is_quiescent() {
            warn!(
                "worktree session shutdown could not prove every process group quiescent: {}",
                cleanup.diagnostics()
            );
        }
        cleanup
    }
}

impl SessionProbe for WorktreeSessionRegistry {
    fn session_running(&self, path: &Path) -> bool {
        let path = canonical(path);
        self.lock().busy(&path)
    }
}

/// Fold one runner notice into a session.
///
/// `AttemptStarted` is the only source of `attempt`, and the leader facts the
/// only source of a cancelled run's `exit_code`/`signal`; the rest is retained
/// as bounded, sanitized records.
fn apply_notice(entry: &mut Entry, notice: RunnerNotice) {
    let current = entry.snapshot.attempt;
    let record =
        |kind: &'static str, attempt: Option<u32>, detail: Option<String>| RunnerNoticeRecord {
            kind,
            attempt,
            detail: detail.map(|d| crate::events::sanitize_detail(&d)),
        };
    let retained = match notice {
        RunnerNotice::AttemptStarted { attempt } => {
            entry.snapshot.attempt = attempt;
            entry.leader = None;
            None
        }
        RunnerNotice::LeaderReaped {
            exit_code, signal, ..
        } => {
            entry.leader = Some(LeaderFact::Reaped { exit_code, signal });
            None
        }
        RunnerNotice::LeaderNotReaped { .. } => {
            entry.leader = Some(LeaderFact::NotReaped);
            None
        }
        RunnerNotice::RetryScheduled {
            next_attempt,
            max_attempts,
            cause,
            delay_ms,
        } => Some(record(
            "retry_scheduled",
            Some(next_attempt),
            Some(format!(
                "cause={}, max_attempts={max_attempts}, delay_ms={delay_ms}",
                match cause {
                    RetryCause::Crash => "crash",
                    RetryCause::InactivityTimeout => "inactivity_timeout",
                }
            )),
        )),
        RunnerNotice::InactivityTerminated {
            attempt,
            timeout_secs,
        } => Some(record(
            "inactivity_terminated",
            Some(attempt),
            Some(format!("timeout_secs={timeout_secs}")),
        )),
        RunnerNotice::RuntimeLimitReached { attempt } => {
            Some(record("runtime_limit_reached", Some(attempt), None))
        }
        RunnerNotice::LaunchFailed { attempt, stage } => Some(record(
            "launch_failed",
            Some(attempt),
            Some(format!("stage={stage}")),
        )),
        RunnerNotice::ShutdownRefused { stage } => Some(record(
            "shutdown_refused",
            None,
            Some(format!("stage={stage}")),
        )),
        RunnerNotice::ProcessGroupCleanupUnconfirmed { pgid, force_killed } => Some(record(
            "process_group_cleanup_unconfirmed",
            Some(current),
            Some(format!("pgid={pgid}, force_killed={force_killed}")),
        )),
    };
    if let Some(record) = retained {
        entry.snapshot.runner_notices.push(record);
    }
}

/// Classify a settled runner invocation.
///
/// A cancelled or runtime-limited invocation's status is synthetic, so its
/// `exit_code` and `signal` come from the reaped leader the runner reported —
/// the real signal, `SIGKILL` included after escalation — and are both `None`
/// when the leader was not reaped.
fn classify(
    status: &std::io::Result<std::process::ExitStatus>,
    termination: CommandTermination,
    stop_requested: bool,
    leader: Option<LeaderFact>,
) -> (SessionState, Option<i32>, Option<String>) {
    let reaped = || match leader {
        Some(LeaderFact::Reaped { exit_code, signal }) => (exit_code, signal.map(signal_name)),
        Some(LeaderFact::NotReaped) | None => (None, None),
    };
    match termination {
        CommandTermination::Cancelled => {
            let (exit_code, signal) = reaped();
            (SessionState::Stopped, exit_code, signal)
        }
        CommandTermination::NotStarted if stop_requested => (SessionState::Stopped, None, None),
        CommandTermination::NotStarted => (SessionState::Failed, None, None),
        CommandTermination::RuntimeLimit => {
            let (exit_code, signal) = reaped();
            (SessionState::Failed, exit_code, signal)
        }
        CommandTermination::Exited | CommandTermination::InactivityTimeout => match status {
            Ok(status) => {
                let state = if status.success() {
                    SessionState::Succeeded
                } else {
                    SessionState::Failed
                };
                (state, status.code(), exit_signal(status))
            }
            Err(_) => (SessionState::Failed, None, None),
        },
    }
}

#[cfg(unix)]
fn exit_signal(status: &std::process::ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(signal_name)
}

#[cfg(not(unix))]
fn exit_signal(_status: &std::process::ExitStatus) -> Option<String> {
    None
}

#[cfg(unix)]
fn signal_name(number: i32) -> String {
    nix::sys::signal::Signal::try_from(number)
        .map(|signal| signal.as_str().to_string())
        .unwrap_or_else(|_| format!("SIG{number}"))
}

#[cfg(not(unix))]
fn signal_name(number: i32) -> String {
    format!("SIG{number}")
}

#[cfg(test)]
mod tests;
