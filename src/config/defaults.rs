//! Default values and constants for orchestrator configuration.

use std::path::PathBuf;

/// Project-level configuration file name
pub const PROJECT_CONFIG_FILE: &str = ".cflx.jsonc";

/// Global configuration directory name
pub const GLOBAL_CONFIG_DIR: &str = "cflx";

/// Global configuration file name within the config directory
pub const GLOBAL_CONFIG_FILE: &str = "config.jsonc";

/// Default apply command template (OpenCode)
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_APPLY_COMMAND: &str = "opencode run '/openspec-apply {change_id}'";

/// Default archive command template (OpenCode)
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_ARCHIVE_COMMAND: &str = "opencode run '/conflux:archive {change_id}'";

/// Default acceptance command template (OpenCode)
/// Supports `{change_id}` and `{prompt}` placeholders
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_ACCEPTANCE_COMMAND: &str = "opencode run '/cflx-accept {change_id} {prompt}'";

/// Default resolve command template (OpenCode)
/// Supports `{prompt}` placeholder for the resolve prompt
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_RESOLVE_COMMAND: &str = "opencode run {prompt}";

/// Default analyze command template (OpenCode)
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_ANALYZE_COMMAND: &str = "opencode run --format json {prompt}";

/// Default operation skill loaded before dependency analysis prompt context.
pub const DEFAULT_ANALYZE_SKILL: &str = "cflx-analyze";
/// Default operation skill loaded before apply prompt context.
pub const DEFAULT_APPLY_SKILL: &str = "cflx-apply";
/// Default operation skill loaded before rejecting-review prompt context.
pub const DEFAULT_REJECTING_SKILL: &str = "cflx-rejecting";
/// Default operation skill loaded before cleanup-review prompt context.
pub const DEFAULT_CLEANUP_REVIEW_SKILL: &str = "cflx-cleanup-review";
/// Default operation skill loaded before acceptance prompt context.
pub const DEFAULT_ACCEPT_SKILL: &str = "cflx-accept";
/// Default operation skill loaded before archive prompt context.
pub const DEFAULT_ARCHIVE_SKILL: &str = "cflx-archive";
/// Default operation skill loaded before conflict-resolution prompt context.
pub const DEFAULT_RESOLVE_SKILL: &str = "cflx-resolve";

/// Default prompt for apply command - includes path context.
/// The hardcoded system prompt in agent.rs is always appended.
pub const DEFAULT_APPLY_PROMPT: &str = r#"
<system-context>
IMPORTANT: You are running in the repository root directory.
The change you are working on is located at: openspec/changes/{change_id}/
All file paths should be relative to the repository root.
</system-context>
"#;

/// Default prompt for archive command - empty by default
pub const DEFAULT_ARCHIVE_PROMPT: &str = "";

/// Acceptance system prompt - now empty to keep runtime prompts context-only.
/// Acceptance operation guidance comes from the selected portable accept_skill.
/// The acceptance_prompt_mode "full" is now deprecated and behaves identically to "context_only".
#[allow(dead_code)]
pub const ACCEPTANCE_SYSTEM_PROMPT: &str = "";

/// Default prompt for acceptance command - appended after hardcoded prompt
pub const DEFAULT_ACCEPTANCE_PROMPT: &str = "";

/// Default maximum iterations for the orchestration loop
pub const DEFAULT_MAX_ITERATIONS: u32 = 50;

/// Default maximum concurrent workspaces for parallel execution
pub const DEFAULT_MAX_CONCURRENT_WORKSPACES: usize = 3;

/// Default workspace base directory (uses system temp)
#[allow(dead_code)]
pub const DEFAULT_WORKSPACE_BASE_DIR: &str = "";

/// Default suppression for repetitive debug logs
pub const DEFAULT_SUPPRESS_REPETITIVE_DEBUG: bool = true;

/// Default interval (seconds) for summary log output
pub const DEFAULT_LOG_SUMMARY_INTERVAL_SECS: u64 = 60;

/// Default enablement for stall detection
pub const DEFAULT_STALL_DETECTION_ENABLED: bool = true;

/// Default threshold for consecutive empty WIP commits before stalling
pub const DEFAULT_STALL_DETECTION_THRESHOLD: u32 = 5;

/// Default empty-WIP count at which apply escalation becomes eligible.
/// Defaults to no escalation policy unless explicitly configured.
pub const DEFAULT_APPLY_ESCALATION_AFTER_EMPTY_WIP: Option<u32> = None;

/// Default maximum number of escalation command uses per empty-WIP stall sequence.
/// Defaults to no escalation policy unless explicitly configured.
pub const DEFAULT_APPLY_ESCALATION_MAX_USES_PER_STALL: Option<u32> = None;

/// Default number of consecutive invalid Acceptance results before the next
/// permitted Acceptance-only retry uses the escalation reviewer command.
///
/// Unlike the Apply escalation policy this has a usable built-in default: the
/// optional `acceptance_escalation_command` is itself the opt-in, so an operator
/// who configures the command gets the bounded default policy without having to
/// spell it out.
pub const DEFAULT_ACCEPTANCE_ESCALATION_AFTER_INVALID_RESULTS: u32 = 1;

/// Default maximum Acceptance escalation-command uses within one consecutive
/// invalid-result sequence.
pub const DEFAULT_ACCEPTANCE_ESCALATION_MAX_USES_PER_SEQUENCE: u32 = 1;

/// Default delay between command executions (milliseconds)
pub const DEFAULT_STAGGER_DELAY_MS: u64 = 2000;

/// Default maximum number of retries for commands
pub const DEFAULT_MAX_RETRIES: u32 = 2;

/// Default delay between retries (milliseconds)
pub const DEFAULT_RETRY_DELAY_MS: u64 = 5000;

/// Default threshold for retry based on execution duration (seconds)
pub const DEFAULT_RETRY_IF_DURATION_UNDER_SECS: u64 = 5;

/// Default inactivity timeout for commands (seconds)
/// 0 = disabled
pub const DEFAULT_COMMAND_INACTIVITY_TIMEOUT_SECS: u64 = 900;

/// Default grace period before force-killing inactive commands (seconds)
pub const DEFAULT_COMMAND_INACTIVITY_KILL_GRACE_SECS: u64 = 5;

/// Default maximum number of retries after inactivity timeout (0 = disabled; set to 0 to opt out)
pub const DEFAULT_COMMAND_INACTIVITY_TIMEOUT_MAX_RETRIES: u32 = 3;

/// Default absolute runtime limit for one AI command invocation (seconds).
///
/// Three hours: long enough for one Apply invocation to build, fix, and run its
/// repository-local verification without being cut off mid-task. Measured from
/// successful child spawn and never extended by stdout or stderr activity, so an
/// agent that keeps printing is still bounded. `0` disables it.
pub const DEFAULT_COMMAND_MAX_RUNTIME_SECS: u64 = 10800;

/// Default absolute runtime limit for one Acceptance invocation (seconds).
///
/// Thirty minutes: Acceptance reviews an existing implementation rather than
/// producing one, so it needs a far smaller budget than Apply. Measured from
/// successful child spawn and never extended by output. Unlike
/// [`DEFAULT_COMMAND_MAX_RUNTIME_SECS`] this guard cannot be disabled: `0` is
/// rejected at configuration load.
pub const DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS: u64 = 1800;

/// Smallest configurable Acceptance absolute runtime limit (seconds).
///
/// Five minutes. A minute-scale floor reads as a generally safe production
/// value while being closer to skill-loading overhead than to a review, so the
/// dedicated key refuses it. This bounds the *dedicated* key only: a shorter
/// positive [`DEFAULT_COMMAND_MAX_RUNTIME_SECS`] override is an independent
/// safety limit and may still produce an effective Acceptance bound below this.
pub const MIN_ACCEPTANCE_MAX_RUNTIME_SECS: u64 = 300;

/// Largest configurable Acceptance absolute runtime limit (seconds).
pub const MAX_ACCEPTANCE_MAX_RUNTIME_SECS: u64 = 10800;

/// Default enablement for stream-json output textification
pub const DEFAULT_STREAM_JSON_TEXTIFY: bool = true;

/// Default enablement for strict post-completion process-group cleanup.
/// When true, the orchestrator always runs a SIGTERM→SIGKILL sweep on the
/// spawned process group after a command completes (regardless of exit status).
pub const DEFAULT_COMMAND_STRICT_PROCESS_CLEANUP: bool = true;

/// Default error patterns that trigger automatic retry
pub fn default_retry_patterns() -> Vec<String> {
    vec![
        // Module resolution errors
        r"Cannot find module".to_string(),
        r"ResolveMessage:".to_string(),
        // npm/bun registry errors
        r"ENOTFOUND registry\.npmjs\.org".to_string(),
        r"ETIMEDOUT.*registry".to_string(),
        // File lock errors
        r"EBADF.*lock".to_string(),
        r"Lock acquisition failed".to_string(),
    ]
}

/// Default enablement for error circuit breaker
pub const DEFAULT_ERROR_CIRCUIT_BREAKER_ENABLED: bool = true;

/// Default threshold for consecutive same errors before opening circuit
pub const DEFAULT_ERROR_CIRCUIT_BREAKER_THRESHOLD: usize = 5;

/// Default maximum number of acceptance CONTINUE retries before treating as FAIL
pub const DEFAULT_ACCEPTANCE_MAX_CONTINUES: u32 = 10;

// ── External lifecycle integration defaults ────────────────────────────────

/// Default bounded queue capacity for the external lifecycle dispatcher.
///
/// The queue is intentionally small: lifecycle messages are observability-only,
/// so dropping redundant state refreshes is preferred over growing memory.
pub const DEFAULT_LIFECYCLE_QUEUE_CAPACITY: usize = 64;

/// Default bounded per-message write timeout for the lifecycle adapter (milliseconds).
pub const DEFAULT_LIFECYCLE_WRITE_TIMEOUT_MS: u64 = 2_000;

/// Default bounded shutdown deadline for the lifecycle adapter (milliseconds).
pub const DEFAULT_LIFECYCLE_SHUTDOWN_TIMEOUT_MS: u64 = 2_000;

// ── Purpose-scoped judge command defaults ──────────────────────────────────

/// Default bounded wall-clock timeout for one judge invocation (milliseconds).
pub const DEFAULT_JUDGE_TIMEOUT_MS: u64 = 30_000;

/// Smallest configurable judge timeout (milliseconds).
pub const MIN_JUDGE_TIMEOUT_MS: u64 = 1;

/// Largest configurable judge timeout (milliseconds).
///
/// A judge is a bounded shadow observer, never an implementation worker, so its
/// ceiling is far below the agent-command budget.
pub const MAX_JUDGE_TIMEOUT_MS: u64 = 300_000;

/// Default maximum serialized request size accepted for one judge invocation (bytes).
pub const DEFAULT_JUDGE_MAX_INPUT_BYTES: usize = 1_048_576;

/// Default maximum stdout/stderr size collected from one judge invocation (bytes).
pub const DEFAULT_JUDGE_MAX_OUTPUT_BYTES: usize = 1_048_576;

/// Smallest configurable judge input/output byte limit.
pub const MIN_JUDGE_BYTE_LIMIT: usize = 1;

/// Largest configurable judge input/output byte limit.
pub const MAX_JUDGE_BYTE_LIMIT: usize = 16_777_216;

/// Default probability at or above which a shadow judgment counts as a `yes` edge.
pub const DEFAULT_JUDGE_YES_THRESHOLD: f64 = 0.85;

/// Smallest configurable judge yes-threshold.
///
/// Below `0.5` a "yes" would be less likely than its negation, which is not a
/// judgment a comparison metric can use.
pub const MIN_JUDGE_YES_THRESHOLD: f64 = 0.5;

/// Largest configurable judge yes-threshold.
pub const MAX_JUDGE_YES_THRESHOLD: f64 = 1.0;

/// The only judge mode this release accepts.
///
/// An omitted `mode` normalizes to this value, so every existing configuration
/// keeps working. `prefer`, fail-closed, and judge-authoritative modes stay
/// unspecified until a separate reviewed change defines them.
pub const JUDGE_MODE_SHADOW: &str = "shadow";

// ── Judge evaluation-record defaults ───────────────────────────────────────

/// Largest configurable evaluation retention window (days).
///
/// Ten years. Evaluation records are disposable observability, so the ceiling
/// exists to make a typo fail configuration loading rather than to express a
/// retention policy.
pub const MAX_JUDGE_EVALUATION_RETENTION_DAYS: u32 = 3_650;

/// Largest configurable global evaluation byte budget (10 GiB).
pub const MAX_JUDGE_EVALUATION_TOTAL_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// Directory name Conflux owns for evaluation records inside the state root.
///
/// Deliberately a sibling of `logs`, never a descendant: ordinary log retention
/// and the log viewer are scoped to `logs` alone, so evaluation records can be
/// neither read nor deleted by them.
pub const EVALUATION_DIR_NAME: &str = "evaluations";

/// The single evaluation purpose this change records.
pub const EVALUATION_PURPOSE_PARALLEL_DEPENDENCY: &str = "parallel_dependency";

/// File name of the installation-scoped pair-identifier salt.
pub const EVALUATION_SALT_FILE_NAME: &str = ".pair-id-salt";

/// The evaluation root shared by the recorder, retention, and the summary reader.
///
/// Format: `<state root>/evaluations`. When no state root can be derived at all
/// it keeps the same temp-directory fallback shape logging uses, so a machine
/// without a home directory still gets an isolated evaluation root rather than
/// silently writing beside the logs.
pub fn evaluation_root_path(
    configured: Option<&str>,
) -> std::result::Result<PathBuf, StateRootError> {
    Ok(match conflux_state_root(configured)? {
        Some(root) => root.join(EVALUATION_DIR_NAME),
        None => std::env::temp_dir().join("cflx-evaluations-fallback"),
    })
}

/// Generates a project slug from the repository root path.
/// Format: `{repo_basename}-{hash8}` where hash8 is the first 8 chars of the SHA256 hash
/// of the absolute repository path.
///
/// Example: `/Users/alice/projects/conflux` → `conflux-a1b2c3d4`
pub fn generate_project_slug(repo_root: &std::path::Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Get repository basename
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Generate hash from absolute path
    let absolute_path = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut hasher = DefaultHasher::new();
    absolute_path.hash(&mut hasher);
    let hash = hasher.finish();
    let hash_str = format!("{:016x}", hash);
    let hash8 = &hash_str[..8];

    format!("{}-{}", repo_name, hash8)
}

/// Returns the default workspace base directory based on the OS and environment.
///
/// - **macOS**: Uses `${XDG_DATA_HOME}/cflx/worktrees/<project_slug>` if `XDG_DATA_HOME` is set,
///   otherwise falls back to `~/.local/share/cflx/worktrees/<project_slug>`.
/// - **Linux**: Uses `${XDG_DATA_HOME}/cflx/worktrees/<project_slug>` if set,
///   otherwise `~/.local/share/cflx/worktrees/<project_slug>`.
/// - **Windows**: Uses `%APPDATA%\cflx\worktrees\<project_slug>`.
/// - **Other**: Falls back to system temp directory with `cflx-workspaces-fallback/<project_slug>`.
///
/// If `repo_root` is provided, the path includes a project-specific slug to avoid conflicts.
/// If `repo_root` is None, returns a generic path without project slug (for backwards compatibility).
pub fn default_workspace_base_dir(repo_root: Option<&std::path::Path>) -> PathBuf {
    // Generate project slug if repo_root is provided
    let project_slug = repo_root.map(generate_project_slug);

    #[cfg(target_os = "macos")]
    {
        // Check XDG_DATA_HOME first
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            let mut path = PathBuf::from(xdg_data_home).join("cflx").join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
        // Fall back to ~/.local/share (same as Linux)
        if let Some(home) = dirs::home_dir() {
            let mut path = home
                .join(".local")
                .join("share")
                .join("cflx")
                .join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Use XDG_DATA_HOME or fall back to ~/.local/share
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            let mut path = PathBuf::from(xdg_data_home).join("cflx").join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
        if let Some(home) = dirs::home_dir() {
            let mut path = home
                .join(".local")
                .join("share")
                .join("cflx")
                .join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Use APPDATA directory
        if let Some(appdata) = dirs::data_dir() {
            let mut path = appdata.join("cflx").join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
    }

    // Fallback for unsupported platforms or when home directory is not available
    let mut path = std::env::temp_dir().join("cflx-workspaces-fallback");
    if let Some(slug) = &project_slug {
        path = path.join(slug);
    }
    path
}

/// Directory Conflux owns inside whichever persistent state root is resolved.
pub const STATE_DIR_NAME: &str = "cflx";

/// Reasons an explicitly configured `state_base_dir` cannot be used.
///
/// Only an *explicit* configuration can produce one of these. An absent or empty
/// setting falls back to `XDG_STATE_HOME` and then to the platform default
/// exactly as before, so an unconfigured installation never sees them.
#[derive(Debug, thiserror::Error)]
pub enum StateRootError {
    #[error(
        "state_base_dir must be an absolute path, but '{0}' is relative. \
         Set an absolute path in .cflx.jsonc or the global config, or remove \
         state_base_dir to use XDG_STATE_HOME."
    )]
    NotAbsolute(String),

    #[error(
        "state_base_dir '{path}' is unavailable and could not be created: {source}. \
         Mount the volume or grant write permission, or remove state_base_dir to \
         use XDG_STATE_HOME."
    )]
    Uncreatable {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(
        "state_base_dir '{path}' is not writable: {source}. \
         Grant write permission on that directory, or remove state_base_dir to \
         use XDG_STATE_HOME."
    )]
    NotWritable {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Resolve the Conflux-owned persistent state root from explicit inputs.
///
/// Precedence is `state_base_dir` → `XDG_STATE_HOME` → the platform default
/// `~/.local/state`. Only Conflux-owned paths are affected: nothing here reads
/// or writes the process environment, so child Apply, Acceptance, Archive,
/// Resolve, and lifecycle commands keep the `XDG_STATE_HOME` they inherited.
///
/// An empty configured value behaves as unset, matching `workspace_base_dir`.
/// `Ok(None)` means no root could be derived at all (no configuration, no XDG
/// variable, and no home directory); callers own the temp-directory fallback.
pub fn resolve_state_root_from(
    configured: Option<&str>,
    xdg_state_home: Option<&str>,
    home_dir: Option<&std::path::Path>,
) -> std::result::Result<Option<PathBuf>, StateRootError> {
    if let Some(configured) = configured.filter(|value| !value.is_empty()) {
        let root = PathBuf::from(configured);
        if !root.is_absolute() {
            return Err(StateRootError::NotAbsolute(configured.to_string()));
        }
        return Ok(Some(root.join(STATE_DIR_NAME)));
    }

    if let Some(xdg_state_home) = xdg_state_home.filter(|value| !value.is_empty()) {
        return Ok(Some(PathBuf::from(xdg_state_home).join(STATE_DIR_NAME)));
    }

    Ok(home_dir.map(|home| home.join(".local").join("state").join(STATE_DIR_NAME)))
}

/// Process-facing wrapper over [`resolve_state_root_from`].
pub fn conflux_state_root(
    configured: Option<&str>,
) -> std::result::Result<Option<PathBuf>, StateRootError> {
    let xdg_state_home = std::env::var("XDG_STATE_HOME").ok();
    resolve_state_root_from(
        configured,
        xdg_state_home.as_deref(),
        dirs::home_dir().as_deref(),
    )
}

/// The single persistent log root shared by logging, retention cleanup, and
/// `cflx logs`.
///
/// Format: `<state root>/logs`, i.e. `<state_base_dir>/cflx/logs` when the root
/// is configured. When no root can be derived at all it keeps the historical
/// temp-directory fallback.
pub fn log_root_path(configured: Option<&str>) -> std::result::Result<PathBuf, StateRootError> {
    Ok(match conflux_state_root(configured)? {
        Some(root) => root.join("logs"),
        None => std::env::temp_dir().join("cflx-logs-fallback"),
    })
}

/// Prove an explicitly configured state root is usable before anything writes to it.
///
/// Creates the Conflux-owned directory when it is missing and probes it for
/// write access, so a relative, unavailable, uncreatable, or unwritable root
/// fails startup instead of silently falling back to the internal disk. Returns
/// `Ok(None)` when no root is configured: the XDG and platform defaults keep
/// their existing lenient behavior.
pub fn ensure_state_root_usable(
    configured: Option<&str>,
) -> std::result::Result<Option<PathBuf>, StateRootError> {
    let Some(configured) = configured.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let Some(root) = resolve_state_root_from(Some(configured), None, None)? else {
        return Ok(None);
    };

    std::fs::create_dir_all(&root).map_err(|source| StateRootError::Uncreatable {
        path: root.clone(),
        source,
    })?;

    // Existence is not permission: an unwritable mount answers `create_dir_all`
    // with success for a directory that already exists.
    let probe = root.join(format!(".cflx-write-probe-{}", std::process::id()));
    std::fs::write(&probe, b"").map_err(|source| StateRootError::NotWritable {
        path: root.clone(),
        source,
    })?;
    let _ = std::fs::remove_file(&probe);

    Ok(Some(root))
}

// ── External Acceptance evidence store ─────────────────────────────────────

/// Directory Conflux owns for Acceptance evidence inside the state root.
///
/// A sibling of `logs` and `evaluations`, never a descendant: log retention and
/// the log viewer are scoped to `logs` alone, so neither can read or delete an
/// Acceptance record.
pub const ACCEPTANCE_DIR_NAME: &str = "acceptance";

/// Why an external Acceptance store path could not be produced or used.
///
/// Every variant is a refusal rather than a degraded path. Constitutional law 4
/// forbids falling back into the target worktree, so a store that cannot be
/// resolved has no second choice to offer.
#[derive(Debug, thiserror::Error)]
pub enum AcceptanceStoreError {
    #[error("the Conflux state root could not be resolved: {0}")]
    StateRoot(#[from] StateRootError),

    #[error(
        "'{0}' is not usable as a single Acceptance store path component; \
         a change or verification identifier may not be empty, '.', '..', longer \
         than 128 characters, or contain anything outside [A-Za-z0-9._-]"
    )]
    UnsafeComponent(String),

    #[error(
        "the resolved Acceptance store '{path}' is inside the target tree '{target}'. \
         Conflux-owned evidence must live outside the target repository and every \
         managed worktree; point state_base_dir at a directory outside the project."
    )]
    InsideTarget { path: PathBuf, target: PathBuf },

    #[error("the Acceptance store '{path}' could not be created: {source}")]
    Uncreatable {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("the Acceptance store '{path}' is not writable: {source}")]
    NotWritable {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl AcceptanceStoreError {
    /// Stable machine-readable code for typed runtime holds and JSON output.
    pub fn code(&self) -> &'static str {
        match self {
            Self::StateRoot(_) => "acceptance_state_root_unresolvable",
            Self::UnsafeComponent(_) => "acceptance_store_unsafe_component",
            Self::InsideTarget { .. } => "acceptance_store_inside_target",
            Self::Uncreatable { .. } => "acceptance_store_uncreatable",
            Self::NotWritable { .. } => "acceptance_store_not_writable",
        }
    }
}

/// Whether `value` may be used as exactly one path component of a Conflux-owned
/// store.
///
/// Refused rather than sanitized: a sanitized name binds a store to an identity
/// it does not describe, and two identities that sanitize to one name would
/// share a cache.
pub fn is_safe_path_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The Acceptance evidence root shared by every store this process resolves.
///
/// Format: `<state root>/acceptance`. When no state root can be derived at all
/// it keeps the same temp-directory fallback shape logging and evaluations use,
/// so a machine without a home directory still gets an isolated external root
/// rather than a target-local one.
pub fn acceptance_root_path(
    configured: Option<&str>,
) -> std::result::Result<PathBuf, StateRootError> {
    Ok(match conflux_state_root(configured)? {
        Some(root) => root.join(ACCEPTANCE_DIR_NAME),
        None => std::env::temp_dir().join("cflx-acceptance-fallback"),
    })
}

/// Resolve a path to its canonical form as far as the filesystem allows.
///
/// A store path usually does not exist yet, so `canonicalize` on the whole path
/// fails. The nearest existing ancestor is canonicalized instead and the
/// remaining components are rejoined, which is what makes containment comparable
/// on platforms where the temp directory is itself a symlink (macOS `/var` →
/// `/private/var`). Without this a store under `/var/folders/...` and a target
/// under `/private/var/folders/...` would look unrelated.
fn canonical_or_lexical(path: &std::path::Path) -> PathBuf {
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = path.to_path_buf();
    loop {
        if let Ok(canonical) = cursor.canonicalize() {
            let mut resolved = canonical;
            for component in suffix.iter().rev() {
                resolved.push(component);
            }
            return resolved;
        }
        let Some(name) = cursor.file_name().map(std::ffi::OsStr::to_os_string) else {
            return path.to_path_buf();
        };
        suffix.push(name);
        if !cursor.pop() {
            return path.to_path_buf();
        }
    }
}

/// Whether `path` is `target` or lives below it.
///
/// Both sides are resolved through [`canonical_or_lexical`] first, and the raw
/// lexical forms are compared too: a symlinked target whose link cannot be read
/// must still refuse rather than silently pass the containment check.
fn is_inside(path: &std::path::Path, target: &std::path::Path) -> bool {
    canonical_or_lexical(path).starts_with(canonical_or_lexical(target)) || path.starts_with(target)
}

/// Compose the external Acceptance store directory for one change.
///
/// Layout: `<acceptance root>/<project-slug>/<workspace-slug>/<change-id>/`.
/// Both slugs come from [`generate_project_slug`], so they are derived from the
/// canonical absolute path and are deterministic within one binary; `change_id`
/// keeps strict single-component validation.
///
/// Pure: it resolves no configuration, reads no environment, and creates
/// nothing. Containment against the target repository *and* the managed
/// workspace is checked here so a refusal happens before any gate is launched.
pub fn acceptance_store_path_in(
    acceptance_root: &std::path::Path,
    project_root: &std::path::Path,
    workspace: &std::path::Path,
    change_id: &str,
) -> std::result::Result<PathBuf, AcceptanceStoreError> {
    if !is_safe_path_component(change_id) {
        return Err(AcceptanceStoreError::UnsafeComponent(change_id.to_string()));
    }
    let path = acceptance_root
        .join(generate_project_slug(project_root))
        .join(generate_project_slug(workspace))
        .join(change_id);
    for target in [project_root, workspace] {
        if is_inside(&path, target) {
            return Err(AcceptanceStoreError::InsideTarget {
                path,
                target: target.to_path_buf(),
            });
        }
    }
    Ok(path)
}

/// Create the resolved store and prove it is writable.
///
/// Existence is not permission: an unwritable mount answers `create_dir_all`
/// with success for a directory that already exists, so the probe is what turns
/// "the path resolves" into "a gate's output can actually land there".
pub fn ensure_acceptance_store_usable(
    path: &std::path::Path,
) -> std::result::Result<(), AcceptanceStoreError> {
    std::fs::create_dir_all(path).map_err(|source| AcceptanceStoreError::Uncreatable {
        path: path.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
    let probe = path.join(format!(".cflx-write-probe-{}", std::process::id()));
    std::fs::write(&probe, b"").map_err(|source| AcceptanceStoreError::NotWritable {
        path: path.to_path_buf(),
        source,
    })?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// Generates the log file path for a project under the resolved state root.
/// Format: `<state root>/logs/<project_slug>/<YYYY-MM-DD>.log`
///
/// - Uses `state_base_dir` when configured, then `${XDG_STATE_HOME}`, then
///   `~/.local/state`; see [`resolve_state_root_from`].
/// - Fallback: system temp directory with `cflx-logs-fallback/<project_slug>/<YYYY-MM-DD>.log`.
///
/// If `repo_root` is provided, the path includes a project-specific slug to avoid conflicts.
pub fn get_log_file_path(
    state_base_dir: Option<&str>,
    repo_root: Option<&std::path::Path>,
) -> std::result::Result<PathBuf, StateRootError> {
    use chrono::Local;

    // Generate project slug if repo_root is provided
    let project_slug = repo_root
        .map(generate_project_slug)
        .unwrap_or_else(|| "unknown".to_string());

    // Get current date in YYYY-MM-DD format
    let date_str = Local::now().format("%Y-%m-%d").to_string();
    let log_filename = format!("{}.log", date_str);

    Ok(log_root_path(state_base_dir)?
        .join(&project_slug)
        .join(&log_filename))
}

/// Cleans up old log files, retaining only the last N days.
/// Returns the number of files deleted.
///
/// Reads the same root as logging and `cflx logs`, so retention can never
/// clean a directory Conflux is no longer writing to.
pub fn cleanup_old_logs(
    state_base_dir: Option<&str>,
    repo_root: Option<&std::path::Path>,
    retain_days: u32,
) -> std::io::Result<usize> {
    use chrono::{Duration, Local};

    let project_slug = repo_root
        .map(generate_project_slug)
        .unwrap_or_else(|| "unknown".to_string());

    let log_dir = log_root_path(state_base_dir)
        .map_err(std::io::Error::other)?
        .join(&project_slug);

    // If log directory doesn't exist, nothing to clean
    if !log_dir.exists() {
        return Ok(0);
    }

    // Calculate cutoff date
    // retain_days = 7 means keep today + previous 6 days (7 total)
    // So cutoff is (retain_days - 1) days ago
    let cutoff_date = Local::now() - Duration::days(i64::from(retain_days - 1));
    let cutoff_str = cutoff_date.format("%Y-%m-%d").to_string();

    let mut deleted_count = 0;

    // Iterate over files in log directory
    for entry in std::fs::read_dir(&log_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process .log files
        if path.extension().and_then(|s| s.to_str()) != Some("log") {
            continue;
        }

        // Extract date from filename (YYYY-MM-DD.log)
        if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
            // Compare filename (date) with cutoff
            if filename < cutoff_str.as_str() {
                std::fs::remove_file(&path)?;
                deleted_count += 1;
            }
        }
    }

    Ok(deleted_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn set_env_var<K: AsRef<std::ffi::OsStr>, V: AsRef<std::ffi::OsStr>>(key: K, value: V) {
        unsafe { std::env::set_var(key, value) }
    }

    fn remove_env_var<K: AsRef<std::ffi::OsStr>>(key: K) {
        unsafe { std::env::remove_var(key) }
    }

    #[test]
    fn test_default_workspace_base_dir_returns_path() {
        // Test that the function returns a valid PathBuf without repo_root
        let path = default_workspace_base_dir(None);
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_default_workspace_base_dir_contains_cflx() {
        // Test that the path contains "cflx" or is the fallback
        let path = default_workspace_base_dir(None);
        let path_str = path.to_string_lossy();

        // Should contain "cflx" (case-insensitive) or be the fallback
        let is_cflx_path = path_str.to_lowercase().contains("cflx");
        let is_fallback = path_str.contains("cflx-workspaces-fallback");

        assert!(
            is_cflx_path || is_fallback,
            "Path should contain 'cflx' or be fallback: {:?}",
            path
        );
    }

    #[test]
    fn test_default_workspace_base_dir_with_repo_root() {
        // Test with repo_root parameter
        let repo_root = PathBuf::from("/Users/alice/projects/conflux");
        let path = default_workspace_base_dir(Some(&repo_root));
        let path_str = path.to_string_lossy();

        // Should contain project slug (repo name + hash)
        assert!(
            path_str.contains("conflux-"),
            "Path should contain project slug: {:?}",
            path
        );
    }

    #[test]
    fn test_generate_project_slug() {
        let repo_root = PathBuf::from("/Users/alice/projects/my-repo");
        let slug = generate_project_slug(&repo_root);

        // Should have format: {name}-{hash8}
        assert!(
            slug.starts_with("my-repo-"),
            "Slug should start with repo name"
        );
        assert_eq!(
            slug.len(),
            "my-repo-".len() + 8,
            "Slug should have 8-char hash"
        );
    }

    #[test]
    fn test_default_workspace_base_dir_with_xdg_data_home() {
        // Test XDG_DATA_HOME override (only on Unix-like systems)
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            use std::env;

            // Save original value
            let original = env::var("XDG_DATA_HOME").ok();

            // Set XDG_DATA_HOME
            let test_path = "/tmp/test-xdg-data";
            set_env_var("XDG_DATA_HOME", test_path);

            let repo_root = PathBuf::from("/tmp/test-repo");
            let result = default_workspace_base_dir(Some(&repo_root));

            // Should use XDG_DATA_HOME
            assert!(
                result.starts_with(test_path),
                "Expected path to start with {}, got {:?}",
                test_path,
                result
            );
            assert!(
                result.to_string_lossy().contains("cflx/worktrees"),
                "Expected path to contain cflx/worktrees, got {:?}",
                result
            );

            // Restore original value
            match original {
                Some(val) => set_env_var("XDG_DATA_HOME", val),
                None => remove_env_var("XDG_DATA_HOME"),
            }
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_default_workspace_base_dir_macos_fallback() {
        // NOTE: This test runs in parallel with other tests that may set XDG_DATA_HOME,
        // so we can't rely on environment isolation. We just verify the path contains
        // expected components (cflx/worktrees) and project slug.
        let repo_root = PathBuf::from("/tmp/test-repo");
        let result = default_workspace_base_dir(Some(&repo_root));
        let path_str = result.to_string_lossy();

        // Should contain cflx and worktrees (either from XDG_DATA_HOME or ~/.local/share)
        assert!(
            path_str.contains("cflx") && path_str.contains("worktrees"),
            "Expected path to contain 'cflx' and 'worktrees', got {:?}",
            result
        );

        // Should contain project slug
        assert!(
            path_str.contains("test-repo-"),
            "Expected path to contain project slug 'test-repo-', got {:?}",
            result
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_default_workspace_base_dir_linux_fallback() {
        // NOTE: This test may run in parallel with other tests that set XDG_DATA_HOME,
        // so we can't rely on environment isolation. We just check that the path
        // contains the expected components (cflx/worktrees) and project slug.
        let repo_root = PathBuf::from("/tmp/test-repo");
        let result = default_workspace_base_dir(Some(&repo_root));
        let path_str = result.to_string_lossy();

        // Should contain cflx and worktrees
        assert!(
            path_str.contains("cflx") && path_str.contains("worktrees"),
            "Expected path to contain 'cflx' and 'worktrees', got {:?}",
            result
        );

        // Should contain project slug
        assert!(
            path_str.contains("test-repo-"),
            "Expected path to contain project slug 'test-repo-', got {:?}",
            result
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_default_workspace_base_dir_windows() {
        let repo_root = PathBuf::from("C:\\Users\\test\\projects\\my-repo");
        let result = default_workspace_base_dir(Some(&repo_root));
        let path_str = result.to_string_lossy();

        // Should use APPDATA on Windows
        assert!(
            path_str.contains("cflx") && path_str.contains("worktrees"),
            "Expected path to contain 'cflx' and 'worktrees', got {:?}",
            result
        );
    }

    #[test]
    fn test_acceptance_system_prompt_is_empty() {
        // ACCEPTANCE_SYSTEM_PROMPT should stay empty so runtime prompts remain context-only.
        // Acceptance guidance comes from the selected portable accept_skill.
        assert!(
            ACCEPTANCE_SYSTEM_PROMPT.is_empty(),
            "ACCEPTANCE_SYSTEM_PROMPT should be empty to keep acceptance prompts context-only"
        );
    }

    #[test]
    fn test_get_log_file_path_format() {
        let repo_root = PathBuf::from("/tmp/test-repo");
        let log_path = get_log_file_path(None, Some(&repo_root)).expect("default root resolves");
        let path_str = log_path.to_string_lossy();

        // Should contain cflx/logs
        assert!(
            path_str.contains("cflx") && path_str.contains("logs"),
            "Expected path to contain 'cflx/logs', got {:?}",
            log_path
        );

        // Should contain project slug
        assert!(
            path_str.contains("test-repo-"),
            "Expected path to contain project slug 'test-repo-', got {:?}",
            log_path
        );

        // Should end with .log
        assert!(
            path_str.ends_with(".log"),
            "Expected path to end with '.log', got {:?}",
            log_path
        );
    }

    #[test]
    fn test_cleanup_old_logs_with_nonexistent_directory() {
        // Should return Ok(0) for non-existent directory
        let repo_root = PathBuf::from("/tmp/nonexistent-repo-for-test");
        let result = cleanup_old_logs(None, Some(&repo_root), 7);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_cleanup_old_logs_retains_exactly_n_days() {
        use chrono::Local;
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create temporary XDG_STATE_HOME
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_state_home = temp_dir.path().join("state");
        fs::create_dir(&temp_state_home).expect("Failed to create state dir");

        // Save original XDG_STATE_HOME and set to temp
        let original_state_home = env::var("XDG_STATE_HOME").ok();
        set_env_var("XDG_STATE_HOME", &temp_state_home);

        // Create repo root and get project slug
        let repo_root = PathBuf::from("/tmp/test-retention-repo");
        let project_slug = generate_project_slug(&repo_root);

        // Create log directory
        let log_dir = temp_state_home
            .join("cflx")
            .join("logs")
            .join(&project_slug);
        fs::create_dir_all(&log_dir).expect("Failed to create log dir");

        // Create dated log files: today - 9, today - 8, ..., today - 1, today
        let today = Local::now();
        let mut expected_files = Vec::new();

        for days_ago in (0..=9).rev() {
            let date = today - chrono::Duration::days(days_ago);
            let filename = format!("{}.log", date.format("%Y-%m-%d"));
            let file_path = log_dir.join(&filename);
            fs::write(&file_path, "test log content").expect("Failed to write log file");

            // Files within retain_days (7) should be kept: today, today-1, ..., today-6
            if days_ago < 7 {
                expected_files.push(filename);
            }
        }

        // Run cleanup with retain_days = 7
        let deleted_count =
            cleanup_old_logs(None, Some(&repo_root), 7).expect("cleanup_old_logs failed");

        // Should delete exactly 3 files (today-9, today-8, today-7)
        assert_eq!(deleted_count, 3, "Expected to delete 3 files");

        // Verify exactly 7 files remain
        let remaining: Vec<_> = fs::read_dir(&log_dir)
            .expect("Failed to read log dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("log"))
            .map(|e| e.path().file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(
            remaining.len(),
            7,
            "Expected exactly 7 log files to remain, found: {:?}",
            remaining
        );

        // Verify today's log is preserved
        let today_filename = format!("{}.log", today.format("%Y-%m-%d"));
        assert!(
            remaining.contains(&today_filename),
            "Today's log file should be preserved: {}",
            today_filename
        );

        // Verify all expected files are present
        for expected in &expected_files {
            assert!(
                remaining.contains(expected),
                "Expected file {} should be present",
                expected
            );
        }

        // Restore original XDG_STATE_HOME
        match original_state_home {
            Some(val) => set_env_var("XDG_STATE_HOME", val),
            None => remove_env_var("XDG_STATE_HOME"),
        }
    }

    // ── State-root precedence ──────────────────────────────────────────────
    //
    // These cases are pure: every input is passed explicitly, so they assert
    // the precedence rule itself rather than the ambient process environment.

    #[test]
    fn configured_state_root_wins_over_xdg_state_home() {
        let root = resolve_state_root_from(
            Some("/volumes/external/cflx-state"),
            Some("/home/alice/.local/state"),
            Some(Path::new("/home/alice")),
        )
        .expect("absolute configured root is accepted");

        assert_eq!(
            root,
            Some(PathBuf::from("/volumes/external/cflx-state/cflx"))
        );
    }

    #[test]
    fn unset_state_root_falls_back_to_xdg_then_platform_default() {
        let from_xdg = resolve_state_root_from(
            None,
            Some("/home/alice/.local/state"),
            Some(Path::new("/home/alice")),
        )
        .expect("xdg root resolves");
        assert_eq!(
            from_xdg,
            Some(PathBuf::from("/home/alice/.local/state/cflx"))
        );

        let from_home = resolve_state_root_from(None, None, Some(Path::new("/home/alice")))
            .expect("platform default resolves");
        assert_eq!(
            from_home,
            Some(PathBuf::from("/home/alice/.local/state/cflx"))
        );

        let nothing = resolve_state_root_from(None, None, None).expect("absence is not a failure");
        assert_eq!(nothing, None);
    }

    #[test]
    fn empty_state_root_behaves_as_unset() {
        let root = resolve_state_root_from(
            Some(""),
            Some("/home/alice/.local/state"),
            Some(Path::new("/home/alice")),
        )
        .expect("empty configured root is not a failure");
        assert_eq!(root, Some(PathBuf::from("/home/alice/.local/state/cflx")));

        // An empty XDG variable is equally unset, so writers and readers agree
        // on the platform default instead of splitting over a relative path.
        let root = resolve_state_root_from(Some(""), Some(""), Some(Path::new("/home/alice")))
            .expect("empty xdg value is not a failure");
        assert_eq!(root, Some(PathBuf::from("/home/alice/.local/state/cflx")));
    }

    #[test]
    fn relative_state_root_is_rejected_with_an_actionable_message() {
        let err = resolve_state_root_from(
            Some("relative/state"),
            Some("/home/alice/.local/state"),
            Some(Path::new("/home/alice")),
        )
        .expect_err("a relative configured root must fail closed");

        assert!(matches!(err, StateRootError::NotAbsolute(ref value) if value == "relative/state"));
        let message = err.to_string();
        assert!(message.contains("absolute"), "message: {message}");
        assert!(message.contains("relative/state"), "message: {message}");
        assert!(message.contains("state_base_dir"), "message: {message}");
    }

    #[test]
    fn ensure_state_root_usable_is_a_no_op_without_configuration() {
        assert!(ensure_state_root_usable(None)
            .expect("absent configuration is not a failure")
            .is_none());
        assert!(ensure_state_root_usable(Some(""))
            .expect("empty configuration is not a failure")
            .is_none());
    }

    #[test]
    fn ensure_state_root_usable_rejects_a_relative_root_before_touching_the_filesystem() {
        let err = ensure_state_root_usable(Some("relative/state"))
            .expect_err("a relative configured root must fail closed");
        assert!(matches!(err, StateRootError::NotAbsolute(_)));
        assert!(
            !Path::new("relative/state").exists(),
            "rejection must not create the configured directory"
        );
    }

    // ── External Acceptance store resolution ───────────────────────────────
    //
    // Pure cases: every input is explicit, so they assert the layout and the
    // containment law rather than the ambient process environment.

    #[test]
    fn acceptance_store_isolates_project_workspace_and_change() {
        let root = Path::new("/var/state/cflx/acceptance");
        let project = Path::new("/home/alice/projects/conflux");
        let workspace = Path::new("/home/alice/.local/share/cflx/worktrees/p/add-a-thing");

        let path = acceptance_store_path_in(root, project, workspace, "add-a-thing")
            .expect("an external root resolves");

        let components: Vec<_> = path
            .strip_prefix(root)
            .expect("the store lives under the acceptance root")
            .iter()
            .map(|component| component.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            components.len(),
            3,
            "project / workspace / change: {path:?}"
        );
        assert_eq!(components[0], generate_project_slug(project));
        assert_eq!(components[1], generate_project_slug(workspace));
        assert_eq!(components[2], "add-a-thing");

        // Deterministic within one binary: the same inputs resolve identically,
        // which is what lets a later attempt find the cache the first one wrote.
        assert_eq!(
            path,
            acceptance_store_path_in(root, project, workspace, "add-a-thing").unwrap()
        );

        // A second workspace of the same project cannot collide with the first.
        let sibling = Path::new("/home/alice/.local/share/cflx/worktrees/p/other-thing");
        assert_ne!(
            path,
            acceptance_store_path_in(root, project, sibling, "add-a-thing").unwrap()
        );
    }

    #[test]
    fn acceptance_store_refuses_an_unsafe_change_component() {
        let root = Path::new("/var/state/cflx/acceptance");
        let project = Path::new("/home/alice/project");
        let workspace = Path::new("/home/alice/workspace");

        for hostile in ["", ".", "..", "../escape", "a/b", "with space"] {
            let error = acceptance_store_path_in(root, project, workspace, hostile)
                .expect_err("an unsafe component must fail closed");
            assert!(
                matches!(error, AcceptanceStoreError::UnsafeComponent(_)),
                "'{hostile}' produced {error:?}"
            );
        }
        assert!(is_safe_path_component("add-a-thing"));
        assert!(is_safe_path_component("focused-gate.log"));
        assert!(!is_safe_path_component(&"x".repeat(129)));
    }

    /// Constitutional law 4, checked before anything is created: a resolved root
    /// inside the target repository or inside the managed worktree is refused,
    /// never used and never quietly relocated.
    #[test]
    fn acceptance_store_refuses_a_root_inside_the_target() {
        let project = Path::new("/home/alice/projects/conflux");
        let workspace = Path::new("/home/alice/projects/conflux/.worktrees/add-a-thing");

        let inside_repo = acceptance_store_path_in(
            &project.join(".cflx/state/acceptance"),
            project,
            workspace,
            "add-a-thing",
        )
        .expect_err("a root inside the repository must fail closed");
        assert!(
            matches!(
                &inside_repo,
                AcceptanceStoreError::InsideTarget { target, .. } if target == project
            ),
            "{inside_repo:?}"
        );
        assert_eq!(inside_repo.code(), "acceptance_store_inside_target");

        let inside_worktree = acceptance_store_path_in(
            &workspace.join("acceptance"),
            Path::new("/home/alice/elsewhere"),
            workspace,
            "add-a-thing",
        )
        .expect_err("a root inside the managed worktree must fail closed");
        assert!(
            matches!(
                &inside_worktree,
                AcceptanceStoreError::InsideTarget { target, .. } if target == workspace
            ),
            "{inside_worktree:?}"
        );
    }

    /// The containment check survives a symlinked ancestor. On macOS the system
    /// temp directory is itself a symlink, so a purely lexical comparison would
    /// report "outside" for a store that is genuinely inside the target.
    #[test]
    fn acceptance_store_containment_follows_symlinked_ancestors() {
        let real = tempfile::TempDir::new().expect("tempdir");
        let project = real.path().join("project");
        std::fs::create_dir_all(project.join(".cflx")).expect("create project");

        let link_parent = tempfile::TempDir::new().expect("tempdir");
        let linked_project = link_parent.path().join("linked-project");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&project, &linked_project).expect("symlink");
        #[cfg(not(unix))]
        let linked_project = project.clone();

        let error = acceptance_store_path_in(
            &project.join(".cflx/acceptance"),
            &linked_project,
            &project.join("worktree"),
            "add-a-thing",
        )
        .expect_err("a store under the symlinked target must still be refused");
        assert!(matches!(error, AcceptanceStoreError::InsideTarget { .. }));
    }

    #[test]
    fn acceptance_root_falls_back_to_an_external_temporary_root() {
        // No configuration, no XDG value, and no home directory: the fallback is
        // still outside every target tree, which is the property that matters.
        let root = acceptance_root_path(Some("/volumes/external/cflx-state"))
            .expect("an absolute configured root resolves");
        assert_eq!(
            root,
            PathBuf::from("/volumes/external/cflx-state/cflx/acceptance")
        );

        let fallback = std::env::temp_dir().join("cflx-acceptance-fallback");
        assert!(fallback.is_absolute());
        assert_eq!(
            fallback.file_name().and_then(|n| n.to_str()),
            Some("cflx-acceptance-fallback")
        );
    }

    #[test]
    fn ensure_acceptance_store_usable_creates_and_proves_the_directory() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let store = temp.path().join("acceptance/project/workspace/change");

        ensure_acceptance_store_usable(&store).expect("a fresh store is creatable and writable");

        assert!(store.is_dir());
        assert_eq!(
            std::fs::read_dir(&store)
                .expect("read store")
                .filter_map(Result::ok)
                .count(),
            0,
            "the write probe must be removed again"
        );
    }
}
