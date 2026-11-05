//! Consent-bound destructive worktree deletion: the consent vocabulary and the
//! bounded, read-only content identity oracle it is bound to.
//!
//! A remote client cannot look at a worktree before it deletes it, so what it
//! consents to has to be *named*: branch, HEAD, the dirty and ahead facts, the
//! merge base, and two content identities. The service re-derives every one of
//! them under the repository mutation guard immediately before teardown and
//! again immediately before removal, and refuses on any difference.
//!
//! The identities bind **bytes**. Metadata (type, size, mtime, mode) is bound in
//! addition, never instead: a same-size rewrite with the mtime and mode restored
//! leaves every metadata field intact, so an identity built from metadata alone
//! would turn the `content_changed` refusal into a false guarantee.
//!
//! What is bound is what `git worktree remove --force` deletes beyond `HEAD`:
//!
//! * every staged, unstaged, unmerged, and untracked record of
//!   `git status --porcelain=v2 -z --untracked-files=normal --ignored=traditional`
//!   together with the on-disk bytes of its path — the discardable identity;
//! * every ignored record and its bytes — the ignored identity, a separate
//!   fact because `dirty` keeps its `--ignored=no` meaning;
//! * everything beneath a directory record Git collapses (`? dir/`, `! dir/`),
//!   which the oracle walks itself. Git's own expansion hides `.git` entries,
//!   empty directories, and special files, so it is not trusted to enumerate
//!   the deleted set.
//!
//! Every path is resolved one component at a time from an open worktree root
//! descriptor with `O_NOFOLLOW`; files are opened `O_NONBLOCK` and re-checked
//! with `fstat`, so a FIFO cannot park the owner under the root guard and a
//! component swapped for a symlink cannot redirect the read. The walk never
//! enters a symlink.
//!
//! One combined budget covers tracked, untracked, and ignored content. Past it
//! the observation is [`ContentObservation::Unbounded`], which carries no
//! identity at all: there is no metadata-only, prefix, or partial fallback, and
//! ignored content is never dropped to fit. An actual `.git` descendant or a
//! status-reported submodule is [`ContentObservation::NestedRepository`]. Both
//! fail the confirmed command closed whatever permissions were given; reducing
//! such content stays a local operation.
//!
//! The oracle only reads: it never stages, writes, runs hooks, or excludes a
//! path, and nothing it computes is stored.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(feature = "web-monitoring")]
use utoipa::ToSchema;

use crate::vcs::git::commands::status_policy::{read_only_status_argv, CONSENT_STATUS_ARGS};

/// Prefix of every published content identity.
pub const CONTENT_ID_PREFIX: &str = "sha256:";

/// The one hash algorithm content identities are computed with.
pub const CONTENT_ID_ALGORITHM: &str = "sha256";

// ============================================================================
// Limits
// ============================================================================

/// The oracle's combined budget over tracked, untracked, and ignored content.
///
/// Each path counts once. `records` counts status records plus every walked
/// entry; `hashed_bytes` counts regular-file bytes and symlink target bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsentLimits {
    /// More records than this is [`ContentObservation::Unbounded`].
    pub max_records: usize,
    /// More hashed bytes than this is [`ContentObservation::Unbounded`].
    pub max_hashed_bytes: u64,
}

impl ConsentLimits {
    /// The single production budget, used by the preview and the command alike.
    pub const PRODUCTION: Self = Self {
        max_records: 500_000,
        max_hashed_bytes: 256 * 1024 * 1024,
    };
}

impl Default for ConsentLimits {
    fn default() -> Self {
        Self::PRODUCTION
    }
}

// ============================================================================
// Observation
// ============================================================================

/// The identities of a fully bound observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedContent {
    /// `sha256:` over the staged, unstaged, unmerged, and untracked set.
    pub discardable_content_id: String,
    /// `sha256:` over the ignored set.
    pub ignored_content_id: String,
    /// True when the ignored set is non-empty.
    pub has_ignored_content: bool,
    /// Paths in the discardable identity.
    pub entries: usize,
    /// Paths in the ignored identity, walked directories included.
    pub ignored_entries: usize,
}

/// One run of the content oracle.
///
/// The non-observation variants carry no identity field at all, so a partial
/// or substitute identity cannot exist in the type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentObservation {
    /// Every path was resolved and bound within budget.
    Observed(ObservedContent),
    /// Status failed, a record was unparsable, or a path could not be read.
    Unobservable,
    /// The combined record or byte budget was exceeded.
    Unbounded,
    /// An actual `.git` descendant or a status-reported submodule.
    NestedRepository,
}

impl ContentObservation {
    /// The published classification.
    pub fn kind(&self) -> ContentObservationKind {
        match self {
            Self::Observed(_) => ContentObservationKind::Observed,
            Self::Unobservable => ContentObservationKind::StatusFailed,
            Self::Unbounded => ContentObservationKind::Unbounded,
            Self::NestedRepository => ContentObservationKind::NestedRepository,
        }
    }

    /// The bound identities, when there are any.
    pub fn observed(&self) -> Option<&ObservedContent> {
        match self {
            Self::Observed(content) => Some(content),
            _ => None,
        }
    }

    /// The refusal a non-observation always produces, whatever the permissions.
    pub fn refusal_reason(&self) -> Option<ConfirmedDeleteReason> {
        match self {
            Self::Observed(_) => None,
            Self::Unobservable => Some(ConfirmedDeleteReason::ContentUnobservable),
            Self::Unbounded => Some(ConfirmedDeleteReason::ContentUnbounded),
            Self::NestedRepository => Some(ConfirmedDeleteReason::NestedRepository),
        }
    }
}

/// Wire classification of a content observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ContentObservationKind {
    /// Both identities are bound.
    Observed,
    /// The observation could not be taken; both identities are `null`.
    StatusFailed,
    /// The combined budget was exceeded; both identities are `null`.
    Unbounded,
    /// An actual nested repository was found; both identities are `null`.
    NestedRepository,
}

/// The identity of the empty record set.
///
/// A clean worktree has this identity for both sets, never `null`. It is also
/// the one ignored identity teardown is allowed to leave behind.
pub fn empty_content_id() -> String {
    hash_records(&mut Vec::new())
}

/// True for a well-formed `sha256:<64 lowercase hex>` identity.
pub fn is_content_id(value: &str) -> bool {
    value
        .strip_prefix(CONTENT_ID_PREFIX)
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(is_lower_hex))
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// Observe a worktree's discardable and ignored content.
///
/// Synchronous on purpose: it is filesystem and subprocess work only, and the
/// async backend runs it on the blocking pool.
pub fn observe_discardable_content(worktree: &Path, limits: &ConsentLimits) -> ContentObservation {
    let output = std::process::Command::new("git")
        .args(read_only_status_argv(CONSENT_STATUS_ARGS))
        .current_dir(worktree)
        .stdin(std::process::Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => {
            observe_from_status(worktree, &output.stdout, limits)
        }
        _ => ContentObservation::Unobservable,
    }
}

/// Bind the records of an already-captured status output.
///
/// Separate from [`observe_discardable_content`] so a test can hold the status
/// fixed while it changes the filesystem underneath — the race the fd-relative
/// resolution exists for.
pub fn observe_from_status(
    worktree: &Path,
    status: &[u8],
    limits: &ConsentLimits,
) -> ContentObservation {
    let records = match parse_status(status) {
        Some(records) => records,
        None => return ContentObservation::Unobservable,
    };
    #[cfg(unix)]
    {
        match unix::Oracle::new(limits, &records).bind(worktree, &records) {
            Ok(bound) => ContentObservation::Observed(bound.finish()),
            Err(Stop::Unobservable) => ContentObservation::Unobservable,
            Err(Stop::Unbounded) => ContentObservation::Unbounded,
            Err(Stop::Nested) => ContentObservation::NestedRepository,
        }
    }
    #[cfg(not(unix))]
    {
        // No fd-relative no-follow resolution is available, so nothing can be
        // bound truthfully.
        let _ = (worktree, limits, records);
        ContentObservation::Unobservable
    }
}

/// The two record sets of a fully bound observation, before hashing.
#[cfg_attr(not(unix), allow(dead_code))]
struct Bound {
    discardable: Vec<(Vec<u8>, Vec<u8>)>,
    ignored: Vec<(Vec<u8>, Vec<u8>)>,
}

impl Bound {
    fn finish(mut self) -> ObservedContent {
        let entries = self.discardable.len();
        let ignored_entries = self.ignored.len();
        ObservedContent {
            discardable_content_id: hash_records(&mut self.discardable),
            ignored_content_id: hash_records(&mut self.ignored),
            has_ignored_content: ignored_entries > 0,
            entries,
            ignored_entries,
        }
    }
}

/// The bound records as text, for tests that assert what a leaf looked like.
#[cfg(all(test, unix))]
fn observe_records(worktree: &Path, limits: &ConsentLimits) -> Option<(Vec<String>, Vec<String>)> {
    let output = std::process::Command::new("git")
        .args(read_only_status_argv(CONSENT_STATUS_ARGS))
        .current_dir(worktree)
        .output()
        .ok()?;
    observe_records_from_status(worktree, &output.stdout, limits)
}

#[cfg(all(test, unix))]
fn observe_records_from_status(
    worktree: &Path,
    status: &[u8],
    limits: &ConsentLimits,
) -> Option<(Vec<String>, Vec<String>)> {
    let records = parse_status(status)?;
    let bound = unix::Oracle::new(limits, &records)
        .bind(worktree, &records)
        .ok()?;
    let render = |set: &Vec<(Vec<u8>, Vec<u8>)>| {
        let mut out: Vec<String> = set
            .iter()
            .map(|(path, fields)| {
                format!(
                    "{}|{}",
                    String::from_utf8_lossy(path),
                    String::from_utf8_lossy(fields).replace('\u{1f}', "|")
                )
            })
            .collect();
        out.sort();
        out
    };
    Some((render(&bound.discardable), render(&bound.ignored)))
}

/// Why the oracle stopped short of an identity.
#[cfg_attr(not(unix), allow(dead_code))]
enum Stop {
    Unobservable,
    Unbounded,
    Nested,
}

// ============================================================================
// Status parsing
// ============================================================================

/// One porcelain v2 record, verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StatusRecord {
    /// `1`, `2`, `u`, `?`, or `!`.
    kind: u8,
    /// The whole record line, original path joined with `\x1f` for renames.
    verbatim: Vec<u8>,
    /// Worktree-relative path; a collapsed directory keeps its trailing `/`.
    path: Vec<u8>,
    /// Source path of a rename or copy.
    original: Option<Vec<u8>>,
    /// True when the submodule field reports a submodule (`S...`).
    submodule: bool,
}

/// Parse `--porcelain=v2 -z` output; `None` for anything unrecognized.
fn parse_status(output: &[u8]) -> Option<Vec<StatusRecord>> {
    let mut tokens = output.split(|byte| *byte == 0);
    let mut records = Vec::new();
    while let Some(token) = tokens.next() {
        if token.is_empty() {
            continue;
        }
        let kind = token[0];
        // Field count before the path, for the kinds that carry fields.
        let fields_before_path = match kind {
            b'1' => 8,
            b'2' => 9,
            b'u' => 10,
            b'?' | b'!' => {
                let path = token.get(2..)?.to_vec();
                if token.get(1) != Some(&b' ') || path.is_empty() {
                    return None;
                }
                records.push(StatusRecord {
                    kind,
                    verbatim: token.to_vec(),
                    path,
                    original: None,
                    submodule: false,
                });
                continue;
            }
            // Headers only appear with `--branch`, which is never requested.
            b'#' => continue,
            _ => return None,
        };
        let parts: Vec<&[u8]> = token
            .splitn(fields_before_path + 1, |b| *b == b' ')
            .collect();
        if parts.len() != fields_before_path + 1 || parts[fields_before_path].is_empty() {
            return None;
        }
        let submodule = parts[2].first() == Some(&b'S');
        let path = parts[fields_before_path].to_vec();
        let mut verbatim = token.to_vec();
        let original = if kind == b'2' {
            let original = tokens.next()?.to_vec();
            if original.is_empty() {
                return None;
            }
            verbatim.push(0x1f);
            verbatim.extend_from_slice(&original);
            Some(original)
        } else {
            None
        };
        records.push(StatusRecord {
            kind,
            verbatim,
            path,
            original,
            submodule,
        });
    }
    Some(records)
}

// ============================================================================
// Hashing
// ============================================================================

/// Sort `(path, fields)` records and hash them into one identity.
fn hash_records(records: &mut [(Vec<u8>, Vec<u8>)]) -> String {
    records.sort();
    let mut hasher = Sha256::new();
    for (index, (path, fields)) in records.iter().enumerate() {
        if index > 0 {
            hasher.update([0x1e]);
        }
        hasher.update(path);
        hasher.update([0x1f]);
        hasher.update(fields);
    }
    format!("{CONTENT_ID_PREFIX}{}", hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

// ============================================================================
// fd-relative resolution
// ============================================================================

#[cfg(unix)]
// `mode_t` is `u32` on Linux and `u16` on macOS, so the `as u32` widenings are
// no-ops on one platform and required on the other.
#[allow(clippy::unnecessary_cast)]
mod unix {
    use std::ffi::{CStr, CString};
    use std::io::Read;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    use sha2::{Digest, Sha256};

    use super::{hex, Bound, ConsentLimits, HashSet, StatusRecord, Stop};

    type Step<T> = Result<T, Stop>;

    /// Deepest directory level the walk descends to before it is `Unbounded`.
    pub(super) const MAX_WALK_DEPTH: usize = 256;

    /// Which identity a record belongs to.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Set {
        Discardable,
        Ignored,
    }

    impl Set {
        /// Record prefix: nothing for discardable paths, `i:` for ignored ones.
        fn prefix(self) -> &'static [u8] {
            match self {
                Self::Discardable => b"",
                Self::Ignored => b"i:",
            }
        }
    }

    /// Accumulated state of one observation.
    pub(super) struct Oracle<'a> {
        limits: &'a ConsentLimits,
        records: usize,
        hashed_bytes: u64,
        discardable: Vec<(Vec<u8>, Vec<u8>)>,
        ignored: Vec<(Vec<u8>, Vec<u8>)>,
        /// Ignored paths (no trailing `/`) that a `?` directory walk must skip:
        /// Git reports an ignored entry inside an all-untracked directory as
        /// both `? dir/` and `! dir/file`, and the path is read once, through
        /// its own record.
        ignored_paths: HashSet<Vec<u8>>,
    }

    impl<'a> Oracle<'a> {
        pub(super) fn new(limits: &'a ConsentLimits, records: &[StatusRecord]) -> Self {
            let ignored_paths = records
                .iter()
                .filter(|record| record.kind == b'!')
                .map(|record| strip_slash(&record.path).to_vec())
                .collect();
            Self {
                limits,
                records: 0,
                hashed_bytes: 0,
                discardable: Vec::new(),
                ignored: Vec::new(),
                ignored_paths,
            }
        }

        pub(super) fn bind(mut self, worktree: &Path, records: &[StatusRecord]) -> Step<Bound> {
            let root = open_root(worktree).map_err(|_| Stop::Unobservable)?;
            for record in records {
                if record.submodule {
                    return Err(Stop::Nested);
                }
                match record.kind {
                    b'?' | b'!' => {
                        let set = if record.kind == b'!' {
                            Set::Ignored
                        } else {
                            Set::Discardable
                        };
                        self.count_record()?;
                        let walk = record.path.ends_with(b"/");
                        let (leaf, dir) = self.leaf_path(&root, &record.path, walk)?;
                        self.push(set, record.path.clone(), &record.verbatim, &leaf);
                        if let Some(dir) = dir {
                            self.walk(&dir, strip_slash(&record.path), set, 1)?;
                        }
                    }
                    _ => {
                        for path in std::iter::once(&record.path).chain(record.original.iter()) {
                            self.count_record()?;
                            let (leaf, _) = self.leaf_path(&root, path, false)?;
                            self.push(Set::Discardable, path.clone(), &record.verbatim, &leaf);
                        }
                    }
                }
            }
            Ok(Bound {
                discardable: self.discardable,
                ignored: self.ignored,
            })
        }

        fn count_record(&mut self) -> Step<()> {
            self.records += 1;
            if self.records > self.limits.max_records {
                return Err(Stop::Unbounded);
            }
            Ok(())
        }

        fn charge_bytes(&mut self, bytes: u64) -> Step<()> {
            self.hashed_bytes = self.hashed_bytes.saturating_add(bytes);
            if self.hashed_bytes > self.limits.max_hashed_bytes {
                return Err(Stop::Unbounded);
            }
            Ok(())
        }

        fn remaining_bytes(&self) -> u64 {
            self.limits
                .max_hashed_bytes
                .saturating_sub(self.hashed_bytes)
        }

        fn push(&mut self, set: Set, path: Vec<u8>, kind: &[u8], leaf: &[u8]) {
            let mut fields = kind.to_vec();
            fields.push(0x1f);
            fields.extend_from_slice(set.prefix());
            fields.extend_from_slice(leaf);
            match set {
                Set::Discardable => self.discardable.push((path, fields)),
                Set::Ignored => self.ignored.push((path, fields)),
            }
        }

        /// Resolve a status path component by component from the root.
        fn leaf_path(
            &mut self,
            root: &OwnedFd,
            path: &[u8],
            want_dir: bool,
        ) -> Step<(Vec<u8>, Option<OwnedFd>)> {
            let path = strip_slash(path);
            let mut components: Vec<&[u8]> = path.split(|b| *b == b'/').collect();
            let Some(name) = components.pop() else {
                return Err(Stop::Unobservable);
            };
            if name.is_empty() || components.iter().any(|c| c.is_empty()) {
                return Err(Stop::Unobservable);
            }
            if components.iter().chain([&name]).any(|c| *c == b".git") {
                return Err(Stop::Nested);
            }
            let mut parent: Option<OwnedFd> = None;
            for component in components {
                let dir = parent.as_ref().unwrap_or(root);
                match open_dir_at(dir, component) {
                    Ok(fd) => parent = Some(fd),
                    Err(error) => {
                        return match error.raw_os_error() {
                            Some(libc::ENOENT) => Ok((b"d".to_vec(), None)),
                            // A component that is no longer a directory — a
                            // symlink swapped in after the status call — is
                            // recorded, never followed.
                            Some(libc::ELOOP) | Some(libc::ENOTDIR) => {
                                Ok((b"o:parent".to_vec(), None))
                            }
                            _ => Err(Stop::Unobservable),
                        };
                    }
                }
            }
            let dir = parent.as_ref().unwrap_or(root);
            self.leaf_at(dir, name, want_dir)
        }

        /// The leaf identity of one directory entry, examined without following it.
        fn leaf_at(
            &mut self,
            dir: &OwnedFd,
            name: &[u8],
            want_dir: bool,
        ) -> Step<(Vec<u8>, Option<OwnedFd>)> {
            let stat = match lstat_at(dir, name) {
                Ok(stat) => stat,
                Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                    return Ok((b"d".to_vec(), None))
                }
                Err(_) => return Err(Stop::Unobservable),
            };
            match file_type(stat.st_mode as u32) {
                FileType::Regular => {
                    if stat.st_size as u64 > self.remaining_bytes() {
                        return Err(Stop::Unbounded);
                    }
                    let fd = match open_file_at(dir, name) {
                        Ok(fd) => fd,
                        Err(error) => {
                            return match error.raw_os_error() {
                                Some(libc::ELOOP) => Ok((b"o:symlink".to_vec(), None)),
                                Some(libc::ENXIO) => Ok((b"o:socket".to_vec(), None)),
                                Some(libc::ENOENT) => Ok((b"d".to_vec(), None)),
                                _ => Err(Stop::Unobservable),
                            }
                        }
                    };
                    // Closes the `fstatat` → `open` race: whatever was opened is
                    // what gets classified.
                    let opened = fstat(&fd).map_err(|_| Stop::Unobservable)?;
                    let kind = file_type(opened.st_mode as u32);
                    if kind != FileType::Regular {
                        return Ok((format!("o:{}", kind.name()).into_bytes(), None));
                    }
                    if opened.st_size as u64 > self.remaining_bytes() {
                        return Err(Stop::Unbounded);
                    }
                    let mut file = std::fs::File::from(fd);
                    let mut hasher = Sha256::new();
                    let mut buffer = vec![0u8; 64 * 1024];
                    loop {
                        let read = file.read(&mut buffer).map_err(|_| Stop::Unobservable)?;
                        if read == 0 {
                            break;
                        }
                        self.charge_bytes(read as u64)?;
                        hasher.update(&buffer[..read]);
                    }
                    Ok((
                        format!(
                            "f:{:o}:{}:{}:{}",
                            opened.st_mode as u32,
                            opened.st_size,
                            mtime_ns(&opened),
                            hex(&hasher.finalize())
                        )
                        .into_bytes(),
                        None,
                    ))
                }
                FileType::Symlink => {
                    let target = readlink_at(dir, name).map_err(|_| Stop::Unobservable)?;
                    self.charge_bytes(target.len() as u64)?;
                    Ok((
                        format!("l:{}", hex(&Sha256::digest(&target))).into_bytes(),
                        None,
                    ))
                }
                FileType::Directory => {
                    let leaf = format!("dir:{:o}", stat.st_mode as u32).into_bytes();
                    if !want_dir {
                        return Ok((leaf, None));
                    }
                    let fd = open_dir_at(dir, name).map_err(|_| Stop::Unobservable)?;
                    Ok((leaf, Some(fd)))
                }
                other => Ok((format!("o:{}", other.name()).into_bytes(), None)),
            }
        }

        /// Enumerate a collapsed directory record and everything beneath it.
        fn walk(&mut self, dir: &OwnedFd, prefix: &[u8], set: Set, depth: usize) -> Step<()> {
            // One descriptor and one stack frame per level: a tree deeper than
            // this is outside the budget, not a reason to exhaust the stack.
            if depth > MAX_WALK_DEPTH {
                return Err(Stop::Unbounded);
            }
            let mut names = read_dir_names(dir).map_err(|_| Stop::Unobservable)?;
            names.sort();
            for name in names {
                // Content below a `.git` entry cannot be enumerated from this
                // worktree's status and is not discarded on anyone's consent.
                if name == b".git" {
                    return Err(Stop::Nested);
                }
                let mut path = prefix.to_vec();
                path.push(b'/');
                path.extend_from_slice(&name);
                if set == Set::Discardable && self.ignored_paths.contains(&path) {
                    continue;
                }
                self.count_record()?;
                let (leaf, child) = self.leaf_at(dir, &name, true)?;
                self.push(set, path.clone(), b"w", &leaf);
                if let Some(child) = child {
                    self.walk(&child, &path, set, depth + 1)?;
                }
            }
            Ok(())
        }
    }

    fn strip_slash(path: &[u8]) -> &[u8] {
        path.strip_suffix(b"/").unwrap_or(path)
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum FileType {
        Regular,
        Directory,
        Symlink,
        Fifo,
        Socket,
        Char,
        Block,
        Unknown,
    }

    impl FileType {
        fn name(self) -> &'static str {
            match self {
                Self::Regular => "file",
                Self::Directory => "directory",
                Self::Symlink => "symlink",
                Self::Fifo => "fifo",
                Self::Socket => "socket",
                Self::Char => "char",
                Self::Block => "block",
                Self::Unknown => "unknown",
            }
        }
    }

    fn file_type(mode: u32) -> FileType {
        match mode & libc::S_IFMT as u32 {
            m if m == libc::S_IFREG as u32 => FileType::Regular,
            m if m == libc::S_IFDIR as u32 => FileType::Directory,
            m if m == libc::S_IFLNK as u32 => FileType::Symlink,
            m if m == libc::S_IFIFO as u32 => FileType::Fifo,
            m if m == libc::S_IFSOCK as u32 => FileType::Socket,
            m if m == libc::S_IFCHR as u32 => FileType::Char,
            m if m == libc::S_IFBLK as u32 => FileType::Block,
            _ => FileType::Unknown,
        }
    }

    fn mtime_ns(stat: &libc::stat) -> i128 {
        stat.st_mtime as i128 * 1_000_000_000 + stat.st_mtime_nsec as i128
    }

    fn cstring(name: &[u8]) -> std::io::Result<CString> {
        CString::new(name).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))
    }

    fn owned(fd: libc::c_int) -> std::io::Result<OwnedFd> {
        if fd < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            // SAFETY: `fd` was just returned by a successful open call and is
            // owned by nothing else.
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }

    fn open_root(path: &Path) -> std::io::Result<OwnedFd> {
        let path = cstring(path.as_os_str().as_bytes())?;
        // SAFETY: `path` is a valid NUL-terminated string.
        owned(unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })
    }

    fn open_dir_at(dir: &OwnedFd, name: &[u8]) -> std::io::Result<OwnedFd> {
        let name = cstring(name)?;
        // SAFETY: `dir` is an open descriptor and `name` is NUL-terminated.
        let fd = owned(unsafe {
            libc::openat(
                dir.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_DIRECTORY
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_CLOEXEC,
            )
        })?;
        let stat = fstat(&fd)?;
        if file_type(stat.st_mode as u32) != FileType::Directory {
            return Err(std::io::Error::from_raw_os_error(libc::ENOTDIR));
        }
        Ok(fd)
    }

    fn open_file_at(dir: &OwnedFd, name: &[u8]) -> std::io::Result<OwnedFd> {
        let name = cstring(name)?;
        // SAFETY: `dir` is an open descriptor and `name` is NUL-terminated.
        owned(unsafe {
            libc::openat(
                dir.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        })
    }

    fn lstat_at(dir: &OwnedFd, name: &[u8]) -> std::io::Result<libc::stat> {
        let name = cstring(name)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `stat` is valid writable storage for one `struct stat`.
        let result = unsafe {
            libc::fstatat(
                dir.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: a successful `fstatat` initialized the storage.
        Ok(unsafe { stat.assume_init() })
    }

    fn fstat(fd: &OwnedFd) -> std::io::Result<libc::stat> {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `stat` is valid writable storage for one `struct stat`.
        if unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: a successful `fstat` initialized the storage.
        Ok(unsafe { stat.assume_init() })
    }

    fn readlink_at(dir: &OwnedFd, name: &[u8]) -> std::io::Result<Vec<u8>> {
        let name = cstring(name)?;
        let mut buffer = vec![0u8; libc::PATH_MAX as usize + 1];
        // SAFETY: `buffer` is valid writable storage of the given length.
        let length = unsafe {
            libc::readlinkat(
                dir.as_raw_fd(),
                name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        if length < 0 {
            return Err(std::io::Error::last_os_error());
        }
        buffer.truncate(length as usize);
        Ok(buffer)
    }

    fn clear_errno() {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        // SAFETY: the thread-local errno location is always valid.
        unsafe {
            *libc::__errno_location() = 0;
        }
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
        // SAFETY: the thread-local errno location is always valid.
        unsafe {
            *libc::__error() = 0;
        }
    }

    /// Entry names of an open directory, without `.` and `..`.
    fn read_dir_names(dir: &OwnedFd) -> std::io::Result<Vec<Vec<u8>>> {
        // `fdopendir` takes ownership of the descriptor it is given, so it gets
        // a duplicate and the caller keeps its own.
        // SAFETY: `dir` is an open descriptor.
        let duplicate = owned(unsafe { libc::fcntl(dir.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) })?;
        // SAFETY: `duplicate` is an open directory descriptor; on success the
        // stream owns it and `closedir` closes it.
        let stream = unsafe { libc::fdopendir(duplicate.as_raw_fd()) };
        if stream.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        std::mem::forget(duplicate);
        let mut names = Vec::new();
        let result = loop {
            clear_errno();
            // SAFETY: `stream` is a valid open directory stream.
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                let error = std::io::Error::last_os_error();
                break match error.raw_os_error() {
                    Some(0) | None => Ok(()),
                    Some(_) => Err(error),
                };
            }
            // SAFETY: `readdir` returned a valid entry whose name is
            // NUL-terminated.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name != b"." && name != b".." {
                names.push(name.to_vec());
            }
        };
        // SAFETY: `stream` is valid and closed exactly once.
        unsafe { libc::closedir(stream) };
        result.map(|()| names)
    }
}

// ============================================================================
// Consent vocabulary
// ============================================================================

/// The facts a client consents to, echoed verbatim from a delete preview.
///
/// Exactly eight fields. `observed_at` is deliberately not one of them, so a
/// verbatim echo of the preview's `consent` object never trips
/// `deny_unknown_fields`. Every fact that a failed observation can leave
/// unknown is nullable, so the echo stays valid whatever the outcome was; the
/// command then refuses on the unknown fact itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
#[serde(deny_unknown_fields)]
pub struct DeleteConsent {
    /// Branch the worktree is on.
    pub branch: String,
    /// Full 40-hex HEAD commit.
    pub head: String,
    /// Uncommitted-change state (`--ignored=no`); `null` when unobservable.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub dirty: Option<bool>,
    /// Commits-ahead state; `null` when unobservable.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub has_commits_ahead: Option<bool>,
    /// Whether ignored content exists; `null` unless content was observed.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub has_ignored_content: Option<bool>,
    /// Full 40-hex merge base with the managed base; `null` when unresolved.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub merge_base: Option<String>,
    /// `sha256:` discardable content identity; `null` unless observed.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub discardable_content_id: Option<String>,
    /// `sha256:` ignored content identity; `null` unless observed.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub ignored_content_id: Option<String>,
}

/// The consent field names, in the order capabilities advertise them.
pub const CONSENT_FIELDS: [&str; 8] = [
    "branch",
    "head",
    "dirty",
    "has_commits_ahead",
    "has_ignored_content",
    "merge_base",
    "discardable_content_id",
    "ignored_content_id",
];

/// The three independent destructive permissions, as capabilities name them.
pub const CONFIRMED_DELETE_PERMISSIONS: [&str; 3] = [
    "skip_teardown",
    "discard_known_dirty",
    "discard_commits_ahead",
];

/// Every typed reason a confirmed delete or a preview option can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConfirmedDeleteReason {
    /// The worktree is on a different branch.
    BranchChanged,
    /// The path is occupied by a different Git worktree.
    IdentityChanged,
    /// HEAD (or the branch ref) moved.
    HeadMoved,
    /// The merge base with base moved.
    BaseMoved,
    /// The dirty fact differs from the consent.
    DirtyChanged,
    /// The commits-ahead fact differs from the consent.
    AheadChanged,
    /// The ignored-content fact differs from the consent.
    IgnoredChanged,
    /// A recomputed content identity differs from the consent.
    ContentChanged,
    /// The dirty state could not be observed.
    DirtyUnknown,
    /// The commits-ahead state could not be observed.
    AheadUnknown,
    /// The content observation could not be taken.
    ContentUnobservable,
    /// The content observation exceeded its combined budget.
    ContentUnbounded,
    /// An actual nested repository or a status-reported submodule.
    NestedRepository,
    /// The main worktree is never deletable.
    MainWorktree,
    /// The repository root is held by another operation or an unresolved merge.
    RootBusy,
    /// Commits ahead of base without `discard_commits_ahead`.
    CommitsAhead,
    /// Known uncommitted changes without `discard_known_dirty`.
    Dirty,
    /// Known ignored content without `discard_known_dirty`.
    IgnoredContent,
    /// `.wt/teardown` failed; the worktree was retained.
    TeardownFailed,
}

impl ConfirmedDeleteReason {
    /// Every reason, in the order capabilities advertise them.
    pub const ALL: [Self; 19] = [
        Self::BranchChanged,
        Self::IdentityChanged,
        Self::HeadMoved,
        Self::BaseMoved,
        Self::DirtyChanged,
        Self::AheadChanged,
        Self::IgnoredChanged,
        Self::ContentChanged,
        Self::DirtyUnknown,
        Self::AheadUnknown,
        Self::ContentUnobservable,
        Self::ContentUnbounded,
        Self::NestedRepository,
        Self::MainWorktree,
        Self::RootBusy,
        Self::CommitsAhead,
        Self::Dirty,
        Self::IgnoredContent,
        Self::TeardownFailed,
    ];

    /// Wire representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BranchChanged => "branch_changed",
            Self::IdentityChanged => "identity_changed",
            Self::HeadMoved => "head_moved",
            Self::BaseMoved => "base_moved",
            Self::DirtyChanged => "dirty_changed",
            Self::AheadChanged => "ahead_changed",
            Self::IgnoredChanged => "ignored_changed",
            Self::ContentChanged => "content_changed",
            Self::DirtyUnknown => "dirty_unknown",
            Self::AheadUnknown => "ahead_unknown",
            Self::ContentUnobservable => "content_unobservable",
            Self::ContentUnbounded => "content_unbounded",
            Self::NestedRepository => "nested_repository",
            Self::MainWorktree => "main_worktree",
            Self::RootBusy => "root_busy",
            Self::CommitsAhead => "commits_ahead",
            Self::Dirty => "dirty",
            Self::IgnoredContent => "ignored_content",
            Self::TeardownFailed => "teardown_failed",
        }
    }

    /// True for the reasons that mean "the consent no longer describes the target".
    pub fn is_stale(self) -> bool {
        matches!(
            self,
            Self::BranchChanged
                | Self::IdentityChanged
                | Self::HeadMoved
                | Self::BaseMoved
                | Self::DirtyChanged
                | Self::AheadChanged
                | Self::IgnoredChanged
                | Self::ContentChanged
        )
    }
}

/// What happened to `.wt/teardown` during a confirmed delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TeardownDisposition {
    /// The script ran (successfully, or it failed and the worktree was retained).
    Ran,
    /// `skip_teardown` was granted, so the script was not run.
    Skipped,
    /// No regular executable `.wt/teardown` exists, so there was nothing to run.
    Absent,
    /// The command refused before teardown was reached.
    NotReached,
}

/// The facts the service freshly observed, reported with every refusal.
///
/// A client can re-render its confirmation from this without a second preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct ConsentObservation {
    /// Branch at observation time.
    pub branch: String,
    /// HEAD at observation time.
    pub head: String,
    /// Dirty state; `null` when unobservable.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub dirty: Option<bool>,
    /// Commits-ahead state; `null` when unobservable.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub has_commits_ahead: Option<bool>,
    /// Ignored-content fact; `null` unless content was observed.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub has_ignored_content: Option<bool>,
    /// Merge base; `null` when unresolved.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub merge_base: Option<String>,
    /// Discardable identity; `null` unless content was observed.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub discardable_content_id: Option<String>,
    /// Ignored identity; `null` unless content was observed.
    #[cfg_attr(feature = "web-monitoring", schema(required))]
    pub ignored_content_id: Option<String>,
    /// How the content observation came out.
    pub content_observation: ContentObservationKind,
}

impl ConsentObservation {
    /// Project an observation onto the reported facts.
    pub fn new(
        facts: &crate::worktree_ops::service::WorktreeFacts,
        content: &ContentObservation,
        merge_base: Option<String>,
    ) -> Self {
        let observed = content.observed();
        Self {
            branch: facts.branch.clone(),
            head: facts.head.clone(),
            dirty: facts.dirty.as_option(),
            has_commits_ahead: fact_option(facts.has_commits_ahead),
            has_ignored_content: observed.map(|c| c.has_ignored_content),
            merge_base,
            discardable_content_id: observed.map(|c| c.discardable_content_id.clone()),
            ignored_content_id: observed.map(|c| c.ignored_content_id.clone()),
            content_observation: content.kind(),
        }
    }

    /// The consent object a client echoes to delete exactly this observation.
    pub fn consent(&self) -> DeleteConsent {
        DeleteConsent {
            branch: self.branch.clone(),
            head: self.head.clone(),
            dirty: self.dirty,
            has_commits_ahead: self.has_commits_ahead,
            has_ignored_content: self.has_ignored_content,
            merge_base: self.merge_base.clone(),
            discardable_content_id: self.discardable_content_id.clone(),
            ignored_content_id: self.ignored_content_id.clone(),
        }
    }
}

/// `Some` only for a confidently observed answer.
pub fn fact_option(fact: crate::worktree_ops::service::SafetyFact) -> Option<bool> {
    use crate::worktree_ops::service::SafetyFact;
    match fact {
        SafetyFact::Yes => Some(true),
        SafetyFact::No => Some(false),
        SafetyFact::Unknown => None,
    }
}

#[cfg(test)]
#[path = "consent_tests.rs"]
mod tests;
