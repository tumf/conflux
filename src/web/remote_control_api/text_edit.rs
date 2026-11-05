//! Bounded remote text read and write.
//!
//! `GET /api/v2/changes/{change_id}/text` and
//! `GET /api/v2/worktrees/{worktree_id}/text` read one file; the
//! `write_change_text` and `write_worktree_text` commands replace or create
//! one. Everything a client could steer is closed here:
//!
//! * **The root is server-derived.** `openspec/changes/<change_id>` of the
//!   project, or the directory an opaque worktree ID is bound to. A client never
//!   names a root, an absolute path, or a ref.
//! * **The path is contained.** Lexical rules first, then a descriptor walk from
//!   the root with `O_NOFOLLOW` on every component, re-run on every write, so a
//!   symlink substituted after a read is still refused. `.git` (file or
//!   directory, any depth), top-level `.wt/` and `.conflux-inflight`, and any
//!   directory below the root carrying its own `.git` entry are never reachable.
//! * **Writes are compare-and-swap.** `sha256:` identity of the exact bytes, then
//!   a same-directory temporary file, `fsync`, and `renameat` that keeps the
//!   permission bits. There is no unconditional write.
//! * **Writer exclusion is eligibility.** [`classify_text_eligibility`] is the
//!   one function both the read's `editable` and the command's refusal come
//!   from. It is evaluated once, at the start of a write.
//!
//! What it does *not* promise: the compare and the rename are not one atomic
//! step against an external editor, and a writer admitted after the
//! eligibility snapshot falls into the same window. Nothing is locked, killed,
//! committed, staged, or pushed.

use std::ffi::{CStr, CString};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::ai_command_runner::WriterCoverage;

use super::auth::CorrelationId;
use super::dto::{
    ApiError, CommandResult, ErrorCode, QueueIntent, TextEditBlockedReason, TextEditCapability,
    TextEditLimits, TextExpectation, TextReadResponse, TextScope, TextWriteEffects,
    TextWriteOutcome, TextWriteParams, TextWriteResult,
};
use super::executor::{CommandFailure, ExecutionSummary};
use super::projection::Projection;
use super::worktrees::WorktreeOperations;
use super::RemoteControlState;

/// Largest editable file, in bytes (1 MiB).
pub const MAX_TEXT_BYTES: u64 = 1_048_576;

/// Longest accepted relative path, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;

/// The one content identity algorithm.
pub const CONTENT_ID_ALGORITHM: &str = "sha256";

/// Route template of the change-scope read.
pub const CHANGE_TEXT_ROUTE: &str = "/api/v2/changes/{change_id}/text";

/// Route template of the worktree-scope read.
pub const WORKTREE_TEXT_ROUTE: &str = "/api/v2/worktrees/{worktree_id}/text";

/// Command types a write is submitted as.
pub const TEXT_WRITE_COMMANDS: [&str; 2] = ["write_change_text", "write_worktree_text"];

/// Marker in the name of the same-directory temporary file a write renames.
///
/// A crash between its creation and the rename leaves
/// `.<leaf>.cflx-tmp-<16 hex>` behind. It is ordinary untracked dirt: never
/// ignored, never excluded from a cleanliness check, never swept.
pub const TEMP_MARKER: &str = ".cflx-tmp-";

/// Paths and kinds that are never editable, as capabilities advertise them.
pub const EXCLUDED: [&str; 5] = [
    ".git",
    ".wt/",
    ".conflux-inflight",
    "nested-repository",
    "executable",
];

/// Event published after a successful write. It moves the sequence, not the
/// revision; the observation refresh it triggers may move the revision.
pub const TEXT_WRITTEN_EVENT: &str = "text_written";

// ============================================================================
// Path rules
// ============================================================================

/// Why a path is outside the editable scope, evaluated in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotEditable {
    /// A component equals `.git` (ASCII case-insensitive).
    GitMetadata,
    /// The first component is `.wt`, or the path is the top-level `.conflux-inflight`.
    ConfluxOwnedPath,
    /// The walk met a symlink or a non-directory, or the leaf is a symlink.
    UnsafePath,
    /// A traversed directory below the root carries its own `.git` entry.
    NestedRepository,
    /// The leaf is not a regular file.
    NotRegularFile,
    /// The leaf has an execute bit.
    Executable,
    /// The leaf exceeds [`MAX_TEXT_BYTES`].
    TooLarge,
    /// The bytes are not valid UTF-8 or contain NUL.
    NotText,
    /// The scope root cannot be opened as a non-symlink directory.
    RootMissing,
}

impl NotEditable {
    /// Every containment reason, in evaluation order.
    pub const ALL: [Self; 9] = [
        Self::GitMetadata,
        Self::ConfluxOwnedPath,
        Self::UnsafePath,
        Self::NestedRepository,
        Self::NotRegularFile,
        Self::Executable,
        Self::TooLarge,
        Self::NotText,
        Self::RootMissing,
    ];

    /// Wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitMetadata => "git_metadata",
            Self::ConfluxOwnedPath => "conflux_owned_path",
            Self::UnsafePath => "unsafe_path",
            Self::NestedRepository => "nested_repository",
            Self::NotRegularFile => "not_regular_file",
            Self::Executable => "executable",
            Self::TooLarge => "too_large",
            Self::NotText => "not_text",
            Self::RootMissing => "root_missing",
        }
    }
}

/// Writer-exclusion reasons, in evaluation order.
pub const ELIGIBILITY_REASONS: [TextEditBlockedReason; 4] = [
    TextEditBlockedReason::ChangeActive,
    TextEditBlockedReason::ChangeAdmitted,
    TextEditBlockedReason::ManagedProcessLive,
    TextEditBlockedReason::RootBusy,
];

/// Lexical validation, before any filesystem access.
///
/// Refuses an empty path, more than [`MAX_PATH_BYTES`], a leading `/`, any
/// `\`, any NUL, and any empty, `.`, or `..` component (which also covers a
/// trailing `/`). The string is UTF-8 by construction.
pub fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("path must not be empty".to_string());
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(format!("path exceeds {MAX_PATH_BYTES} bytes"));
    }
    if path.starts_with('/') {
        return Err("path must be relative to the scope root".to_string());
    }
    if path.contains('\\') {
        return Err("path must not contain a backslash".to_string());
    }
    if path.contains('\0') {
        return Err("path must not contain NUL".to_string());
    }
    for component in path.split('/') {
        match component {
            "" => return Err("path must not contain an empty component".to_string()),
            "." | ".." => return Err("path must not contain a `.` or `..` component".to_string()),
            _ => {}
        }
    }
    Ok(())
}

/// The containment reasons decided from the components alone.
fn lexical_refusal(components: &[&str]) -> Option<NotEditable> {
    if components
        .iter()
        .any(|component| component.eq_ignore_ascii_case(".git"))
    {
        return Some(NotEditable::GitMetadata);
    }
    if components.first() == Some(&".wt") || components == [".conflux-inflight"] {
        return Some(NotEditable::ConfluxOwnedPath);
    }
    None
}

// ============================================================================
// Content identity
// ============================================================================

/// `sha256:` followed by 64 lowercase hex digits of SHA-256 over `bytes`.
pub fn content_id(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Whether `value` has the published `content_id` shape.
pub fn is_content_id(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// Octal permission bits as published, for example `0644`.
fn format_mode(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}

// ============================================================================
// Descriptor walk
// ============================================================================

/// Why a filesystem step refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextFailure {
    /// A lexical rule refused the path.
    Lexical(String),
    /// The path is outside the editable scope.
    NotEditable(NotEditable),
    /// An intermediate directory does not exist; none is created.
    MissingDirectory,
    /// An unexpected filesystem failure. The message names no absolute path.
    Io(String),
}

fn io_failure(step: &str, error: std::io::Error) -> TextFailure {
    TextFailure::Io(format!("{step} failed ({})", error.kind()))
}

fn c_name(component: &str) -> Result<CString, TextFailure> {
    CString::new(component).map_err(|_| TextFailure::Lexical("path contains NUL".to_string()))
}

fn openat(
    dir: &OwnedFd,
    name: &CStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> std::io::Result<OwnedFd> {
    // SAFETY: `name` is a valid NUL-terminated string and `dir` an open
    // descriptor for the duration of the call; the result is owned below.
    let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags, mode as libc::c_uint) };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        // SAFETY: `fd` is a fresh descriptor nothing else owns.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

fn fstat(fd: &OwnedFd) -> std::io::Result<libc::stat> {
    // SAFETY: `stat` is plain old data and fully written by a successful call.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) } < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(stat)
    }
}

/// Whether `dir` holds an entry named `name` of any type, without following it.
fn entry_exists_at(dir: &OwnedFd, name: &CStr) -> std::io::Result<bool> {
    // SAFETY: as in `fstat`; `name` is NUL-terminated.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ENOENT) {
        Ok(false)
    } else {
        Err(error)
    }
}

const DIR_FLAGS: libc::c_int =
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;

/// Open the scope root as a non-symlink directory.
fn open_root(root: &FsPath) -> Result<OwnedFd, TextFailure> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(root.as_os_str().as_bytes())
        .map_err(|_| TextFailure::NotEditable(NotEditable::RootMissing))?;
    // SAFETY: `path` is NUL-terminated; the result is owned below.
    let fd = unsafe { libc::open(path.as_ptr(), DIR_FLAGS) };
    if fd < 0 {
        return Err(TextFailure::NotEditable(NotEditable::RootMissing));
    }
    // SAFETY: `fd` is a fresh descriptor nothing else owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// The current leaf, read through the walk's own descriptor.
#[derive(Debug, Clone)]
struct Leaf {
    bytes: Vec<u8>,
    mode: u32,
}

/// A completed walk: the parent directory descriptor and the leaf, if present.
struct Walked {
    parent: OwnedFd,
    leaf_name: CString,
    leaf: Option<Leaf>,
}

/// Walk `path` from `root` and apply every containment rule.
///
/// Every directory component is opened with `O_DIRECTORY | O_NOFOLLOW`, and
/// each one below the root is probed for its own `.git` entry before the walk
/// continues. The leaf is opened `O_NOFOLLOW | O_NONBLOCK`, so neither a
/// symlink nor a FIFO can be followed or block the walk.
fn walk(root: &FsPath, path: &str) -> Result<Walked, TextFailure> {
    validate_path(path).map_err(TextFailure::Lexical)?;
    let components: Vec<&str> = path.split('/').collect();
    if let Some(reason) = lexical_refusal(&components) {
        return Err(TextFailure::NotEditable(reason));
    }

    let git = c_name(".git")?;
    let (leaf_name, directories) = components.split_last().expect("validated as non-empty");
    let mut dir = open_root(root)?;
    for component in directories {
        let name = c_name(component)?;
        dir = match openat(&dir, &name, DIR_FLAGS, 0) {
            Ok(next) => next,
            Err(error) => {
                return Err(match error.raw_os_error() {
                    Some(libc::ENOENT) => TextFailure::MissingDirectory,
                    Some(libc::ELOOP) | Some(libc::ENOTDIR) => {
                        TextFailure::NotEditable(NotEditable::UnsafePath)
                    }
                    _ => io_failure("opening a directory component", error),
                })
            }
        };
        match entry_exists_at(&dir, &git) {
            Ok(false) => {}
            Ok(true) => return Err(TextFailure::NotEditable(NotEditable::NestedRepository)),
            Err(error) => return Err(io_failure("probing for a nested repository", error)),
        }
    }

    let leaf_name = c_name(leaf_name)?;
    let leaf = match openat(
        &dir,
        &leaf_name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOCTTY,
        0,
    ) {
        Ok(fd) => Some(read_leaf(fd)?),
        Err(error) => match error.raw_os_error() {
            Some(libc::ENOENT) => None,
            Some(libc::ELOOP) | Some(libc::ENOTDIR) => {
                return Err(TextFailure::NotEditable(NotEditable::UnsafePath))
            }
            _ => return Err(io_failure("opening the file", error)),
        },
    };
    Ok(Walked {
        parent: dir,
        leaf_name,
        leaf,
    })
}

/// Apply the leaf rules and read its bytes, bounded by the size check first.
fn read_leaf(fd: OwnedFd) -> Result<Leaf, TextFailure> {
    let stat = fstat(&fd).map_err(|error| io_failure("inspecting the file", error))?;
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err(TextFailure::NotEditable(NotEditable::NotRegularFile));
    }
    let mode = stat.st_mode as u32 & 0o7777;
    if mode & 0o111 != 0 {
        return Err(TextFailure::NotEditable(NotEditable::Executable));
    }
    if stat.st_size as u64 > MAX_TEXT_BYTES {
        return Err(TextFailure::NotEditable(NotEditable::TooLarge));
    }
    let mut bytes = Vec::with_capacity(stat.st_size as usize);
    // One byte past the ceiling, so a file that grew after `fstat` is caught.
    std::fs::File::from(fd)
        .take(MAX_TEXT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_failure("reading the file", error))?;
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        return Err(TextFailure::NotEditable(NotEditable::TooLarge));
    }
    if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
        return Err(TextFailure::NotEditable(NotEditable::NotText));
    }
    Ok(Leaf { bytes, mode })
}

// ============================================================================
// Read and write
// ============================================================================

/// One bounded read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    /// The file's text; `None` when absent.
    pub content: Option<String>,
    /// Permission bits; `None` when absent.
    pub mode: Option<u32>,
}

/// Read one file under `root` through the descriptor walk.
pub fn read_text(root: &FsPath, path: &str) -> Result<TextFile, TextFailure> {
    let walked = walk(root, path)?;
    Ok(match walked.leaf {
        None => TextFile {
            content: None,
            mode: None,
        },
        Some(leaf) => TextFile {
            // `read_leaf` already proved the bytes are UTF-8.
            content: Some(String::from_utf8(leaf.bytes).unwrap_or_default()),
            mode: Some(leaf.mode),
        },
    })
}

/// How one write transaction settled on the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteSettlement {
    /// The bytes were replaced or created.
    Written {
        /// Identity before; `None` when the file was created.
        before: Option<String>,
        /// Identity re-read after the rename.
        after: String,
        /// Bytes written.
        bytes: u64,
        /// Permission bits re-read after the rename.
        mode: u32,
    },
    /// The rename landed, but re-reading the new entry failed.
    ///
    /// The bytes are on disk: this is never reported as "nothing written".
    WrittenUnverified {
        /// Identity before; `None` when the file was created.
        before: Option<String>,
        /// Bytes written.
        bytes: u64,
        /// Why the re-read failed.
        failure: TextFailure,
    },
    /// The content already matched; nothing was opened for writing.
    Unchanged {
        /// The unchanged identity.
        content_id: String,
        /// The unchanged permission bits.
        mode: u32,
    },
    /// `expected` no longer holds; nothing was written.
    Stale {
        /// What the file holds now; `None` when absent.
        current: Option<String>,
    },
    /// A rule or the filesystem refused; nothing was written.
    Refused(TextFailure),
}

/// Compare-and-swap one file under `root`.
pub fn write_text(
    root: &FsPath,
    path: &str,
    expected: &TextExpectation,
    content: &[u8],
) -> WriteSettlement {
    write_text_inner(root, path, expected, content, Inject::Nothing)
}

/// A fault a test injects into the write transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inject {
    Nothing,
    /// Fail after the temporary file is written, before the rename.
    BeforeRename,
    /// Make the post-rename re-read fail.
    AfterRename,
}

fn write_text_inner(
    root: &FsPath,
    path: &str,
    expected: &TextExpectation,
    content: &[u8],
    inject: Inject,
) -> WriteSettlement {
    let walked = match walk(root, path) {
        Ok(walked) => walked,
        Err(failure) => return WriteSettlement::Refused(failure),
    };
    let before = walked.leaf.as_ref().map(|leaf| content_id(&leaf.bytes));

    match (expected, &before) {
        (TextExpectation::Absent {}, Some(current)) => {
            return WriteSettlement::Stale {
                current: Some(current.clone()),
            }
        }
        (TextExpectation::Content { .. }, None) => return WriteSettlement::Stale { current: None },
        (TextExpectation::Content { content_id }, Some(current)) if content_id != current => {
            return WriteSettlement::Stale {
                current: Some(current.clone()),
            }
        }
        _ => {}
    }
    if let (Some(leaf), Some(current)) = (&walked.leaf, &before) {
        if leaf.bytes == content {
            return WriteSettlement::Unchanged {
                content_id: current.clone(),
                mode: leaf.mode,
            };
        }
    }

    let mode = walked.leaf.as_ref().map(|leaf| leaf.mode & 0o7777);
    if let Err(failure) = replace_leaf(
        &walked.parent,
        &walked.leaf_name,
        content,
        mode,
        inject == Inject::BeforeRename,
    ) {
        return WriteSettlement::Refused(failure);
    }
    if inject == Inject::AfterRename {
        return WriteSettlement::WrittenUnverified {
            before,
            bytes: content.len() as u64,
            failure: TextFailure::Io("injected re-read failure".to_string()),
        };
    }

    // Re-read through a fresh `O_NOFOLLOW` open, so the reported identity is
    // what the directory entry now holds rather than what was intended.
    match openat(
        &walked.parent,
        &walked.leaf_name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        0,
    )
    .map_err(|error| io_failure("re-reading the written file", error))
    .and_then(read_leaf)
    {
        Ok(leaf) => WriteSettlement::Written {
            before,
            after: content_id(&leaf.bytes),
            bytes: content.len() as u64,
            mode: leaf.mode,
        },
        Err(failure) => WriteSettlement::WrittenUnverified {
            before,
            bytes: content.len() as u64,
            failure,
        },
    }
}

/// Create the temporary file, write, `fsync`, and rename it over the leaf.
///
/// The temporary file is removed on every failure path this process survives.
fn replace_leaf(
    parent: &OwnedFd,
    leaf: &CStr,
    content: &[u8],
    mode: Option<u32>,
    fail_before_rename: bool,
) -> Result<(), TextFailure> {
    let temp_name = format!(
        ".{}{TEMP_MARKER}{:016x}",
        leaf.to_string_lossy(),
        rand::random::<u64>()
    );
    let temp = c_name(&temp_name)?;
    let file = openat(
        parent,
        &temp,
        libc::O_CREAT | libc::O_EXCL | libc::O_WRONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        // A new file is `0644` subject to the process umask; an existing one
        // keeps exactly its own bits, restored by `fchmod` below.
        mode.unwrap_or(0o644) as libc::mode_t,
    )
    .map_err(|error| io_failure("creating the temporary file", error))?;

    let mut file = std::fs::File::from(file);
    let written = (|| -> std::io::Result<()> {
        if let Some(mode) = mode {
            // SAFETY: `file` owns an open descriptor for the whole call.
            if unsafe { libc::fchmod(file.as_raw_fd(), mode as libc::mode_t) } < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        file.write_all(content)?;
        file.sync_all()?;
        if fail_before_rename {
            return Err(std::io::Error::other("injected failure before rename"));
        }
        // SAFETY: both names are NUL-terminated and `parent` is open.
        if unsafe {
            libc::renameat(
                parent.as_raw_fd(),
                temp.as_ptr(),
                parent.as_raw_fd(),
                leaf.as_ptr(),
            )
        } < 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    })();
    drop(file);

    if let Err(error) = written {
        // SAFETY: as above; a failure here leaves the residue the usage guide
        // names, which is reported as ordinary untracked dirt.
        unsafe { libc::unlinkat(parent.as_raw_fd(), temp.as_ptr(), 0) };
        return Err(io_failure("replacing the file", error));
    }
    // The rename already landed; a failed directory sync is not a reason to
    // report the bytes as unwritten.
    // SAFETY: `parent` is an open directory descriptor.
    if unsafe { libc::fsync(parent.as_raw_fd()) } < 0 {
        tracing::warn!(
            "text write: syncing the parent directory failed: {}",
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}

// ============================================================================
// Eligibility (writer exclusion)
// ============================================================================

/// The facts writer exclusion is decided from, taken once per request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibilityFacts {
    /// The change's — for a worktree, its owning change's — display status and
    /// whether its queue intent is `queued`. `None` when no change owns it.
    pub owner: Option<(String, bool)>,
    /// Managed writer coverage for the canonical root.
    pub coverage: WriterCoverage,
    /// Whether the repository root mutation guard is currently held.
    pub root_guard_held: bool,
    /// Whether the base repository is merging; `None` when it could not be read.
    pub base_merge_in_progress: Option<bool>,
    /// Whether a resolve is running.
    pub is_resolving: bool,
}

/// A writer-exclusion refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ineligible {
    /// The first matching reason.
    pub reason: TextEditBlockedReason,
    /// Bounded diagnostic.
    pub detail: Option<String>,
}

/// The one writer-exclusion classifier the read and the command share.
///
/// Reasons are evaluated in [`ELIGIBILITY_REASONS`] order and the first match
/// is published. Only `Clear` coverage falls through to `root_busy`: `Busy`
/// and `Unknown` both refuse as `managed_process_live`.
pub fn classify_text_eligibility(facts: &EligibilityFacts) -> Result<(), Ineligible> {
    if let Some((status, queued)) = &facts.owner {
        if crate::orchestration::operator_command::is_active_status(status) {
            return Err(Ineligible {
                reason: TextEditBlockedReason::ChangeActive,
                detail: Some(format!("the owning change is '{status}'")),
            });
        }
        if *queued || matches!(status.as_str(), "queued" | "blocked") {
            return Err(Ineligible {
                reason: TextEditBlockedReason::ChangeAdmitted,
                detail: Some(format!(
                    "the owning change is admitted ('{status}', queue intent {})",
                    if *queued { "queued" } else { "not queued" }
                )),
            });
        }
    }
    if let Some(detail) = facts.coverage.detail() {
        return Err(Ineligible {
            reason: TextEditBlockedReason::ManagedProcessLive,
            detail: Some(crate::ai_command_runner::bound_coverage_detail(detail)),
        });
    }
    let busy = if facts.root_guard_held {
        Some("the repository root guard is held by another operation")
    } else if facts.base_merge_in_progress != Some(false) {
        Some(match facts.base_merge_in_progress {
            Some(_) => "a base merge is in progress",
            None => "whether a base merge is in progress could not be determined",
        })
    } else if facts.is_resolving {
        Some("a resolve is running")
    } else {
        None
    };
    match busy {
        Some(detail) => Err(Ineligible {
            reason: TextEditBlockedReason::RootBusy,
            detail: Some(detail.to_string()),
        }),
        None => Ok(()),
    }
}

// ============================================================================
// Scope resolution
// ============================================================================

/// The owner-side facts a text edit is resolved against.
///
/// Implemented by the monitoring state, which already knows the project root,
/// the change-to-worktree relation, and the managed-termination port.
#[async_trait]
pub trait TextEditSource: Send + Sync {
    /// The project repository root.
    async fn project_root(&self) -> Option<PathBuf>;
    /// The change whose worktree relation is `worktree`, if any.
    async fn worktree_owner(&self, worktree: &FsPath) -> Option<String>;
    /// Managed writer coverage for a canonical root. Never `Clear` by absence.
    async fn writer_coverage_under(&self, root: &FsPath) -> WriterCoverage;
    /// Republish the observation a successful write may have changed.
    async fn after_text_write(&self);
}

/// Everything one text request needs, assembled per request.
pub struct TextEditContext {
    /// The projection eligibility is read from and events published to.
    pub projection: Arc<Projection>,
    /// The owner-side facts; `None` is a process that serves no text edit.
    pub source: Option<Arc<dyn TextEditSource>>,
    /// The worktree port that resolves opaque IDs and probes the root guard.
    pub worktrees: Arc<dyn WorktreeOperations>,
}

/// A resolved scope root plus its eligibility snapshot.
struct Target {
    root: PathBuf,
    owner_change: Option<String>,
    facts: EligibilityFacts,
    state_revision: u64,
}

/// A refusal with the typed result a write record carries.
fn refused(code: ErrorCode, message: String, result: Option<TextWriteResult>) -> CommandFailure {
    let failure = CommandFailure::new(code, message);
    match result {
        Some(result) => failure.with_result(CommandResult::TextWrite(Box::new(result))),
        None => failure,
    }
}

fn empty_result(scope: &TextScope, path: &str) -> TextWriteResult {
    TextWriteResult {
        scope: scope.clone(),
        path: path.to_string(),
        outcome: None,
        reason: None,
        detail: None,
        content_id_before: None,
        content_id_after: None,
        current_content_id: None,
        bytes_written: None,
        mode: None,
        effects: TextWriteEffects {
            file_written: false,
        },
    }
}

/// Map a filesystem refusal onto the wire, with a write result when writing.
fn map_failure(failure: TextFailure, write: Option<(&TextScope, &str)>) -> CommandFailure {
    let result_with = |reason: Option<&str>, detail: Option<String>| {
        write.map(|(scope, path)| TextWriteResult {
            reason: reason.map(str::to_string),
            detail,
            ..empty_result(scope, path)
        })
    };
    match failure {
        TextFailure::Lexical(message) => CommandFailure::new(ErrorCode::ValidationFailed, message),
        TextFailure::NotEditable(reason) => refused(
            ErrorCode::TextNotEditable,
            format!("the path is not editable: {}", reason.as_str()),
            result_with(Some(reason.as_str()), None),
        ),
        TextFailure::MissingDirectory => CommandFailure::new(
            ErrorCode::NotFound,
            "an intermediate directory of the path does not exist; none is created",
        ),
        TextFailure::Io(message) => refused(
            ErrorCode::InternalError,
            message.clone(),
            result_with(None, Some(message)),
        ),
    }
}

impl TextEditContext {
    fn source(&self) -> Result<Arc<dyn TextEditSource>, CommandFailure> {
        self.source.clone().ok_or_else(|| {
            CommandFailure::new(
                ErrorCode::LifecycleConflict,
                "this instance serves no text edit surface",
            )
        })
    }

    /// Resolve the root and take the eligibility snapshot.
    ///
    /// `writing` is set for a write, so a refusal here still carries the
    /// typed result a write record reports.
    async fn resolve(
        &self,
        scope: &TextScope,
        path: &str,
        writing: bool,
    ) -> Result<Target, CommandFailure> {
        let source = self.source()?;
        let root_missing = |message: &str| {
            refused(
                ErrorCode::TextNotEditable,
                format!("{message}: {}", NotEditable::RootMissing.as_str()),
                writing.then(|| TextWriteResult {
                    reason: Some(NotEditable::RootMissing.as_str().to_string()),
                    ..empty_result(scope, path)
                }),
            )
        };
        let (snapshot, state_revision, _) = self.projection.snapshot();
        let row = |change_id: &str| {
            snapshot
                .changes
                .iter()
                .find(|change| change.id == change_id)
                .map(|change| {
                    (
                        change.display_status.clone(),
                        matches!(change.queue_intent, QueueIntent::Queued),
                    )
                })
        };

        let (root, owner_change, owner) = match scope {
            TextScope::Change { change_id } => {
                let owner = row(change_id)
                    .filter(|(status, _)| {
                        !crate::orchestration::operator_command::is_completed_status(status)
                    })
                    .ok_or_else(|| {
                        CommandFailure::new(
                            ErrorCode::NotFound,
                            format!(
                                "change '{change_id}' is not a tracked, non-archived change in \
                                 this instance"
                            ),
                        )
                    })?;
                let project_root = source.project_root().await.ok_or_else(|| {
                    root_missing("the project root is not known to this instance")
                })?;
                (
                    project_root
                        .join("openspec")
                        .join("changes")
                        .join(change_id),
                    Some(change_id.clone()),
                    Some(owner),
                )
            }
            TextScope::Worktree { worktree_id } => {
                let path = self.worktrees.text_root(worktree_id).await?;
                let owner_change = source.worktree_owner(&path).await;
                let owner = owner_change.as_deref().and_then(row);
                (path, owner_change, owner)
            }
        };

        // The root itself must be a real directory, not a symlink to one.
        let canonical = match std::fs::symlink_metadata(&root) {
            Ok(meta) if meta.is_dir() => std::fs::canonicalize(&root).ok(),
            _ => None,
        }
        .ok_or_else(|| root_missing("the scope root is not a directory"))?;

        // Through the main worktree, a path inside `openspec/changes/<id>/`
        // belongs to that change: its own eligibility applies, so the project
        // root is never a way around a change-scope refusal.
        let (owner_change, owner) = match (owner_change, owner) {
            (None, None) if matches!(scope, TextScope::Worktree { .. }) => {
                let project_root = source
                    .project_root()
                    .await
                    .and_then(|root| std::fs::canonicalize(root).ok());
                let components: Vec<&str> = path.split('/').collect();
                match (project_root, components.as_slice()) {
                    (Some(project_root), ["openspec", "changes", change_id, _, ..])
                        if project_root == canonical =>
                    {
                        match row(change_id) {
                            Some(owner) => (Some(change_id.to_string()), Some(owner)),
                            None => (None, None),
                        }
                    }
                    _ => (None, None),
                }
            }
            other => other,
        };

        let base_merge_in_progress = match source.project_root().await {
            Some(project_root) => crate::vcs::git::commands::is_merge_in_progress(&project_root)
                .await
                .ok(),
            None => None,
        };
        let facts = EligibilityFacts {
            owner,
            coverage: source.writer_coverage_under(&canonical).await,
            root_guard_held: self.worktrees.root_guard_is_held().await,
            base_merge_in_progress,
            is_resolving: snapshot.is_resolving,
        };
        Ok(Target {
            root: canonical,
            owner_change,
            facts,
            state_revision,
        })
    }

    /// Serve one bounded read.
    pub async fn read(
        &self,
        scope: TextScope,
        path: String,
    ) -> Result<TextReadResponse, CommandFailure> {
        validate_path(&path)
            .map_err(|message| CommandFailure::new(ErrorCode::ValidationFailed, message))?;
        let target = self.resolve(&scope, &path, false).await?;
        let root = target.root.clone();
        let read_path = path.clone();
        let file = tokio::task::spawn_blocking(move || read_text(&root, &read_path))
            .await
            .map_err(|_| CommandFailure::new(ErrorCode::InternalError, "the read task failed"))?
            .map_err(|failure| map_failure(failure, None))?;
        let eligibility = classify_text_eligibility(&target.facts);
        let (reason, detail) = match eligibility {
            Ok(()) => (None, None),
            Err(ineligible) => (Some(ineligible.reason), ineligible.detail),
        };
        Ok(TextReadResponse {
            instance_id: self.projection.instance_id().to_string(),
            state_revision: target.state_revision,
            scope,
            path,
            exists: file.content.is_some(),
            content_id: file
                .content
                .as_deref()
                .map(|text| content_id(text.as_bytes())),
            size_bytes: file.content.as_ref().map(|text| text.len() as u64),
            content: file.content,
            mode: file.mode.map(format_mode),
            editable: reason.is_none(),
            edit_blocked_reason: reason,
            edit_blocked_detail: detail,
            limits: limits(),
        })
    }

    /// Run one write transaction. The caller holds the application gate.
    pub async fn write(
        &self,
        scope: TextScope,
        params: &TextWriteParams,
    ) -> Result<ExecutionSummary, CommandFailure> {
        let path = params.path.clone();
        // 1. Root and eligibility snapshot, before touching the filesystem.
        let target = self.resolve(&scope, &path, true).await?;
        if let Err(ineligible) = classify_text_eligibility(&target.facts) {
            return Err(refused(
                ErrorCode::TargetIneligible,
                format!(
                    "the target is not editable now: {}",
                    ineligible.reason.as_str()
                ),
                Some(TextWriteResult {
                    reason: Some(ineligible.reason.as_str().to_string()),
                    detail: ineligible.detail,
                    ..empty_result(&scope, &path)
                }),
            ));
        }

        // 2-5. Walk, compare, replace, re-read.
        let root = target.root.clone();
        let write_path = path.clone();
        let expected = params.expected.clone();
        let content = params.content.clone().into_bytes();
        let settlement = tokio::task::spawn_blocking(move || {
            write_text(&root, &write_path, &expected, &content)
        })
        .await
        .map_err(|_| CommandFailure::new(ErrorCode::InternalError, "the write task failed"))?;

        match settlement {
            WriteSettlement::Refused(failure) => Err(map_failure(failure, Some((&scope, &path)))),
            WriteSettlement::WrittenUnverified {
                before,
                bytes,
                failure,
            } => {
                let reason = match &failure {
                    TextFailure::NotEditable(reason) => reason.as_str().to_string(),
                    TextFailure::Lexical(message) | TextFailure::Io(message) => message.clone(),
                    TextFailure::MissingDirectory => "the written entry disappeared".to_string(),
                };
                if let Some(source) = &self.source {
                    source.after_text_write().await;
                }
                Err(refused(
                    ErrorCode::InternalError,
                    format!(
                        "'{path}' was written ({bytes} bytes) but could not be re-read: {reason}"
                    ),
                    Some(TextWriteResult {
                        detail: Some(format!("written, but the re-read failed: {reason}")),
                        content_id_before: before,
                        bytes_written: Some(bytes),
                        effects: TextWriteEffects { file_written: true },
                        ..empty_result(&scope, &path)
                    }),
                ))
            }
            WriteSettlement::Stale { current } => Err(refused(
                ErrorCode::TextStale,
                "the file no longer holds the expected content; nothing was written".to_string(),
                Some(TextWriteResult {
                    reason: Some("stale".to_string()),
                    current_content_id: current,
                    ..empty_result(&scope, &path)
                }),
            )),
            WriteSettlement::Unchanged { content_id, mode } => Ok(ExecutionSummary::no_op(
                format!("'{path}' already holds the requested content"),
            )
            .with_result(CommandResult::TextWrite(Box::new(TextWriteResult {
                outcome: Some(TextWriteOutcome::Unchanged),
                content_id_before: Some(content_id.clone()),
                content_id_after: Some(content_id.clone()),
                current_content_id: Some(content_id),
                bytes_written: Some(0),
                mode: Some(format_mode(mode)),
                ..empty_result(&scope, &path)
            })))),
            WriteSettlement::Written {
                before,
                after,
                bytes,
                mode,
            } => {
                // 6. One sequence-only event, then the ordinary observation
                // refresh, which may advance the revision when `dirty` moves.
                self.projection.apply_presentation(
                    TEXT_WRITTEN_EVENT,
                    target.owner_change.clone(),
                    serde_json::json!({
                        "scope": scope,
                        "path": path,
                        "content_id_after": after,
                    }),
                );
                if matches!(scope, TextScope::Worktree { .. }) {
                    self.worktrees.refresh_after_text_write().await;
                }
                if let Some(source) = &self.source {
                    source.after_text_write().await;
                }
                Ok(ExecutionSummary::changed(format!(
                    "'{path}' written ({bytes} bytes); nothing was committed, staged, or pushed"
                ))
                .with_result(CommandResult::TextWrite(Box::new(
                    TextWriteResult {
                        outcome: Some(TextWriteOutcome::Written),
                        content_id_before: before,
                        content_id_after: Some(after.clone()),
                        current_content_id: Some(after),
                        bytes_written: Some(bytes),
                        mode: Some(format_mode(mode)),
                        effects: TextWriteEffects { file_written: true },
                        ..empty_result(&scope, &path)
                    },
                ))))
            }
        }
    }
}

/// The fixed bounds every read repeats.
pub fn limits() -> TextEditLimits {
    TextEditLimits {
        max_bytes: MAX_TEXT_BYTES,
        max_path_bytes: MAX_PATH_BYTES,
        content_id_algorithm: CONTENT_ID_ALGORITHM.to_string(),
    }
}

/// The capability this build serves, with `available` as given.
pub fn capability(available: bool) -> TextEditCapability {
    TextEditCapability {
        available,
        read_routes: vec![
            CHANGE_TEXT_ROUTE.to_string(),
            WORKTREE_TEXT_ROUTE.to_string(),
        ],
        commands: TEXT_WRITE_COMMANDS.iter().map(|c| c.to_string()).collect(),
        max_bytes: MAX_TEXT_BYTES,
        max_path_bytes: MAX_PATH_BYTES,
        content_id_algorithm: CONTENT_ID_ALGORITHM.to_string(),
        excluded: EXCLUDED.iter().map(|e| e.to_string()).collect(),
        reasons: NotEditable::ALL
            .iter()
            .map(|reason| reason.as_str().to_string())
            .chain(
                ELIGIBILITY_REASONS
                    .iter()
                    .map(|reason| reason.as_str().to_string()),
            )
            .collect(),
    }
}

// ============================================================================
// Read routes
// ============================================================================

/// Query of the two reads.
#[derive(Debug, Deserialize)]
pub struct TextQuery {
    /// Root-relative path.
    pub path: Option<String>,
}

fn context(state: &RemoteControlState) -> TextEditContext {
    TextEditContext {
        projection: state.projection.clone(),
        source: state.text_edit.clone(),
        worktrees: state.worktrees.clone(),
    }
}

async fn serve_read(
    state: RemoteControlState,
    correlation: CorrelationId,
    scope: TextScope,
    query: TextQuery,
) -> Response {
    let Some(path) = query.path else {
        return ApiError::new(
            ErrorCode::ValidationFailed,
            "the text read requires a path query parameter",
            &correlation.0,
        )
        .into_response();
    };
    match context(&state).read(scope, path).await {
        Ok(body) => ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response(),
        Err(failure) => ApiError::new(failure.error_code, failure.message, &correlation.0)
            .with_revision(state.projection.revision())
            .into_response(),
    }
}

/// Read one bounded text file inside a change's directory.
#[utoipa::path(
    get,
    path = "/api/v2/changes/{change_id}/text",
    tag = "remote-control",
    params(
        ("change_id" = String, Path, description = "Tracked, non-archived change ID"),
        ("path" = String, Query, description = "Path relative to `openspec/changes/<change_id>`")
    ),
    responses(
        (status = 200, description = "The file, its identity, and whether a write would be accepted now", body = TextReadResponse),
        (status = 404, description = "No such change, or a missing intermediate directory", body = ApiError),
        (status = 409, description = "`text_not_editable`: the path is outside the editable scope", body = ApiError),
        (status = 422, description = "The path failed lexical validation", body = ApiError)
    )
)]
pub async fn read_change_text(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
    Path(change_id): Path<String>,
    Query(query): Query<TextQuery>,
) -> Response {
    serve_read(state, correlation, TextScope::Change { change_id }, query).await
}

/// Read one bounded text file inside a worktree addressed by opaque ID.
#[utoipa::path(
    get,
    path = "/api/v2/worktrees/{worktree_id}/text",
    tag = "remote-control",
    params(
        ("worktree_id" = String, Path, description = "Opaque process-local worktree ID"),
        ("path" = String, Query, description = "Path relative to the worktree directory")
    ),
    responses(
        (status = 200, description = "The file, its identity, and whether a write would be accepted now", body = TextReadResponse),
        (status = 404, description = "Unknown or retired worktree ID, or a missing intermediate directory", body = ApiError),
        (status = 409, description = "`text_not_editable`, or no worktree runtime is bound", body = ApiError),
        (status = 422, description = "The path failed lexical validation", body = ApiError)
    )
)]
pub async fn read_worktree_text(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
    Path(worktree_id): Path<String>,
    Query(query): Query<TextQuery>,
) -> Response {
    serve_read(
        state,
        correlation,
        TextScope::Worktree { worktree_id },
        query,
    )
    .await
}

#[cfg(test)]
mod tests;
