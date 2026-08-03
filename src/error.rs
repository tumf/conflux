use crate::vcs::{VcsBackend, VcsError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Agent command failed: {0}")]
    AgentCommand(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("No changes found")]
    NoChanges,

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Config load error: {0}")]
    ConfigLoad(String),

    #[error("Config parse error: {0}")]
    ConfigParse(String),

    #[error("Hook execution failed ({hook_type}): {message}")]
    HookFailed { hook_type: String, message: String },

    #[error("Hook timed out ({hook_type}): exceeded {timeout_secs}s")]
    HookTimeout {
        hook_type: String,
        timeout_secs: u64,
    },

    #[error("Failed to launch editor: {0}")]
    EditorLaunchFailed(String),

    #[error("Change directory not found: {0}")]
    ChangeNotFound(String),

    #[error("VCS error: {0}")]
    Vcs(Box<VcsError>),

    // Legacy error variants kept for backward compatibility
    // These delegate to VcsError internally
    #[error("Git command failed: {0}")]
    GitCommand(String),

    #[error("Git merge conflict: {0}")]
    GitConflict(String),

    #[error("Git has uncommitted changes: {0}")]
    #[allow(dead_code)] // Used when VcsError::UncommittedChanges is converted
    GitUncommittedChanges(String),

    #[error("No VCS backend available for parallel execution")]
    #[allow(dead_code)] // Reserved for future use when git is unavailable
    NoVcsBackend,

    #[error("Permission auto-rejected: {denied_path}\n{guidance}")]
    #[allow(dead_code)]
    // Legacy soft-block variant retained for older serial/parallel apply paths.
    PermissionBlocked {
        denied_path: String,
        guidance: String,
    },

    #[error("Repeated unresolved permission/tool policy denial: {guidance}")]
    PermissionStalled {
        denied_path: String,
        guidance: String,
    },

    /// The sole per-change Apply-dispatch budget owner refused to reserve
    /// another dispatch because the positive `max_iterations` ceiling is spent.
    ///
    /// This is deliberately a distinct variant rather than an untyped
    /// [`OrchestratorError::AgentCommand`]: serial CLI, TUI, and parallel run
    /// boundaries must preserve `iteration_limit` finish-status ownership rather
    /// than reclassify budget exhaustion as an ordinary agent-command crash.
    #[error("Max iterations ({max}) reached for change '{change_id}' after {attempts} Apply dispatch(es): {diagnostic}")]
    IterationLimit {
        change_id: String,
        attempts: u32,
        max: u32,
        diagnostic: String,
    },

    /// An in-flight operation observed explicit cancellation and terminated its
    /// child.
    ///
    /// Typed rather than an untyped [`OrchestratorError::AgentCommand`] so run
    /// boundaries can report one intentional stop for both global cancellation
    /// and a per-change queue stop instead of an execution failure. The
    /// rendering is unchanged, so existing message-based handling keeps working.
    #[error("Cancelled {operation} for '{change_id}' in workspace '{workspace}'")]
    Cancelled {
        operation: String,
        change_id: String,
        workspace: String,
    },
}

impl OrchestratorError {
    /// Explicit cancellation of `operation` for `change_id` in `workspace`.
    pub fn cancelled(
        operation: impl Into<String>,
        change_id: impl Into<String>,
        workspace: &std::path::Path,
    ) -> Self {
        OrchestratorError::Cancelled {
            operation: operation.into(),
            change_id: change_id.into(),
            workspace: workspace.display().to_string(),
        }
    }

    /// True when this error is an explicit operator/queue cancellation rather
    /// than a failure.
    pub fn is_cancellation(&self) -> bool {
        matches!(self, OrchestratorError::Cancelled { .. })
    }
}

#[allow(dead_code)] // Legacy API helpers, kept for backward compatibility
impl OrchestratorError {
    /// Create a GitCommand error (legacy, prefer VcsError::git_command)
    pub fn git_command(msg: impl Into<String>) -> Self {
        OrchestratorError::GitCommand(msg.into())
    }

    /// Create an error from VcsError with proper variant mapping
    pub fn from_vcs_error(err: VcsError) -> Self {
        match err {
            VcsError::Command {
                backend,
                message,
                command,
                working_dir,
                stderr,
                stdout,
            } => {
                // Use the Display implementation which includes full context
                let full_message = format!(
                    "{}",
                    VcsError::Command {
                        backend,
                        message: message.clone(),
                        command: command.clone(),
                        working_dir: working_dir.clone(),
                        stderr: stderr.clone(),
                        stdout: stdout.clone(),
                    }
                );
                match backend {
                    VcsBackend::Git => OrchestratorError::GitCommand(full_message),
                    VcsBackend::Auto => OrchestratorError::Vcs(Box::new(VcsError::Command {
                        backend,
                        message,
                        command,
                        working_dir,
                        stderr,
                        stdout,
                    })),
                }
            }
            VcsError::Conflict { backend, details } => match backend {
                VcsBackend::Git => OrchestratorError::GitConflict(details),
                VcsBackend::Auto => {
                    OrchestratorError::Vcs(Box::new(VcsError::Conflict { backend, details }))
                }
            },
            VcsError::NotAvailable { .. } => OrchestratorError::NoVcsBackend,
            VcsError::UncommittedChanges(msg) => OrchestratorError::GitUncommittedChanges(msg),
            VcsError::NoBackend => OrchestratorError::NoVcsBackend,
            VcsError::Io(e) => OrchestratorError::Io(e),
        }
    }
}

impl From<VcsError> for OrchestratorError {
    fn from(err: VcsError) -> Self {
        OrchestratorError::Vcs(Box::new(err))
    }
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;
