//! Repository-scoped orchestration lock.
//!
//! At most one local orchestration-owning `cflx` process may run per Git
//! repository. Repository identity is the canonical Git *common* directory, so
//! linked worktrees of one repository share a single exclusion domain while
//! unrelated repositories stay independent.
//!
//! Ownership authority is the OS advisory lock held on an open file descriptor
//! for the process lifetime; normal and abnormal exit both release it. The JSON
//! owner file stored next to the lock is observability-only: it may be missing,
//! partial, malformed, or left over from a dead process, and none of those
//! states can grant or deny ownership. Nothing in this module is a workflow
//! control input.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// Lock file held open (and OS-locked) for the owner's process lifetime.
pub const LOCK_FILE_NAME: &str = "cflx-orchestration.lock";

/// Diagnostic owner metadata, replaced atomically. Never an ownership authority.
pub const OWNER_FILE_NAME: &str = "cflx-orchestration-owner.json";

/// How the owning process was invoked. Reported in conflict diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationMode {
    Tui,
    Run,
}

impl InvocationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            InvocationMode::Tui => "tui",
            InvocationMode::Run => "run",
        }
    }
}

/// The CLI entrypoint being started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationKind {
    /// `cflx` with no subcommand (default TUI).
    DefaultTui,
    /// `cflx tui`.
    Tui,
    /// `cflx run`.
    Run,
    /// Any command that does not own local orchestration.
    Other,
}

/// Whether an invocation must take the repository orchestration lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockDecision {
    Acquire(InvocationMode),
    Bypass,
}

/// Decide whether an invocation owns local orchestration for this repository.
pub fn classify_invocation(kind: InvocationKind) -> LockDecision {
    match kind {
        InvocationKind::DefaultTui | InvocationKind::Tui => {
            LockDecision::Acquire(InvocationMode::Tui)
        }
        InvocationKind::Run => LockDecision::Acquire(InvocationMode::Run),
        InvocationKind::Other => LockDecision::Bypass,
    }
}

/// Best-effort diagnostic description of the current lock owner.
///
/// Every field is optional so a partially written, truncated, or
/// forward-versioned owner file still yields whatever can be read safely.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
}

impl OwnerMetadata {
    /// True when no field carries usable information.
    pub fn is_empty(&self) -> bool {
        self.pid.is_none()
            && self.started_at.is_none()
            && self.workspace.is_none()
            && self.mode.is_none()
            && self.api_url.is_none()
    }

    /// Drop fields that cannot describe a live owner (blank strings, pid 0).
    fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        }
        self.pid = self.pid.filter(|pid| *pid > 0);
        self.started_at = clean(self.started_at);
        self.workspace = clean(self.workspace);
        self.mode = clean(self.mode);
        self.api_url = clean(self.api_url);
        self
    }
}

/// Parse owner metadata, returning `None` when nothing usable can be read.
///
/// Malformed or empty content is never an error: the OS lock, not this file,
/// decides ownership.
pub fn parse_owner_metadata(raw: &str) -> Option<OwnerMetadata> {
    let parsed: OwnerMetadata = serde_json::from_str(raw).ok()?;
    let parsed = parsed.sanitized();
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

/// Render the operator-facing conflict diagnostic.
///
/// Unavailable fields are omitted rather than guessed; in particular no API URL
/// is claimed unless the owner published one after a successful bind.
pub fn conflict_message(common_dir: &Path, owner: Option<&OwnerMetadata>) -> String {
    let mut out = String::new();
    out.push_str("Error: another Conflux process is already orchestrating this repository.\n");
    out.push_str(&format!("  repository: {}\n", common_dir.display()));

    let mut described = false;
    if let Some(owner) = owner {
        if let Some(pid) = owner.pid {
            out.push_str(&format!("  owner pid:  {pid}\n"));
            described = true;
        }
        if let Some(mode) = &owner.mode {
            out.push_str(&format!("  mode:       {mode}\n"));
            described = true;
        }
        if let Some(started_at) = &owner.started_at {
            out.push_str(&format!("  started at: {started_at}\n"));
            described = true;
        }
        if let Some(workspace) = &owner.workspace {
            out.push_str(&format!("  workspace:  {workspace}\n"));
            described = true;
        }
        if let Some(api_url) = &owner.api_url {
            out.push_str(&format!("  api url:    {api_url}\n"));
            described = true;
        }
    }
    if !described {
        out.push_str("  owner details are unavailable (lock metadata missing or unreadable)\n");
    }

    out.push_str(
        "Stop the owning process, or use its reported endpoint, instead of starting another one.",
    );
    out
}

/// Lexically normalize `.` and `..` without touching the filesystem.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                // `/..` is `/`; a leading `..` has nothing to cancel.
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                _ => out.push(Component::ParentDir),
            },
            other => out.push(other),
        }
    }
    out
}

/// Extract the target of a `gitdir: <path>` pointer file.
pub fn parse_gitdir_pointer(contents: &str) -> Option<&str> {
    contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("gitdir:"))
        .map(str::trim)
        .filter(|target| !target.is_empty())
}

/// Resolve the Git directory for a `.git` entry.
///
/// `pointer_contents` is `None` when `.git` is a real directory and `Some` when
/// it is a linked-worktree pointer file.
pub fn resolve_git_dir(dot_git: &Path, pointer_contents: Option<&str>) -> Option<PathBuf> {
    let Some(contents) = pointer_contents else {
        return Some(normalize_path(dot_git));
    };
    let target = Path::new(parse_gitdir_pointer(contents)?);
    if target.is_absolute() {
        Some(normalize_path(target))
    } else {
        Some(normalize_path(&dot_git.parent()?.join(target)))
    }
}

/// Resolve the Git *common* directory from a Git directory and its optional
/// `commondir` file contents.
///
/// A linked worktree's Git directory contains `commondir` pointing back at the
/// shared directory, which is what makes worktrees of one repository collapse
/// onto a single lock.
pub fn resolve_common_dir(git_dir: &Path, commondir_contents: Option<&str>) -> PathBuf {
    match commondir_contents
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None => normalize_path(git_dir),
        Some(value) => {
            let target = Path::new(value);
            if target.is_absolute() {
                normalize_path(target)
            } else {
                normalize_path(&git_dir.join(target))
            }
        }
    }
}

/// Find the canonical Git common directory for `workspace`, if any.
///
/// Returns `None` when the path is not inside a Git repository; callers treat
/// that as "no repository identity to lock" rather than an error.
pub fn discover_common_dir(workspace: &Path) -> Option<PathBuf> {
    let start = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    for dir in start.ancestors() {
        let dot_git = dir.join(".git");
        let Ok(meta) = std::fs::metadata(&dot_git) else {
            continue;
        };
        let git_dir = if meta.is_dir() {
            resolve_git_dir(&dot_git, None)?
        } else {
            let contents = std::fs::read_to_string(&dot_git).ok()?;
            resolve_git_dir(&dot_git, Some(&contents))?
        };
        let commondir = std::fs::read_to_string(git_dir.join("commondir")).ok();
        let common_dir = resolve_common_dir(&git_dir, commondir.as_deref());
        return Some(std::fs::canonicalize(&common_dir).unwrap_or(common_dir));
    }
    None
}

/// The repository and, when readable, the owner a conflict was lost to.
#[derive(Debug)]
pub struct LockConflict {
    pub common_dir: PathBuf,
    pub owner: Option<OwnerMetadata>,
}

/// Why a repository lock could not be acquired.
#[derive(Debug)]
pub enum LockError {
    /// Another live process holds the OS lock. Boxed to keep this error small
    /// enough to travel in a `Result` without bloating the success path.
    Conflict(Box<LockConflict>),
    /// The lock file itself could not be opened or locked.
    Io { path: PathBuf, source: io::Error },
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::Conflict(conflict) => {
                write!(
                    f,
                    "{}",
                    conflict_message(&conflict.common_dir, conflict.owner.as_ref())
                )
            }
            LockError::Io { path, source } => {
                write!(
                    f,
                    "failed to use repository lock file '{}': {}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl std::error::Error for LockError {}

/// An owned repository orchestration lock.
///
/// The OS releases the lock when the descriptor closes, which happens on both
/// normal drop and abnormal process termination. No stale-file cleanup exists
/// because none is needed.
pub struct RepositoryLock {
    _file: File,
    common_dir: PathBuf,
    owner_path: PathBuf,
    metadata: Mutex<OwnerMetadata>,
}

impl RepositoryLock {
    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    /// Snapshot of the metadata this process last published.
    pub fn metadata_snapshot(&self) -> OwnerMetadata {
        self.metadata
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Record the API base URL after a listener has actually bound.
    ///
    /// Called only with a URL returned by a successful bind, so a conflicting
    /// invocation never sees an endpoint that was requested but not reachable.
    pub fn publish_api_url(&self, url: &str) {
        let Ok(mut guard) = self.metadata.lock() else {
            return;
        };
        if guard.api_url.as_deref() == Some(url) {
            return;
        }
        guard.api_url = Some(url.to_string());
        let snapshot = guard.clone();
        drop(guard);
        write_owner_metadata(&self.owner_path, &snapshot);
    }
}

/// Try to take the repository orchestration lock for `workspace`.
///
/// `Ok(None)` means the workspace is not in a Git repository, so there is no
/// repository identity to exclude on.
pub fn acquire(
    workspace: &Path,
    mode: InvocationMode,
) -> Result<Option<RepositoryLock>, LockError> {
    let Some(common_dir) = discover_common_dir(workspace) else {
        return Ok(None);
    };
    let lock_path = common_dir.join(LOCK_FILE_NAME);
    let owner_path = common_dir.join(OWNER_FILE_NAME);

    // `truncate(false)`: the file's contents are irrelevant, only the lock on
    // its descriptor matters, and truncating would race with a live owner.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| LockError::Io {
            path: lock_path.clone(),
            source,
        })?;

    match try_lock_exclusive(&file) {
        Ok(true) => {}
        Ok(false) => {
            return Err(LockError::Conflict(Box::new(LockConflict {
                common_dir,
                owner: read_owner_metadata(&owner_path),
            })))
        }
        Err(source) => {
            return Err(LockError::Io {
                path: lock_path,
                source,
            })
        }
    }

    let workspace_display = std::fs::canonicalize(workspace)
        .unwrap_or_else(|_| workspace.to_path_buf())
        .display()
        .to_string();
    let metadata = OwnerMetadata {
        pid: Some(std::process::id()),
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        workspace: Some(workspace_display),
        mode: Some(mode.as_str().to_string()),
        api_url: None,
    };
    write_owner_metadata(&owner_path, &metadata);

    Ok(Some(RepositoryLock {
        _file: file,
        common_dir,
        owner_path,
        metadata: Mutex::new(metadata),
    }))
}

/// Read owner metadata best-effort. Any failure yields `None`.
pub fn read_owner_metadata(owner_path: &Path) -> Option<OwnerMetadata> {
    let raw = std::fs::read_to_string(owner_path).ok()?;
    parse_owner_metadata(&raw)
}

/// Replace the owner file atomically so a competitor never reads partial JSON.
fn write_owner_metadata(owner_path: &Path, metadata: &OwnerMetadata) {
    let Ok(json) = serde_json::to_string_pretty(metadata) else {
        return;
    };
    let tmp_path =
        owner_path.with_file_name(format!("{}.{}.tmp", OWNER_FILE_NAME, std::process::id()));
    if std::fs::write(&tmp_path, json.as_bytes()).is_err() {
        let _ = std::fs::remove_file(&tmp_path);
        return;
    }
    if std::fs::rename(&tmp_path, owner_path).is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
}

#[cfg(unix)]
fn try_lock_exclusive(file: &File) -> io::Result<bool> {
    use std::os::unix::io::AsRawFd;

    // Non-blocking: a busy repository must fail fast, never queue behind the
    // current owner.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(true);
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => Ok(false),
        _ => Err(err),
    }
}

/// Windows locking is future work; until it lands, no exclusion is claimed
/// rather than a false one being reported.
#[cfg(not(unix))]
fn try_lock_exclusive(_file: &File) -> io::Result<bool> {
    Ok(true)
}

/// Process-wide lock owner, kept alive for the whole process lifetime.
static ACTIVE_LOCK: OnceLock<RepositoryLock> = OnceLock::new();

/// Retain the acquired lock for the process lifetime.
pub fn install(lock: RepositoryLock) {
    let _ = ACTIVE_LOCK.set(lock);
}

/// Publish an API base URL onto the installed lock, if this process owns one.
///
/// A no-op for bypassed invocations and for non-repository workspaces, so
/// listener startup paths can call it unconditionally after a successful bind.
pub fn publish_api_url(url: &str) {
    if let Some(lock) = ACTIVE_LOCK.get() {
        lock.publish_api_url(url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_orchestration_entrypoints_acquire_and_others_bypass() {
        assert_eq!(
            classify_invocation(InvocationKind::DefaultTui),
            LockDecision::Acquire(InvocationMode::Tui)
        );
        assert_eq!(
            classify_invocation(InvocationKind::Tui),
            LockDecision::Acquire(InvocationMode::Tui)
        );
        assert_eq!(
            classify_invocation(InvocationKind::Run),
            LockDecision::Acquire(InvocationMode::Run)
        );
        assert_eq!(
            classify_invocation(InvocationKind::Other),
            LockDecision::Bypass
        );
    }

    #[test]
    fn owner_metadata_round_trips_without_api_url() {
        let metadata = OwnerMetadata {
            pid: Some(4321),
            started_at: Some("2026-07-31T12:00:00+00:00".to_string()),
            workspace: Some("/repo".to_string()),
            mode: Some("run".to_string()),
            api_url: None,
        };
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(!json.contains("api_url"), "json={json}");
        assert_eq!(parse_owner_metadata(&json), Some(metadata));
    }

    #[test]
    fn partial_metadata_keeps_readable_fields() {
        let parsed = parse_owner_metadata(r#"{"pid": 77, "mode": "server"}"#).unwrap();
        assert_eq!(parsed.pid, Some(77));
        assert_eq!(parsed.mode.as_deref(), Some("server"));
        assert_eq!(parsed.started_at, None);
        assert_eq!(parsed.api_url, None);
    }

    #[test]
    fn malformed_or_useless_metadata_parses_to_none() {
        assert_eq!(parse_owner_metadata("not json at all"), None);
        assert_eq!(parse_owner_metadata("{\"pid\": 12"), None);
        assert_eq!(parse_owner_metadata("{}"), None);
        // Blank fields and pid 0 describe no owner.
        assert_eq!(
            parse_owner_metadata(r#"{"pid": 0, "mode": "  ", "workspace": ""}"#),
            None
        );
        // A forward-compatible extra field must not discard readable fields.
        let parsed = parse_owner_metadata(r#"{"pid": 5, "future_field": true}"#).unwrap();
        assert_eq!(parsed.pid, Some(5));
    }

    #[test]
    fn conflict_message_reports_every_available_field() {
        let owner = OwnerMetadata {
            pid: Some(4321),
            started_at: Some("2026-07-31T12:00:00+00:00".to_string()),
            workspace: Some("/repo".to_string()),
            mode: Some("server".to_string()),
            api_url: Some("http://localhost:39876".to_string()),
        };
        let message = conflict_message(Path::new("/repo/.git"), Some(&owner));
        assert!(message.contains("/repo/.git"), "message={message}");
        assert!(message.contains("4321"), "message={message}");
        assert!(message.contains("server"), "message={message}");
        assert!(
            message.contains("2026-07-31T12:00:00+00:00"),
            "message={message}"
        );
        assert!(message.contains("/repo"), "message={message}");
        assert!(
            message.contains("http://localhost:39876"),
            "message={message}"
        );
    }

    #[test]
    fn conflict_message_omits_api_url_when_absent() {
        let owner = OwnerMetadata {
            pid: Some(4321),
            mode: Some("run".to_string()),
            ..OwnerMetadata::default()
        };
        let message = conflict_message(Path::new("/repo/.git"), Some(&owner));
        assert!(!message.contains("api url"), "message={message}");
        assert!(message.contains("4321"), "message={message}");
    }

    #[test]
    fn conflict_message_stays_valid_without_metadata() {
        let message = conflict_message(Path::new("/repo/.git"), None);
        assert!(
            message.contains("already orchestrating this repository"),
            "message={message}"
        );
        assert!(
            message.contains("owner details are unavailable"),
            "message={message}"
        );
        assert!(!message.contains("api url"), "message={message}");
    }

    #[test]
    fn normalize_path_cancels_parent_components() {
        assert_eq!(
            normalize_path(Path::new("/repo/.git/worktrees/wt/../..")),
            PathBuf::from("/repo/.git")
        );
        assert_eq!(
            normalize_path(Path::new("/repo/./.git")),
            PathBuf::from("/repo/.git")
        );
        assert_eq!(normalize_path(Path::new("/..")), PathBuf::from("/"));
    }

    #[test]
    fn plain_git_directory_is_its_own_common_dir() {
        let git_dir = resolve_git_dir(Path::new("/repo/.git"), None).unwrap();
        assert_eq!(git_dir, PathBuf::from("/repo/.git"));
        assert_eq!(
            resolve_common_dir(&git_dir, None),
            PathBuf::from("/repo/.git")
        );
    }

    #[test]
    fn linked_worktree_resolves_to_the_shared_common_dir() {
        let git_dir = resolve_git_dir(
            Path::new("/repo/wt/.git"),
            Some("gitdir: /repo/.git/worktrees/wt\n"),
        )
        .unwrap();
        assert_eq!(git_dir, PathBuf::from("/repo/.git/worktrees/wt"));
        assert_eq!(
            resolve_common_dir(&git_dir, Some("../..\n")),
            PathBuf::from("/repo/.git")
        );
    }

    #[test]
    fn relative_gitdir_pointer_resolves_against_the_worktree() {
        let git_dir = resolve_git_dir(
            Path::new("/repo/wt/.git"),
            Some("gitdir: ../.git/worktrees/wt"),
        )
        .unwrap();
        assert_eq!(git_dir, PathBuf::from("/repo/.git/worktrees/wt"));
    }

    #[test]
    fn absolute_commondir_is_used_verbatim() {
        assert_eq!(
            resolve_common_dir(
                Path::new("/repo/.git/worktrees/wt"),
                Some("/elsewhere/.git")
            ),
            PathBuf::from("/elsewhere/.git")
        );
    }

    #[test]
    fn unusable_gitdir_pointers_are_rejected() {
        assert_eq!(
            parse_gitdir_pointer("gitdir: /repo/.git"),
            Some("/repo/.git")
        );
        assert_eq!(parse_gitdir_pointer("gitdir:   "), None);
        assert_eq!(parse_gitdir_pointer("something else"), None);
        assert!(resolve_git_dir(Path::new("/repo/.git"), Some("garbage")).is_none());
    }
}
