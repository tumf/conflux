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
    Vcs(#[from] VcsError),

    // Legacy error variants kept for backward compatibility
    // These delegate to VcsError internally
    #[error("jj command failed: {0}")]
    JjCommand(String),

    #[error("jj conflict detected: {0}")]
    JjConflict(String),

    /// Reserved for future use when jj availability check fails.
    /// Currently check_jj_repo returns false instead of this error.
    #[error("jj not available: {0}")]
    #[allow(dead_code)] // Kept for future use in stricter jj availability checks
    JjNotAvailable(String),

    #[error("Git command failed: {0}")]
    GitCommand(String),

    #[error("Git merge conflict: {0}")]
    GitConflict(String),

    #[error("Git has uncommitted changes: {0}")]
    #[allow(dead_code)] // Used when VcsError::UncommittedChanges is converted
    GitUncommittedChanges(String),

    #[error("No VCS backend available for parallel execution")]
    #[allow(dead_code)] // Reserved for future use when both jj and git are unavailable
    NoVcsBackend,
}

#[allow(dead_code)] // Legacy API helpers, kept for backward compatibility
impl OrchestratorError {
    /// Create a JjCommand error (legacy, prefer VcsError::jj_command)
    pub fn jj_command(msg: impl Into<String>) -> Self {
        OrchestratorError::JjCommand(msg.into())
    }

    /// Create a GitCommand error (legacy, prefer VcsError::git_command)
    pub fn git_command(msg: impl Into<String>) -> Self {
        OrchestratorError::GitCommand(msg.into())
    }

    /// Create an error from VcsError with proper variant mapping
    pub fn from_vcs_error(err: VcsError) -> Self {
        match err {
            VcsError::Command { backend, message } => match backend {
                VcsBackend::Jj => OrchestratorError::JjCommand(message),
                VcsBackend::Git => OrchestratorError::GitCommand(message),
                VcsBackend::Auto => OrchestratorError::Vcs(VcsError::Command { backend, message }),
            },
            VcsError::Conflict { backend, details } => match backend {
                VcsBackend::Jj => OrchestratorError::JjConflict(details),
                VcsBackend::Git => OrchestratorError::GitConflict(details),
                VcsBackend::Auto => OrchestratorError::Vcs(VcsError::Conflict { backend, details }),
            },
            VcsError::NotAvailable { backend, reason } => match backend {
                VcsBackend::Jj => OrchestratorError::JjNotAvailable(reason),
                VcsBackend::Git | VcsBackend::Auto => OrchestratorError::NoVcsBackend,
            },
            VcsError::UncommittedChanges(msg) => OrchestratorError::GitUncommittedChanges(msg),
            VcsError::NoBackend => OrchestratorError::NoVcsBackend,
            VcsError::Io(e) => OrchestratorError::Io(e),
        }
    }
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;
