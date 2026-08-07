//! Which worktrees a refresh may spend Git commands on, and the process-local
//! cache that keeps an unchanged answer from being recomputed.
//!
//! Listing worktrees is cheap; deciding whether each one conflicts with base is
//! not. `git merge-tree` costs what the conflict surface costs, and a stale
//! worktree — still registered in Git, no longer backing a live change — can
//! carry the largest surface in the repository while being the least
//! interesting row on screen. Periodic refresh therefore classifies inspection
//! eligibility from current repository evidence *before* it spawns anything,
//! and an ineligible worktree stays listed with an explicit
//! [`InspectionState::NotInspected`] rather than being hidden or, worse,
//! presented as conflict-free.
//!
//! Everything here is presentation-side observability. The cache lives for one
//! process, is keyed only by Git-derived identity and revisions, and is never
//! read by scheduler dispatch, resume routing, acceptance, archive, merge
//! execution, or next-action selection: discarding it changes no decision,
//! which is what `openspec/CONSTITUTION.md` requires of anything outside the
//! workspace.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
#[cfg(feature = "web-monitoring")]
use utoipa::ToSchema;

use super::same_path;
use crate::worktree_ops::service::SafetyFact;

/// The bounded merge-simulation surface this layer spends its Git commands on.
///
/// Re-exported here because `vcs` is crate-internal while `worktree_ops` is the
/// public observation boundary: proving that a refresh's diagnostics stay
/// bounded should not require reaching through a private module.
#[allow(unused_imports)] // The binary target compiles this tree privately.
pub use crate::vcs::git::commands::{
    check_merge_conflicts, MergeSimulation, MAX_CONFLICT_SAMPLE, MAX_OUTPUT_PREFIX_BYTES,
};

/// How a worktree's ahead/conflict facts were obtained.
///
/// The distinction is not cosmetic: "no conflicts were found" and "nobody
/// looked" are the same empty conflict list, and a merge affordance that cannot
/// tell them apart will offer a merge on the strength of a check that never
/// ran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum InspectionState {
    /// Ahead/conflict commands ran for this worktree during this observation.
    Checked,
    /// An unchanged earlier observation was reused; no command ran.
    Reused,
    /// Nothing was inspected, so both facts are unknown.
    #[default]
    NotInspected,
}

impl InspectionState {
    /// True when ahead/conflict facts rest on an actual merge simulation.
    pub fn is_inspected(self) -> bool {
        !matches!(self, Self::NotInspected)
    }

    /// Operator-facing badge. Empty for the ordinary freshly checked row, which
    /// is what every other indicator is already relative to.
    pub fn label(self) -> &'static str {
        match self {
            Self::Checked => "",
            Self::Reused => "cached",
            Self::NotInspected => "not inspected",
        }
    }
}

/// Why an observation is being taken.
///
/// This is the caller's *intent*, not a resolved policy: the eligible change
/// identities are derived from the repository by [`InspectionScope::resolve`],
/// so no frontend can widen or narrow the policy by passing a different set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationRequest {
    /// Periodic automatic refresh — the TUI worktree refresh and the Web/UDS
    /// repository refresh both use this, and therefore one shared policy.
    Periodic,
    /// Structure only: paths, branches, HEADs. No ahead/conflict inspection.
    Listing,
    /// One operator-addressed worktree, inspected fresh regardless of whether
    /// periodic refresh would have skipped it.
    Target(PathBuf),
}

/// The resolved policy a single observation runs under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectionScope {
    /// Inspect only worktrees whose branch identity is in this set.
    Periodic {
        /// Branch identities of currently active or rejected changes.
        eligible_branches: BTreeSet<String>,
    },
    /// Inspect nothing.
    Listing,
    /// Inspect exactly this path, bypassing any cached answer.
    Target(PathBuf),
}

/// What a scope permits for one candidate worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Inspect now, ignoring any cached observation.
    Fresh,
    /// Inspect only on a cache miss.
    Cacheable,
    /// Spawn nothing.
    Skip,
}

/// The facts an eligibility decision is allowed to use.
///
/// Deliberately only Git-derived structure: eligibility must be recomputable
/// from the repository alone, so nothing here comes from a previous refresh.
#[derive(Debug, Clone, Copy)]
pub struct InspectionCandidate<'a> {
    /// Absolute worktree path as Git reports it.
    pub path: &'a Path,
    /// Checked-out branch, empty when detached.
    pub branch: &'a str,
    /// True for the repository's main worktree.
    pub is_main: bool,
    /// True when the worktree is detached or holds no branch at all.
    pub has_no_branch_identity: bool,
}

impl<'a> InspectionCandidate<'a> {
    /// Build a candidate from an observed worktree row.
    pub fn new(path: &'a Path, branch: &'a str, is_main: bool, is_detached: bool) -> Self {
        Self {
            path,
            branch,
            is_main,
            has_no_branch_identity: is_detached || branch.is_empty(),
        }
    }
}

impl InspectionScope {
    /// Resolve a caller's intent against the current repository.
    ///
    /// The eligible set is read from the workspace change tree every time
    /// rather than carried between refreshes: a change that was archived or
    /// rejected a second ago must stop being inspected on the very next pass.
    pub fn resolve(repo_root: &Path, request: &ObservationRequest) -> Self {
        match request {
            ObservationRequest::Listing => Self::Listing,
            ObservationRequest::Target(path) => Self::Target(path.clone()),
            ObservationRequest::Periodic => Self::Periodic {
                eligible_branches: eligible_branches(repo_root),
            },
        }
    }

    /// Decide what may be spent on one worktree.
    pub fn admits(&self, candidate: &InspectionCandidate<'_>) -> Admission {
        // The main worktree *is* the base: there is nothing to simulate merging
        // it into. A detached or branchless worktree has no branch identity to
        // compare, which is the same reason the pre-filter code always had.
        if candidate.is_main || candidate.has_no_branch_identity {
            return Admission::Skip;
        }

        match self {
            Self::Listing => Admission::Skip,
            // An operator asked about this exact worktree, so the answer must
            // come from the repository as it is now — including for branches
            // like `ws-session-*` that map to no OpenSpec change at all.
            Self::Target(path) => {
                if same_path(path, candidate.path) {
                    Admission::Fresh
                } else {
                    Admission::Skip
                }
            }
            Self::Periodic { eligible_branches } => {
                if eligible_branches.contains(candidate.branch) {
                    return Admission::Cacheable;
                }
                match crate::vcs::GitWorkspaceManager::extract_change_id_from_worktree_name(
                    candidate.branch,
                ) {
                    Some(change_id) if eligible_branches.contains(&change_id) => {
                        Admission::Cacheable
                    }
                    _ => Admission::Skip,
                }
            }
        }
    }
}

/// Branch identities that periodic refresh is allowed to inspect.
///
/// Active changes and rejected ones both qualify: a rejected change's worktree
/// is retained on purpose, and an operator reviewing it needs the same accurate
/// merge guidance as any other. Archived, completed, and unrelated branches are
/// absent, which is exactly what makes them ineligible.
///
/// Both the raw change ID and its sanitized branch name are inserted, because
/// the branch a worktree is created on is the sanitized form.
pub fn eligible_branches(repo_root: &Path) -> BTreeSet<String> {
    // A change tree that cannot be read yields no eligibility, which fails
    // closed onto "inspect nothing" rather than "inspect everything".
    let active = crate::openspec::list_changes_native_from(repo_root).unwrap_or_default();
    let rejected =
        crate::openspec::list_rejected_changes_native_from(repo_root).unwrap_or_default();

    let mut branches = BTreeSet::new();
    for change in active.into_iter().chain(rejected) {
        branches.insert(super::service::branch_name_for_change(&change.id));
        branches.insert(change.id);
    }
    branches
}

/// Everything that must be unchanged for a previous inspection to still answer.
///
/// The repository is part of the key because one process can serve several
/// repositories, and two of them can legitimately hold the same branch name at
/// the same commit. Scoping by repository can only ever *prevent* reuse, never
/// cause a stale one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObservationKey {
    /// Repository the observation was taken in.
    pub repository: PathBuf,
    /// Branch identity of the inspected worktree.
    pub branch: String,
    /// Base branch tip at observation time.
    pub base_head: String,
    /// Worktree HEAD at observation time.
    pub worktree_head: String,
    /// Merge base of the two.
    pub merge_base: String,
}

/// The bounded result of one merge simulation plus its ahead count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// Bounded conflict sample; empty means the simulation found no conflict.
    pub conflict_files: Vec<String>,
    /// Whether the branch carries commits base does not have.
    pub has_commits_ahead: SafetyFact,
}

/// How many observations one process retains before starting over.
///
/// The cache exists to stop one unchanged revision tuple from being simulated
/// twice, not to remember history: every HEAD movement mints a new key, so an
/// unbounded map would grow for the life of a long-running TUI. Overflowing it
/// discards everything, which costs one extra simulation per live worktree and
/// cannot produce a wrong answer.
pub const MAX_CACHED_OBSERVATIONS: usize = 512;

/// Process-local, disposable store of revision-keyed observations.
#[derive(Debug, Default)]
pub struct ObservationCache {
    entries: Mutex<HashMap<ObservationKey, Observation>>,
}

impl ObservationCache {
    /// An empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// The observation for this exact revision tuple, if one was stored.
    pub fn get(&self, key: &ObservationKey) -> Option<Observation> {
        self.lock().get(key).cloned()
    }

    /// Store an observation under its revision tuple.
    pub fn insert(&self, key: ObservationKey, observation: Observation) {
        let mut entries = self.lock();
        if entries.len() >= MAX_CACHED_OBSERVATIONS && !entries.contains_key(&key) {
            entries.clear();
        }
        entries.insert(key, observation);
    }

    /// Number of retained observations.
    // The binary target recompiles this tree without tests and sees no caller
    // for the inspection surface below; the lib and its integration tests do.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// True when nothing is retained.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Discard everything. Never changes any decision, only recomputation cost.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn clear(&self) {
        self.lock().clear();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<ObservationKey, Observation>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The one cache both periodic refresh paths share.
///
/// Sharing it is the point: a TUI refresh and a Web/UDS refresh that see the
/// same revision tuple must together cost one merge simulation, not two.
pub fn shared_cache() -> &'static ObservationCache {
    static SHARED: OnceLock<ObservationCache> = OnceLock::new();
    SHARED.get_or_init(ObservationCache::new)
}

// ============================================================================
// Command recorder
// ============================================================================

/// A Git command class the inspection layer can spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectionCommand {
    /// `git merge-base`, which derives the cache key.
    MergeBase,
    /// `git merge-tree`, the merge simulation itself.
    Conflicts,
    /// `git rev-list --count`, the commits-ahead count.
    CommitsAhead,
}

/// One spawned inspection command, attributed to the worktree it ran for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionRecord {
    /// Which command class ran.
    pub command: InspectionCommand,
    /// Worktree the command was spawned for.
    pub worktree: PathBuf,
}

static RECORDING: AtomicBool = AtomicBool::new(false);

fn records() -> &'static Mutex<Vec<InspectionRecord>> {
    static RECORDS: OnceLock<Mutex<Vec<InspectionRecord>>> = OnceLock::new();
    RECORDS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Start recording spawned inspection commands.
///
/// Off by default, so an ordinary process pays one relaxed atomic load per
/// inspected worktree and stores nothing. Tests turn it on to count what a
/// refresh *actually* spawned instead of asserting on the code path they
/// believe was taken. Records are observability only and are never read by any
/// decision.
#[cfg_attr(not(test), allow(dead_code))]
pub fn record_inspection_commands() {
    RECORDING.store(true, Ordering::Relaxed);
}

pub(crate) fn record(command: InspectionCommand, worktree: &Path) {
    if !RECORDING.load(Ordering::Relaxed) {
        return;
    }
    records()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(InspectionRecord {
            command,
            worktree: worktree.to_path_buf(),
        });
}

/// Every recorded command spawned for a worktree under `prefix`.
///
/// Scoping by prefix is what lets concurrent tests share one process-wide
/// recorder: each one filters to its own repository.
#[cfg_attr(not(test), allow(dead_code))]
pub fn recorded_inspections_under(prefix: &Path) -> Vec<InspectionRecord> {
    let prefix = std::fs::canonicalize(prefix).unwrap_or_else(|_| prefix.to_path_buf());
    records()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|record| {
            let path =
                std::fs::canonicalize(&record.worktree).unwrap_or_else(|_| record.worktree.clone());
            path.starts_with(&prefix)
        })
        .cloned()
        .collect()
}

/// Count recorded commands of one class under `prefix`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn recorded_inspection_count(prefix: &Path, command: InspectionCommand) -> usize {
    recorded_inspections_under(prefix)
        .into_iter()
        .filter(|record| record.command == command)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn periodic(eligible: &[&str]) -> InspectionScope {
        InspectionScope::Periodic {
            eligible_branches: eligible.iter().map(|id| (*id).to_string()).collect(),
        }
    }

    fn candidate<'a>(path: &'a Path, branch: &'a str) -> InspectionCandidate<'a> {
        InspectionCandidate::new(path, branch, false, false)
    }

    fn key(branch: &str, base: &str, head: &str, merge_base: &str) -> ObservationKey {
        ObservationKey {
            repository: PathBuf::from("/repo"),
            branch: branch.to_string(),
            base_head: base.to_string(),
            worktree_head: head.to_string(),
            merge_base: merge_base.to_string(),
        }
    }

    fn observation() -> Observation {
        Observation {
            conflict_files: Vec::new(),
            has_commits_ahead: SafetyFact::Yes,
        }
    }

    #[test]
    fn periodic_refresh_inspects_only_current_change_branches() {
        let scope = periodic(&["live-change"]);
        let live = PathBuf::from("/w/live-change");
        let stale = PathBuf::from("/w/stale-change");

        assert_eq!(
            scope.admits(&candidate(&live, "live-change")),
            Admission::Cacheable,
            "a branch naming a current change is the case automatic inspection exists for"
        );
        assert_eq!(
            scope.admits(&candidate(&stale, "stale-change")),
            Admission::Skip,
            "a branch that maps to no current change must not cost a merge simulation"
        );
        assert_eq!(
            scope.admits(&candidate(&stale, "ws-session-a1b2c3")),
            Admission::Skip,
            "a session branch maps to no change and is periodically ineligible"
        );
    }

    #[test]
    fn legacy_prefixed_branches_resolve_to_their_change_id() {
        let scope = periodic(&["live-change"]);
        let path = PathBuf::from("/w/legacy");

        assert_eq!(
            scope.admits(&candidate(&path, "ws-live-change-a1b2c3d4")),
            Admission::Cacheable,
            "the legacy ws-<id>-<hex> branch form names the same change"
        );
    }

    #[test]
    fn structural_rows_never_spawn_inspection_commands() {
        let scope = periodic(&["live-change"]);
        let path = PathBuf::from("/w/live-change");

        assert_eq!(
            scope.admits(&InspectionCandidate::new(&path, "main", true, false)),
            Admission::Skip,
            "the main worktree is the base; there is nothing to simulate merging it into"
        );
        assert_eq!(
            scope.admits(&InspectionCandidate::new(&path, "", false, true)),
            Admission::Skip,
            "a detached worktree has no branch identity to compare"
        );
        assert_eq!(
            scope.admits(&InspectionCandidate::new(&path, "", false, false)),
            Admission::Skip,
            "a branchless worktree has no branch identity to compare"
        );
    }

    #[test]
    fn listing_scope_inspects_nothing_at_all() {
        let path = PathBuf::from("/w/live-change");

        assert_eq!(
            InspectionScope::Listing.admits(&candidate(&path, "live-change")),
            Admission::Skip,
            "a structural listing must not pay for merge simulation even for a live change"
        );
    }

    #[test]
    fn an_operator_target_is_inspected_fresh_even_when_periodic_refresh_would_skip_it() {
        let target = PathBuf::from("/w/ws-session-a1b2c3");
        let other = PathBuf::from("/w/live-change");
        let scope = InspectionScope::Target(target.clone());

        assert_eq!(
            scope.admits(&candidate(&target, "ws-session-a1b2c3")),
            Admission::Fresh,
            "an operator-addressed worktree is inspected from current evidence, mapped or not"
        );
        assert_eq!(
            scope.admits(&candidate(&other, "live-change")),
            Admission::Skip,
            "a targeted observation pays for its target only"
        );
    }

    #[test]
    fn a_cached_observation_is_reused_only_for_an_identical_revision_tuple() {
        let cache = ObservationCache::new();
        let stored = key("live-change", "base1", "head1", "merge1");
        cache.insert(stored.clone(), observation());

        assert_eq!(cache.get(&stored), Some(observation()));

        for (name, changed) in [
            ("branch", key("other-change", "base1", "head1", "merge1")),
            ("base head", key("live-change", "base2", "head1", "merge1")),
            (
                "worktree head",
                key("live-change", "base1", "head2", "merge1"),
            ),
            ("merge base", key("live-change", "base1", "head1", "merge2")),
        ] {
            assert_eq!(
                cache.get(&changed),
                None,
                "a changed {name} must invalidate the observation rather than reuse it"
            );
        }

        let mut other_repository = stored.clone();
        other_repository.repository = PathBuf::from("/other-repo");
        assert_eq!(
            cache.get(&other_repository),
            None,
            "two repositories holding the same branch at the same commit are different observations"
        );
    }

    #[test]
    fn the_cache_is_bounded_and_disposable() {
        let cache = ObservationCache::new();
        for index in 0..(MAX_CACHED_OBSERVATIONS + 1) {
            cache.insert(
                key("live-change", "base", &format!("head{index}"), "merge"),
                observation(),
            );
        }

        assert!(
            cache.len() <= MAX_CACHED_OBSERVATIONS,
            "an unbounded cache would grow for the life of the process"
        );

        cache.clear();
        assert!(
            cache.is_empty(),
            "the cache must be discardable at any time"
        );
    }

    #[test]
    fn inspection_state_separates_looked_from_found_nothing() {
        assert!(InspectionState::Checked.is_inspected());
        assert!(InspectionState::Reused.is_inspected());
        assert!(!InspectionState::NotInspected.is_inspected());
        assert_eq!(InspectionState::default(), InspectionState::NotInspected);
        assert_eq!(InspectionState::Reused.label(), "cached");
        assert_eq!(InspectionState::NotInspected.label(), "not inspected");
    }
}
