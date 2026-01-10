use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("OpenCode command failed: {0}")]
    #[allow(dead_code)]
    OpenCodeCommand(String),

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

    #[error("jj command failed: {0}")]
    JjCommand(String),

    #[error("jj conflict detected: {0}")]
    JjConflict(String),

    #[error("jj not available: {0}")]
    #[allow(dead_code)]
    JjNotAvailable(String),
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;
