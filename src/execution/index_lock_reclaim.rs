//! Same-dispatch reclamation of an orphaned managed-worktree `index.lock`.
//!
//! Forced Apply cleanup can kill a Git descendant in the window between
//! creating `index.lock` and committing or rolling it back. Process-group
//! quiescence is then true while filesystem convergence is false: the lock has
//! no surviving owner, so the WIP and final-commit retry policies — which wait
//! for contention to clear and never unlink — cannot repair it.
//!
//! This module owns the only deletion exception in Conflux, and it is
//! deliberately narrower than generic stale-lock cleanup. A lock may be
//! unlinked only when *this* process holds ephemeral evidence, captured before
//! *this* dispatch spawned, that the candidate was absent; only after that
//! dispatch's cleanup reported Unix [`ProcessGroupQuiescence::Confirmed`]; and
//! only when two observations separated by a fixed dwell agree that the
//! pathname is still the same regular, non-symlink, zero-byte file.
//!
//! Everything else fails closed. Provenance is never inferred from `lsof`, file
//! age, wall-clock timestamps, PID ownership, or process-group attribution, and
//! the pre-dispatch observation is process-local: it is consumed once, never
//! serialized, and a restarted process therefore has no reclamation authority
//! at all.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use crate::execution::index_lock::managed_worktree_lock_paths;
use crate::process_manager::ProcessGroupQuiescence;

/// Dwell between the two post-quiescence observations.
///
/// Long enough that a live Git process mid-transaction is overwhelmingly likely
/// to change size or mtime within it, short enough to stay inside the Apply
/// finalization barrier. It is only ever paid when residue actually exists.
pub(crate) const RECLAIM_DWELL: Duration = Duration::from_millis(500);

/// The file evidence one observation yields.
///
/// Deliberately a value type rather than a `Metadata` handle: the decision this
/// module makes must be verifiable without a filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LockFileFacts {
    pub(crate) is_symlink: bool,
    pub(crate) is_regular_file: bool,
    pub(crate) dev: u64,
    pub(crate) ino: u64,
    pub(crate) len: u64,
    /// Modification time as `(seconds, nanoseconds)`.
    pub(crate) mtime: (i64, i64),
}

impl LockFileFacts {
    /// Compact identity evidence for diagnostics.
    fn evidence(&self) -> String {
        format!(
            "dev={} ino={} size={} mtime={}.{:09}",
            self.dev, self.ino, self.len, self.mtime.0, self.mtime.1
        )
    }
}

/// Which of the two post-quiescence observations failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservationStage {
    /// The no-follow metadata read that opens the dwell.
    First,
    /// The no-follow descriptor read that closes it.
    Second,
}

impl ObservationStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Second => "second",
        }
    }
}

/// Filesystem and timing boundary the reclamation decision runs against.
///
/// Keeping metadata reads, the no-follow open, unlink, and the dwell behind one
/// trait is what lets every decision branch be proven with in-memory doubles and
/// no real waiting.
#[async_trait]
pub(crate) trait IndexLockReclaimEnvironment: Send + Sync {
    /// Resolve the `index.lock` paths that identify the managed worktree.
    async fn resolve_lock_paths(&self, workspace_path: &Path) -> Result<Vec<PathBuf>, String>;

    /// Metadata of the pathname itself, without following a final symlink.
    fn observe_link(&self, path: &Path) -> io::Result<LockFileFacts>;

    /// Open the pathname with `O_NOFOLLOW` and report descriptor metadata.
    fn observe_open_nofollow(&self, path: &Path) -> io::Result<LockFileFacts>;

    /// Remove the pathname.
    fn unlink(&self, path: &Path) -> io::Result<()>;

    /// Wait between the two observations.
    async fn dwell(&self, duration: Duration);
}

/// What one lock candidate looked like immediately before the Apply spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LockCandidateState {
    /// The candidate did not exist. Only this state can authorize reclamation.
    Absent,
    /// The candidate already existed, so this dispatch did not create it.
    Present { dev: u64, ino: u64 },
    /// The candidate could not be observed, so pre-absence is unproven.
    Unreadable(String),
}

impl LockCandidateState {
    /// Stable label for test assertions on captured candidate states.
    ///
    /// Candidate states are reported through the outcome they produce, never
    /// logged individually, so this exists only for tests.
    #[cfg(test)]
    fn as_str(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Present { .. } => "present",
            Self::Unreadable(_) => "unreadable",
        }
    }
}

/// One managed-worktree lock pathname and its pre-dispatch state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LockCandidate {
    pub(crate) path: PathBuf,
    pub(crate) state: LockCandidateState,
}

/// Process-lifetime evidence captured immediately before one Apply spawn.
///
/// Per `openspec/CONSTITUTION.md` this is ephemeral in-memory state for the
/// lifetime of a single owned command: it is never persisted, never influences
/// restart routing, and is consumed by value so one capture cannot authorize a
/// second dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreDispatchLockObservation {
    /// The workspace has no managed Git worktree, so no lock can be orphaned.
    NoManagedWorktree,
    /// Candidate pathnames were resolved and observed.
    Observed {
        workspace: PathBuf,
        candidates: Vec<LockCandidate>,
    },
    /// Candidate pathnames could not be resolved before the dispatch, so this
    /// dispatch has no reclamation authority at all.
    Unresolved { workspace: PathBuf, reason: String },
}

impl PreDispatchLockObservation {
    /// Observe every managed-worktree lock candidate before an Apply spawn.
    ///
    /// Resolution or observation failure never blocks the dispatch: it only
    /// removes this dispatch's authority to reclaim, so a lock that remains
    /// afterwards is refused instead of deleted.
    pub(crate) async fn capture(
        environment: &dyn IndexLockReclaimEnvironment,
        workspace_path: &Path,
        is_git: bool,
    ) -> Self {
        if !is_git {
            return Self::NoManagedWorktree;
        }

        let paths = match environment.resolve_lock_paths(workspace_path).await {
            Ok(paths) if paths.is_empty() => {
                return Self::Unresolved {
                    workspace: workspace_path.to_path_buf(),
                    reason: "the managed worktree reported no index.lock candidate".to_string(),
                }
            }
            Ok(paths) => paths,
            Err(reason) => {
                debug!(
                    workspace = %workspace_path.display(),
                    reason = %reason,
                    "Could not resolve managed worktree index.lock candidates before the Apply \
                     dispatch; post-quiescence reclamation is unavailable for it"
                );
                return Self::Unresolved {
                    workspace: workspace_path.to_path_buf(),
                    reason,
                };
            }
        };

        let candidates = paths
            .into_iter()
            .map(|path| {
                let state = match environment.observe_link(&path) {
                    Ok(facts) => LockCandidateState::Present {
                        dev: facts.dev,
                        ino: facts.ino,
                    },
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        LockCandidateState::Absent
                    }
                    Err(error) => LockCandidateState::Unreadable(error.to_string()),
                };
                LockCandidate { path, state }
            })
            .collect();

        Self::Observed {
            workspace: workspace_path.to_path_buf(),
            candidates,
        }
    }

    /// Stable label for test assertions on a capture's shape.
    ///
    /// The observation is consumed by the barrier and reported through the
    /// resulting outcome, so nothing outside tests labels it directly.
    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::NoManagedWorktree => "no_managed_worktree",
            Self::Observed { .. } => "observed",
            Self::Unresolved { .. } => "unresolved",
        }
    }
}

/// Why a remaining lock was not treated as a same-dispatch orphan.
///
/// Every variant is fail-closed: the pathname is left exactly as it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IndexLockReclaimRefusal {
    /// The candidate already existed before this dispatch spawned.
    PreExistingLock { path: PathBuf },
    /// Pre-dispatch absence was never proven for this candidate.
    MissingPreObservation { path: PathBuf, reason: String },
    /// The pathname is a symlink or another non-regular file.
    NotRegularFile { path: PathBuf, symlink: bool },
    /// The pathname carries content, so it is not an abandoned empty lock.
    NonZeroLength { path: PathBuf, len: u64 },
    /// Device, inode, size, or modification time changed during the dwell.
    IdentityChanged {
        path: PathBuf,
        first: LockFileFacts,
        second: LockFileFacts,
    },
    /// The pathname could not be observed.
    ObservationFailed {
        path: PathBuf,
        stage: ObservationStage,
        error: String,
    },
    /// This platform provides no reclamation evidence.
    Unsupported { path: PathBuf, reason: String },
    /// Unlink failed for a reason other than the file already being gone.
    UnlinkFailed { path: PathBuf, error: String },
}

impl IndexLockReclaimRefusal {
    /// Stable label for structured diagnostics.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::PreExistingLock { .. } => "pre_existing_lock",
            Self::MissingPreObservation { .. } => "missing_pre_observation",
            Self::NotRegularFile { .. } => "not_regular_file",
            Self::NonZeroLength { .. } => "non_zero_length",
            Self::IdentityChanged { .. } => "identity_changed",
            Self::ObservationFailed { .. } => "observation_failed",
            Self::Unsupported { .. } => "unsupported",
            Self::UnlinkFailed { .. } => "unlink_failed",
        }
    }

    /// The lock pathname this refusal is about.
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::PreExistingLock { path }
            | Self::MissingPreObservation { path, .. }
            | Self::NotRegularFile { path, .. }
            | Self::NonZeroLength { path, .. }
            | Self::IdentityChanged { path, .. }
            | Self::ObservationFailed { path, .. }
            | Self::Unsupported { path, .. }
            | Self::UnlinkFailed { path, .. } => path,
        }
    }

    /// The evidence condition that failed, without exposing file contents.
    pub(crate) fn evidence(&self) -> String {
        match self {
            Self::PreExistingLock { .. } => {
                "the lock already existed immediately before this Apply dispatch spawned"
                    .to_string()
            }
            Self::MissingPreObservation { reason, .. } => format!(
                "pre-dispatch absence was never proven for this lock ({})",
                reason
            ),
            Self::NotRegularFile { symlink, .. } => {
                if *symlink {
                    "the pathname is a symlink, not a regular lock file".to_string()
                } else {
                    "the pathname is not a regular file".to_string()
                }
            }
            Self::NonZeroLength { len, .. } => {
                format!("the lock is {} bytes rather than empty", len)
            }
            Self::IdentityChanged { first, second, .. } => format!(
                "the lock changed during the {}ms dwell (first {}; second {})",
                RECLAIM_DWELL.as_millis(),
                first.evidence(),
                second.evidence()
            ),
            Self::ObservationFailed { stage, error, .. } => format!(
                "the {} post-quiescence observation failed: {}",
                stage.as_str(),
                error
            ),
            Self::Unsupported { reason, .. } => {
                format!(
                    "this platform cannot produce reclamation evidence: {}",
                    reason
                )
            }
            Self::UnlinkFailed { error, .. } => {
                format!("removing the orphaned lock failed: {}", error)
            }
        }
    }
}

/// The result of one post-quiescence convergence check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IndexLockReclaimOutcome {
    /// Quiescence was not Unix-confirmed, so no lock was inspected at all.
    ///
    /// This adds no blocking authority of its own: the existing process-group
    /// cleanup diagnostics already govern whether finalization may continue.
    NotAuthorized { quiescence: &'static str },
    /// No managed-worktree lock residue remained after the dispatch.
    NotPresent,
    /// The orphaned lock was unlinked.
    Reclaimed { path: PathBuf },
    /// The lock disappeared on its own before it could be unlinked.
    NaturallyConverged { path: PathBuf },
    /// The residue is not a provable same-dispatch orphan.
    Refused(IndexLockReclaimRefusal),
}

impl IndexLockReclaimOutcome {
    /// Stable label for structured diagnostics.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::NotAuthorized { .. } => "not_authorized",
            Self::NotPresent => "not_present",
            Self::Reclaimed { .. } => "reclaimed",
            Self::NaturallyConverged { .. } => "naturally_converged",
            Self::Refused(refusal) => refusal.as_str(),
        }
    }

    /// The refusal, when this outcome is one.
    ///
    /// Production callers route on the outcome itself and render refusals with
    /// [`Self::diagnostics`]; only tests inspect the refusal's own evidence.
    #[cfg(test)]
    pub(crate) fn refusal(&self) -> Option<&IndexLockReclaimRefusal> {
        match self {
            Self::Refused(refusal) => Some(refusal),
            _ => None,
        }
    }

    /// Actionable one-line diagnostics for logs and operator-facing errors.
    pub(crate) fn diagnostics(&self) -> String {
        match self {
            Self::NotAuthorized { quiescence } => format!(
                "index.lock convergence not authorized: process-group quiescence is '{}', not 'confirmed'",
                quiescence
            ),
            Self::NotPresent => {
                "no managed worktree index.lock remained after the Apply dispatch".to_string()
            }
            Self::Reclaimed { path } => format!(
                "reclaimed the orphaned managed worktree lock '{}'",
                path.display()
            ),
            Self::NaturallyConverged { path } => format!(
                "the managed worktree lock '{}' disappeared on its own",
                path.display()
            ),
            Self::Refused(refusal) => format!(
                "refused to reclaim '{}' ({}): {}",
                refusal.path().display(),
                refusal.as_str(),
                refusal.evidence()
            ),
        }
    }
}

/// Whether the first observation describes a reclaimable empty lock.
///
/// Pure so every branch is provable without a filesystem.
pub(crate) fn classify_first_observation(
    path: &Path,
    facts: &LockFileFacts,
) -> Option<IndexLockReclaimRefusal> {
    if facts.is_symlink || !facts.is_regular_file {
        return Some(IndexLockReclaimRefusal::NotRegularFile {
            path: path.to_path_buf(),
            symlink: facts.is_symlink,
        });
    }
    if facts.len != 0 {
        return Some(IndexLockReclaimRefusal::NonZeroLength {
            path: path.to_path_buf(),
            len: facts.len,
        });
    }
    None
}

/// Whether the dwell preserved the exact file object the first observation saw.
pub(crate) fn classify_second_observation(
    path: &Path,
    first: &LockFileFacts,
    second: &LockFileFacts,
) -> Option<IndexLockReclaimRefusal> {
    let stable = second.is_regular_file
        && second.dev == first.dev
        && second.ino == first.ino
        && second.len == 0
        && second.mtime == first.mtime;

    if stable {
        None
    } else {
        Some(IndexLockReclaimRefusal::IdentityChanged {
            path: path.to_path_buf(),
            first: *first,
            second: *second,
        })
    }
}

/// Converge same-dispatch `index.lock` residue after confirmed quiescence.
///
/// The observation is taken by value: one pre-dispatch capture authorizes at
/// most one reclamation, and nothing survives to authorize a later dispatch or
/// a restarted process.
pub(crate) async fn reclaim_orphaned_index_lock(
    observation: PreDispatchLockObservation,
    quiescence: ProcessGroupQuiescence,
    environment: &dyn IndexLockReclaimEnvironment,
) -> IndexLockReclaimOutcome {
    // `NotApplicable` says verification does not apply, not that this Unix
    // process set was proven empty, so it is not deletion authority either.
    if quiescence != ProcessGroupQuiescence::Confirmed {
        return IndexLockReclaimOutcome::NotAuthorized {
            quiescence: quiescence.as_str(),
        };
    }

    let candidates = match observation {
        PreDispatchLockObservation::NoManagedWorktree => {
            return IndexLockReclaimOutcome::NotPresent
        }
        PreDispatchLockObservation::Observed { candidates, .. } => candidates,
        PreDispatchLockObservation::Unresolved { workspace, reason } => {
            // Reclamation is unavailable, but residue must still be refused
            // rather than silently finalized over. Resolving now only decides
            // *which pathname* to refuse; it can never restore authority.
            match environment.resolve_lock_paths(&workspace).await {
                Ok(paths) => paths
                    .into_iter()
                    .map(|path| LockCandidate {
                        path,
                        state: LockCandidateState::Unreadable(reason.clone()),
                    })
                    .collect(),
                // The workspace cannot even name its lock. There is no residue
                // this boundary can prove, and the repository gates below
                // report the real failure with better evidence than a guess.
                Err(_) => return IndexLockReclaimOutcome::NotPresent,
            }
        }
    };

    // Every candidate names the same lock through a different path prefix, so
    // the first one that still resolves is the residue.
    let mut residue: Option<(PathBuf, LockFileFacts)> = None;
    for candidate in &candidates {
        match environment.observe_link(&candidate.path) {
            Ok(facts) => {
                residue = Some((candidate.path.clone(), facts));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == io::ErrorKind::Unsupported => {
                return IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::Unsupported {
                    path: candidate.path.clone(),
                    reason: error.to_string(),
                })
            }
            Err(error) => {
                return IndexLockReclaimOutcome::Refused(
                    IndexLockReclaimRefusal::ObservationFailed {
                        path: candidate.path.clone(),
                        stage: ObservationStage::First,
                        error: error.to_string(),
                    },
                )
            }
        }
    }

    let Some((path, first)) = residue else {
        return IndexLockReclaimOutcome::NotPresent;
    };

    // Pre-existence is checked across *all* candidates, not just the one that
    // resolved: an alias prefix names the same file, so a lock visible through
    // any of them before the spawn is a lock this dispatch did not create.
    if let Some(candidate) = candidates
        .iter()
        .find(|candidate| matches!(candidate.state, LockCandidateState::Present { .. }))
    {
        return IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::PreExistingLock {
            path: candidate.path.clone(),
        });
    }
    if let Some((candidate, reason)) =
        candidates
            .iter()
            .find_map(|candidate| match &candidate.state {
                LockCandidateState::Unreadable(reason) => Some((candidate, reason.clone())),
                _ => None,
            })
    {
        return IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::MissingPreObservation {
            path: candidate.path.clone(),
            reason,
        });
    }

    if let Some(refusal) = classify_first_observation(&path, &first) {
        return IndexLockReclaimOutcome::Refused(refusal);
    }

    environment.dwell(RECLAIM_DWELL).await;

    let second = match environment.observe_open_nofollow(&path) {
        Ok(facts) => facts,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return IndexLockReclaimOutcome::NaturallyConverged { path }
        }
        Err(error) if error.kind() == io::ErrorKind::Unsupported => {
            return IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::Unsupported {
                path,
                reason: error.to_string(),
            })
        }
        Err(error) => {
            return IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::ObservationFailed {
                path,
                stage: ObservationStage::Second,
                error: error.to_string(),
            })
        }
    };

    if let Some(refusal) = classify_second_observation(&path, &first, &second) {
        return IndexLockReclaimOutcome::Refused(refusal);
    }

    match environment.unlink(&path) {
        Ok(()) => IndexLockReclaimOutcome::Reclaimed { path },
        // Another actor removed the same residue: the pathname converged, which
        // is exactly the state this boundary exists to reach.
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            IndexLockReclaimOutcome::NaturallyConverged { path }
        }
        Err(error) => IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::UnlinkFailed {
            path,
            error: error.to_string(),
        }),
    }
}

/// Production environment backed by the real filesystem and a real timer.
pub(crate) struct RealIndexLockReclaimEnvironment;

#[cfg(unix)]
fn facts_from_metadata(metadata: &std::fs::Metadata) -> LockFileFacts {
    use std::os::unix::fs::MetadataExt;

    LockFileFacts {
        is_symlink: metadata.file_type().is_symlink(),
        is_regular_file: metadata.file_type().is_file(),
        dev: metadata.dev(),
        ino: metadata.ino(),
        len: metadata.len(),
        mtime: (metadata.mtime(), metadata.mtime_nsec()),
    }
}

#[cfg(not(unix))]
fn unsupported_platform() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "index.lock reclamation requires Unix file identity metadata",
    )
}

#[async_trait]
impl IndexLockReclaimEnvironment for RealIndexLockReclaimEnvironment {
    async fn resolve_lock_paths(&self, workspace_path: &Path) -> Result<Vec<PathBuf>, String> {
        managed_worktree_lock_paths(workspace_path)
            .await
            .map_err(|error| error.to_string())
    }

    #[cfg(unix)]
    fn observe_link(&self, path: &Path) -> io::Result<LockFileFacts> {
        Ok(facts_from_metadata(&std::fs::symlink_metadata(path)?))
    }

    #[cfg(not(unix))]
    fn observe_link(&self, _path: &Path) -> io::Result<LockFileFacts> {
        Err(unsupported_platform())
    }

    #[cfg(unix)]
    fn observe_open_nofollow(&self, path: &Path) -> io::Result<LockFileFacts> {
        use std::os::unix::fs::OpenOptionsExt;

        // `O_NOFOLLOW` is what keeps the second observation from being read
        // through a symlink, and reading metadata from the descriptor rather
        // than the pathname is what binds it to an actual file object.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        Ok(facts_from_metadata(&file.metadata()?))
    }

    #[cfg(not(unix))]
    fn observe_open_nofollow(&self, _path: &Path) -> io::Result<LockFileFacts> {
        Err(unsupported_platform())
    }

    fn unlink(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    async fn dwell(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Test-only environments, shared by this module's own tests and by the Apply
/// finalization tests that consume its outcomes.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::Mutex;

    /// The real environment with the dwell replaced by a recorded no-op.
    ///
    /// Everything that touches the filesystem stays real; only waiting is
    /// faked, so a test observes genuine `lstat`/`O_NOFOLLOW`/`unlink`
    /// behaviour without spending half a second on every case.
    pub(crate) struct InstantDwellEnvironment {
        inner: RealIndexLockReclaimEnvironment,
        /// Runs once, between the two observations, to stage a concurrent
        /// mutation in exactly the window the two-point check polices.
        during_dwell: Mutex<Option<Box<dyn FnOnce() + Send>>>,
        dwells: Mutex<Vec<Duration>>,
    }

    impl InstantDwellEnvironment {
        pub(crate) fn new() -> Self {
            Self {
                inner: RealIndexLockReclaimEnvironment,
                during_dwell: Mutex::new(None),
                dwells: Mutex::new(Vec::new()),
            }
        }

        /// Run `action` between the first and second observation.
        pub(crate) fn mutating_during_dwell(action: impl FnOnce() + Send + 'static) -> Self {
            let environment = Self::new();
            *environment.during_dwell.lock().unwrap() = Some(Box::new(action));
            environment
        }

        /// Every dwell this environment was asked for, in order.
        pub(crate) fn dwells(&self) -> Vec<Duration> {
            self.dwells.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl IndexLockReclaimEnvironment for InstantDwellEnvironment {
        async fn resolve_lock_paths(&self, workspace_path: &Path) -> Result<Vec<PathBuf>, String> {
            self.inner.resolve_lock_paths(workspace_path).await
        }

        fn observe_link(&self, path: &Path) -> io::Result<LockFileFacts> {
            self.inner.observe_link(path)
        }

        fn observe_open_nofollow(&self, path: &Path) -> io::Result<LockFileFacts> {
            self.inner.observe_open_nofollow(path)
        }

        fn unlink(&self, path: &Path) -> io::Result<()> {
            self.inner.unlink(path)
        }

        async fn dwell(&self, duration: Duration) {
            self.dwells.lock().unwrap().push(duration);
            if let Some(action) = self.during_dwell.lock().unwrap().take() {
                action();
            }
        }
    }
}

#[cfg(test)]
#[path = "index_lock_reclaim_fs_tests.rs"]
mod fs_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    const LOCK: &str = "/repo/.git/worktrees/demo/index.lock";
    const ALIAS: &str = "/private/repo/.git/worktrees/demo/index.lock";

    fn empty_lock() -> LockFileFacts {
        LockFileFacts {
            is_symlink: false,
            is_regular_file: true,
            dev: 7,
            ino: 42,
            len: 0,
            mtime: (1_700_000_000, 123),
        }
    }

    /// An in-memory filesystem whose per-path answers are scripted per call, so
    /// every decision branch — including a file that changes between the two
    /// observations — is deterministic and needs no real dwell.
    #[derive(Default)]
    struct FakeEnvironment {
        link: Mutex<HashMap<PathBuf, Vec<io::Result<LockFileFacts>>>>,
        open: Mutex<HashMap<PathBuf, Vec<io::Result<LockFileFacts>>>>,
        unlink: Mutex<HashMap<PathBuf, io::Result<()>>>,
        resolved: Mutex<Option<Result<Vec<PathBuf>, String>>>,
        dwells: Mutex<Vec<Duration>>,
        unlinked: Mutex<Vec<PathBuf>>,
    }

    fn clone_result(result: &io::Result<LockFileFacts>) -> io::Result<LockFileFacts> {
        match result {
            Ok(facts) => Ok(*facts),
            Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
        }
    }

    impl FakeEnvironment {
        fn with_link(self, path: &str, answers: Vec<io::Result<LockFileFacts>>) -> Self {
            self.link
                .lock()
                .unwrap()
                .insert(PathBuf::from(path), answers);
            self
        }

        fn with_open(self, path: &str, answers: Vec<io::Result<LockFileFacts>>) -> Self {
            self.open
                .lock()
                .unwrap()
                .insert(PathBuf::from(path), answers);
            self
        }

        fn with_unlink(self, path: &str, answer: io::Result<()>) -> Self {
            self.unlink
                .lock()
                .unwrap()
                .insert(PathBuf::from(path), answer);
            self
        }

        fn with_resolved(self, answer: Result<Vec<PathBuf>, String>) -> Self {
            *self.resolved.lock().unwrap() = Some(answer);
            self
        }

        fn dwells(&self) -> Vec<Duration> {
            self.dwells.lock().unwrap().clone()
        }

        fn unlinked(&self) -> Vec<PathBuf> {
            self.unlinked.lock().unwrap().clone()
        }

        fn next(
            map: &Mutex<HashMap<PathBuf, Vec<io::Result<LockFileFacts>>>>,
            path: &Path,
        ) -> io::Result<LockFileFacts> {
            let mut map = map.lock().unwrap();
            match map.get_mut(path) {
                Some(answers) if !answers.is_empty() => {
                    if answers.len() == 1 {
                        clone_result(&answers[0])
                    } else {
                        answers.remove(0)
                    }
                }
                _ => Err(io::Error::new(io::ErrorKind::NotFound, "absent")),
            }
        }
    }

    #[async_trait]
    impl IndexLockReclaimEnvironment for FakeEnvironment {
        async fn resolve_lock_paths(&self, _workspace: &Path) -> Result<Vec<PathBuf>, String> {
            match self.resolved.lock().unwrap().as_ref() {
                Some(Ok(paths)) => Ok(paths.clone()),
                Some(Err(reason)) => Err(reason.clone()),
                None => Ok(vec![PathBuf::from(LOCK)]),
            }
        }

        fn observe_link(&self, path: &Path) -> io::Result<LockFileFacts> {
            Self::next(&self.link, path)
        }

        fn observe_open_nofollow(&self, path: &Path) -> io::Result<LockFileFacts> {
            Self::next(&self.open, path)
        }

        fn unlink(&self, path: &Path) -> io::Result<()> {
            self.unlinked.lock().unwrap().push(path.to_path_buf());
            match self.unlink.lock().unwrap().remove(path) {
                Some(answer) => answer,
                None => Ok(()),
            }
        }

        async fn dwell(&self, duration: Duration) {
            self.dwells.lock().unwrap().push(duration);
        }
    }

    fn absent_before(paths: &[&str]) -> PreDispatchLockObservation {
        PreDispatchLockObservation::Observed {
            workspace: PathBuf::from("/repo"),
            candidates: paths
                .iter()
                .map(|path| LockCandidate {
                    path: PathBuf::from(path),
                    state: LockCandidateState::Absent,
                })
                .collect(),
        }
    }

    // ---- pre-dispatch observation ---------------------------------------

    #[tokio::test]
    async fn a_non_git_workspace_has_no_managed_lock_candidates() {
        let environment = FakeEnvironment::default();

        let observation =
            PreDispatchLockObservation::capture(&environment, Path::new("/repo"), false).await;

        assert_eq!(observation, PreDispatchLockObservation::NoManagedWorktree);
    }

    #[tokio::test]
    async fn pre_dispatch_capture_distinguishes_absent_present_and_unreadable_candidates() {
        let environment = FakeEnvironment::default()
            .with_resolved(Ok(vec![PathBuf::from(LOCK), PathBuf::from(ALIAS)]))
            .with_link(LOCK, vec![Ok(empty_lock())])
            .with_link(
                ALIAS,
                vec![Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "denied",
                ))],
            );

        let observation =
            PreDispatchLockObservation::capture(&environment, Path::new("/repo"), true).await;

        let PreDispatchLockObservation::Observed { candidates, .. } = observation else {
            panic!("expected observed candidates");
        };
        assert_eq!(
            candidates[0].state,
            LockCandidateState::Present { dev: 7, ino: 42 }
        );
        assert_eq!(candidates[1].state.as_str(), "unreadable");
    }

    #[tokio::test]
    async fn unresolvable_candidates_leave_the_dispatch_without_reclamation_authority() {
        let environment =
            FakeEnvironment::default().with_resolved(Err("not a git repository".to_string()));

        let observation =
            PreDispatchLockObservation::capture(&environment, Path::new("/repo"), true).await;

        assert!(matches!(
            observation,
            PreDispatchLockObservation::Unresolved { .. }
        ));
    }

    #[tokio::test]
    async fn an_empty_candidate_list_is_unresolved_rather_than_authority() {
        let environment = FakeEnvironment::default().with_resolved(Ok(Vec::new()));

        let observation =
            PreDispatchLockObservation::capture(&environment, Path::new("/repo"), true).await;

        assert!(matches!(
            observation,
            PreDispatchLockObservation::Unresolved { .. }
        ));
    }

    // ---- authorization ---------------------------------------------------

    #[tokio::test]
    async fn unconfirmed_quiescence_never_inspects_the_lock() {
        for quiescence in [
            ProcessGroupQuiescence::NotApplicable,
            ProcessGroupQuiescence::MembersRemain,
            ProcessGroupQuiescence::Unverifiable,
        ] {
            let environment = FakeEnvironment::default().with_link(LOCK, vec![Ok(empty_lock())]);

            let outcome =
                reclaim_orphaned_index_lock(absent_before(&[LOCK]), quiescence, &environment).await;

            assert_eq!(
                outcome,
                IndexLockReclaimOutcome::NotAuthorized {
                    quiescence: quiescence.as_str()
                }
            );
            assert!(environment.unlinked().is_empty());
            assert!(environment.dwells().is_empty());
        }
    }

    #[tokio::test]
    async fn a_workspace_without_a_managed_worktree_converges_trivially() {
        let environment = FakeEnvironment::default();

        let outcome = reclaim_orphaned_index_lock(
            PreDispatchLockObservation::NoManagedWorktree,
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome, IndexLockReclaimOutcome::NotPresent);
    }

    // ---- reclamation and natural convergence -----------------------------

    #[tokio::test]
    async fn a_same_dispatch_orphan_is_reclaimed_after_a_stable_dwell() {
        let environment = FakeEnvironment::default()
            .with_link(LOCK, vec![Ok(empty_lock())])
            .with_open(LOCK, vec![Ok(empty_lock())]);

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(
            outcome,
            IndexLockReclaimOutcome::Reclaimed {
                path: PathBuf::from(LOCK)
            }
        );
        assert_eq!(outcome.as_str(), "reclaimed");
        assert_eq!(environment.dwells(), vec![RECLAIM_DWELL]);
        assert_eq!(environment.unlinked(), vec![PathBuf::from(LOCK)]);
    }

    #[tokio::test]
    async fn no_residue_needs_no_dwell_and_no_unlink() {
        let environment = FakeEnvironment::default();

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK, ALIAS]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome, IndexLockReclaimOutcome::NotPresent);
        assert!(environment.dwells().is_empty());
        assert!(environment.unlinked().is_empty());
    }

    #[tokio::test]
    async fn a_lock_that_disappears_during_the_dwell_is_natural_convergence() {
        let environment = FakeEnvironment::default().with_link(LOCK, vec![Ok(empty_lock())]);
        // No scripted open answer: the fake reports the pathname as absent.

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(
            outcome,
            IndexLockReclaimOutcome::NaturallyConverged {
                path: PathBuf::from(LOCK)
            }
        );
        assert!(environment.unlinked().is_empty());
    }

    #[tokio::test]
    async fn an_enoent_unlink_is_natural_convergence_rather_than_failure() {
        let environment = FakeEnvironment::default()
            .with_link(LOCK, vec![Ok(empty_lock())])
            .with_open(LOCK, vec![Ok(empty_lock())])
            .with_unlink(LOCK, Err(io::Error::new(io::ErrorKind::NotFound, "gone")));

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "naturally_converged");
    }

    // ---- refusals --------------------------------------------------------

    #[tokio::test]
    async fn a_pre_existing_lock_is_never_reclaimed_by_this_dispatch() {
        let environment = FakeEnvironment::default().with_link(LOCK, vec![Ok(empty_lock())]);
        let observation = PreDispatchLockObservation::Observed {
            workspace: PathBuf::from("/repo"),
            candidates: vec![LockCandidate {
                path: PathBuf::from(LOCK),
                state: LockCandidateState::Present { dev: 7, ino: 42 },
            }],
        };

        let outcome = reclaim_orphaned_index_lock(
            observation,
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "pre_existing_lock");
        assert!(environment.unlinked().is_empty());
        assert!(environment.dwells().is_empty());
    }

    #[tokio::test]
    async fn a_lock_pre_existing_through_a_path_alias_is_never_reclaimed() {
        // The residue resolves through the canonical path, but the same file
        // was already visible through its alias before the spawn.
        let environment = FakeEnvironment::default().with_link(LOCK, vec![Ok(empty_lock())]);
        let observation = PreDispatchLockObservation::Observed {
            workspace: PathBuf::from("/repo"),
            candidates: vec![
                LockCandidate {
                    path: PathBuf::from(LOCK),
                    state: LockCandidateState::Absent,
                },
                LockCandidate {
                    path: PathBuf::from(ALIAS),
                    state: LockCandidateState::Present { dev: 7, ino: 42 },
                },
            ],
        };

        let outcome = reclaim_orphaned_index_lock(
            observation,
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "pre_existing_lock");
        assert_eq!(
            outcome
                .refusal()
                .map(|refusal| refusal.path().to_path_buf()),
            Some(PathBuf::from(ALIAS))
        );
        assert!(environment.unlinked().is_empty());
    }

    #[tokio::test]
    async fn an_unobserved_candidate_is_refused_rather_than_reclaimed() {
        let environment = FakeEnvironment::default().with_link(LOCK, vec![Ok(empty_lock())]);
        let observation = PreDispatchLockObservation::Observed {
            workspace: PathBuf::from("/repo"),
            candidates: vec![LockCandidate {
                path: PathBuf::from(LOCK),
                state: LockCandidateState::Unreadable("denied".to_string()),
            }],
        };

        let outcome = reclaim_orphaned_index_lock(
            observation,
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "missing_pre_observation");
        assert!(environment.unlinked().is_empty());
    }

    #[tokio::test]
    async fn an_unresolved_dispatch_refuses_residue_it_can_still_name() {
        let environment = FakeEnvironment::default()
            .with_resolved(Ok(vec![PathBuf::from(LOCK)]))
            .with_link(LOCK, vec![Ok(empty_lock())]);

        let outcome = reclaim_orphaned_index_lock(
            PreDispatchLockObservation::Unresolved {
                workspace: PathBuf::from("/repo"),
                reason: "not a git repository".to_string(),
            },
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "missing_pre_observation");
        assert!(environment.unlinked().is_empty());
    }

    #[tokio::test]
    async fn an_unresolvable_workspace_claims_no_residue_at_all() {
        let environment = FakeEnvironment::default().with_resolved(Err("no git dir".to_string()));

        let outcome = reclaim_orphaned_index_lock(
            PreDispatchLockObservation::Unresolved {
                workspace: PathBuf::from("/repo"),
                reason: "no git dir".to_string(),
            },
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome, IndexLockReclaimOutcome::NotPresent);
    }

    #[tokio::test]
    async fn a_symlink_or_non_regular_pathname_is_refused() {
        let symlink = LockFileFacts {
            is_symlink: true,
            is_regular_file: false,
            ..empty_lock()
        };
        let environment = FakeEnvironment::default().with_link(LOCK, vec![Ok(symlink)]);

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "not_regular_file");
        assert!(outcome.diagnostics().contains("symlink"));
        assert!(environment.unlinked().is_empty());
        assert!(environment.dwells().is_empty());
    }

    #[tokio::test]
    async fn a_non_zero_lock_is_refused_because_it_carries_content() {
        let written = LockFileFacts {
            len: 137,
            ..empty_lock()
        };
        let environment = FakeEnvironment::default().with_link(LOCK, vec![Ok(written)]);

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "non_zero_length");
        assert!(outcome.diagnostics().contains("137 bytes"));
        assert!(environment.unlinked().is_empty());
    }

    #[tokio::test]
    async fn an_inode_replacement_during_the_dwell_is_refused() {
        let replaced = LockFileFacts {
            ino: 43,
            ..empty_lock()
        };
        let environment = FakeEnvironment::default()
            .with_link(LOCK, vec![Ok(empty_lock())])
            .with_open(LOCK, vec![Ok(replaced)]);

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "identity_changed");
        assert!(environment.unlinked().is_empty());
    }

    #[tokio::test]
    async fn size_or_mtime_movement_during_the_dwell_is_refused() {
        for changed in [
            LockFileFacts {
                len: 12,
                ..empty_lock()
            },
            LockFileFacts {
                mtime: (1_700_000_001, 0),
                ..empty_lock()
            },
        ] {
            let environment = FakeEnvironment::default()
                .with_link(LOCK, vec![Ok(empty_lock())])
                .with_open(LOCK, vec![Ok(changed)]);

            let outcome = reclaim_orphaned_index_lock(
                absent_before(&[LOCK]),
                ProcessGroupQuiescence::Confirmed,
                &environment,
            )
            .await;

            assert_eq!(outcome.as_str(), "identity_changed");
            assert!(environment.unlinked().is_empty());
        }
    }

    #[tokio::test]
    async fn a_metadata_failure_on_either_observation_is_refused() {
        let first_failed = FakeEnvironment::default().with_link(
            LOCK,
            vec![Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "denied",
            ))],
        );
        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &first_failed,
        )
        .await;
        assert_eq!(outcome.as_str(), "observation_failed");
        assert!(outcome.diagnostics().contains("first"));

        let second_failed = FakeEnvironment::default()
            .with_link(LOCK, vec![Ok(empty_lock())])
            .with_open(
                LOCK,
                vec![Err(io::Error::other("too many levels of symbolic links"))],
            );
        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &second_failed,
        )
        .await;
        assert_eq!(outcome.as_str(), "observation_failed");
        assert!(outcome.diagnostics().contains("second"));
        assert!(second_failed.unlinked().is_empty());
    }

    #[tokio::test]
    async fn an_unsupported_platform_is_refused_rather_than_reclaimed() {
        let environment = FakeEnvironment::default().with_link(
            LOCK,
            vec![Err(io::Error::new(io::ErrorKind::Unsupported, "no fstat"))],
        );

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "unsupported");
        assert!(environment.unlinked().is_empty());
    }

    #[tokio::test]
    async fn a_failed_unlink_is_refused_rather_than_reported_as_converged() {
        let environment = FakeEnvironment::default()
            .with_link(LOCK, vec![Ok(empty_lock())])
            .with_open(LOCK, vec![Ok(empty_lock())])
            .with_unlink(
                LOCK,
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied")),
            );

        let outcome = reclaim_orphaned_index_lock(
            absent_before(&[LOCK]),
            ProcessGroupQuiescence::Confirmed,
            &environment,
        )
        .await;

        assert_eq!(outcome.as_str(), "unlink_failed");
        assert!(outcome.diagnostics().contains("denied"));
    }

    // ---- diagnostics -----------------------------------------------------

    /// Every outcome this boundary can produce, so the diagnostics assertions
    /// below cover the whole surface rather than a sample of it.
    fn every_outcome() -> Vec<IndexLockReclaimOutcome> {
        let path = PathBuf::from(LOCK);
        vec![
            IndexLockReclaimOutcome::NotAuthorized {
                quiescence: "not_applicable",
            },
            IndexLockReclaimOutcome::NotPresent,
            IndexLockReclaimOutcome::Reclaimed { path: path.clone() },
            IndexLockReclaimOutcome::NaturallyConverged { path: path.clone() },
            IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::PreExistingLock {
                path: path.clone(),
            }),
            IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::MissingPreObservation {
                path: path.clone(),
                reason: "permission denied".to_string(),
            }),
            IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::NotRegularFile {
                path: path.clone(),
                symlink: true,
            }),
            IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::NonZeroLength {
                path: path.clone(),
                len: 137,
            }),
            IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::IdentityChanged {
                path: path.clone(),
                first: empty_lock(),
                second: LockFileFacts {
                    ino: 43,
                    ..empty_lock()
                },
            }),
            IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::ObservationFailed {
                path: path.clone(),
                stage: ObservationStage::Second,
                error: "too many levels of symbolic links".to_string(),
            }),
            IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::Unsupported {
                path: path.clone(),
                reason: "no fstat".to_string(),
            }),
            IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::UnlinkFailed {
                path,
                error: "denied".to_string(),
            }),
        ]
    }

    /// An operator reading a log has to be able to tell "we removed an orphan"
    /// from "it vanished on its own" from each distinct reason we refused. A
    /// collision would make two materially different repository situations read
    /// identically.
    #[test]
    fn every_outcome_carries_a_distinct_diagnostic_label() {
        let outcomes = every_outcome();
        let labels = outcomes
            .iter()
            .map(|outcome| outcome.as_str())
            .collect::<Vec<_>>();

        for expected in [
            "reclaimed",
            "naturally_converged",
            "pre_existing_lock",
            "identity_changed",
            "unsupported",
            "unlink_failed",
        ] {
            assert!(
                labels.contains(&expected),
                "the {expected:?} outcome must be nameable: {labels:?}"
            );
        }

        let mut unique = labels.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), labels.len(), "labels collided: {:?}", labels);
    }

    /// The prose has to be distinguishable too, not just the token: the label
    /// goes to structured fields, the prose is what an operator actually reads.
    #[test]
    fn every_outcome_carries_distinct_diagnostics_naming_its_lock() {
        let outcomes = every_outcome();
        let diagnostics = outcomes
            .iter()
            .map(|outcome| outcome.diagnostics())
            .collect::<Vec<_>>();

        for (outcome, rendered) in outcomes.iter().zip(&diagnostics) {
            // The two outcomes that describe no particular file are the only
            // ones without a path to report.
            if !matches!(
                outcome,
                IndexLockReclaimOutcome::NotPresent | IndexLockReclaimOutcome::NotAuthorized { .. }
            ) {
                assert!(
                    rendered.contains(LOCK),
                    "{} must name the lock it is about: {rendered}",
                    outcome.as_str()
                );
            }
        }

        let mut unique = diagnostics.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            diagnostics.len(),
            "diagnostics collided: {:?}",
            diagnostics
        );
    }

    #[test]
    fn identity_diagnostics_report_device_inode_size_and_mtime_without_contents() {
        let outcome = IndexLockReclaimOutcome::Refused(IndexLockReclaimRefusal::IdentityChanged {
            path: PathBuf::from(LOCK),
            first: empty_lock(),
            second: LockFileFacts {
                ino: 43,
                len: 5,
                ..empty_lock()
            },
        });

        let diagnostics = outcome.diagnostics();
        assert!(diagnostics.contains("dev=7"), "{}", diagnostics);
        assert!(diagnostics.contains("ino=42"), "{}", diagnostics);
        assert!(diagnostics.contains("ino=43"), "{}", diagnostics);
        assert!(diagnostics.contains("size=5"), "{}", diagnostics);
        assert!(
            diagnostics.contains("mtime=1700000000.000000123"),
            "{}",
            diagnostics
        );
        assert!(diagnostics.contains(LOCK), "{}", diagnostics);
    }
}
