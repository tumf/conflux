//! Shared, frontend-independent worktree operation service.
//!
//! Frontends (TUI keypresses, `/api/v2` commands) map operator intent onto this
//! service instead of driving Git themselves. The service owns the parts that
//! must not diverge between frontends: eligibility classification, the
//! repository mutation guard, mandatory teardown on delete, the base merge, the
//! `on_merged` hook, and the operation events every frontend publishes.
//!
//! Two boundaries are kept injectable so the decision logic can be verified
//! without a real repository:
//!
//! * [`WorktreeBackend`] — every Git/filesystem/hook side effect;
//! * [`WorktreeEventSink`] — where operation events go.
//!
//! What is *not* shared is per-frontend policy that the two contracts genuinely
//! disagree on. Those are explicit values ([`DeleteOptions`], [`ConflictPolicy`])
//! rather than duplicated implementations, so a reader can see the whole
//! difference in one place:
//!
//! * the TUI keeps its local recovery `skip_teardown` escape hatch and it is the
//!   only caller that may grant `allow_known_dirty`, and only after a second,
//!   explicitly destructive operator confirmation;
//! * `/api/v2` is fail-closed on both and preserves a conflicted merge instead
//!   of aborting it, because a remote client has no way to inspect or resolve
//!   the intermediate state.
//!
//! Every safety observation deletion depends on is tri-state. An observation
//! that failed is [`SafetyFact::Unknown`] or [`DirtyState::Unknown`], never a
//! `false` that reads like a clean bill of health, and deletion refuses on it.

use std::fmt;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Recovery guidance reported for a preserved merge conflict.
///
/// There is deliberately no remote resolve or abort command, so the only honest
/// answer a remote client can be given is "come back locally".
pub const RECOVERY_LOCAL_OR_TUI: &str = "local_or_tui_required";

// ============================================================================
// Observations
// ============================================================================

/// Tri-state dirty observation.
///
/// `Unknown` is a real outcome, not an error to swallow: a failed status read
/// must be representable so a caller can decide whether to fail closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyState {
    /// The worktree has no uncommitted changes.
    Clean,
    /// The worktree has uncommitted changes.
    Dirty,
    /// Dirty state could not be determined.
    Unknown,
}

impl DirtyState {
    /// Wire representation: `null` when the observation failed.
    pub fn as_option(self) -> Option<bool> {
        match self {
            Self::Clean => Some(false),
            Self::Dirty => Some(true),
            Self::Unknown => None,
        }
    }
}

/// Tri-state observation of a yes/no safety fact.
///
/// The point of the third state is that a failed `git` invocation must not be
/// indistinguishable from a confident "no". Deletion refuses on `Unknown`
/// exactly as it refuses on the unsafe answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyFact {
    /// The condition was observed to hold.
    Yes,
    /// The condition was observed not to hold.
    No,
    /// The condition could not be observed.
    Unknown,
}

impl SafetyFact {
    /// Fold an observation result into a fact, mapping failure onto `Unknown`.
    pub fn observed<E>(result: Result<bool, E>) -> Self {
        match result {
            Ok(true) => Self::Yes,
            Ok(false) => Self::No,
            Err(_) => Self::Unknown,
        }
    }

    /// True only for a confidently observed `Yes`.
    ///
    /// Projections that must answer with a plain boolean use this: `Unknown` is
    /// not evidence *for* the condition, and eligibility is reported separately.
    pub fn is_known_yes(self) -> bool {
        self == Self::Yes
    }
}

impl From<bool> for SafetyFact {
    fn from(value: bool) -> Self {
        if value {
            Self::Yes
        } else {
            Self::No
        }
    }
}

/// Everything an operation decision needs to know about one worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeFacts {
    /// Absolute filesystem path. Never serialized to a remote client.
    pub path: PathBuf,
    /// Opaque per-worktree Git identity (the `gitdir:` pointer).
    pub identity: String,
    /// Checked-out branch, empty when detached.
    pub branch: String,
    /// Current HEAD commit.
    pub head: String,
    /// True for the repository's main worktree.
    pub is_main: bool,
    /// True when HEAD is detached.
    pub is_detached: bool,
    /// Whether this branch has commits the base does not have.
    pub has_commits_ahead: SafetyFact,
    /// Repository-relative paths that would conflict with a base merge.
    pub conflict_files: Vec<String>,
    /// Uncommitted-change observation.
    pub dirty: DirtyState,
    /// Whether the base repository is sitting on an unresolved merge.
    pub base_merge_in_progress: SafetyFact,
}

impl WorktreeFacts {
    /// A minimal fact set, used by tests and by callers that fill fields in.
    // The binary crate recompiles this tree without tests and sees no caller.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(path: impl Into<PathBuf>, branch: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            identity: String::new(),
            branch: branch.into(),
            head: String::new(),
            is_main: false,
            is_detached: false,
            has_commits_ahead: SafetyFact::No,
            conflict_files: Vec::new(),
            dirty: DirtyState::Clean,
            base_merge_in_progress: SafetyFact::No,
        }
    }
}

/// The freshly observed target of a known-dirty refusal.
///
/// A local frontend may escalate a `Dirty` refusal into a destructive
/// confirmation. What that confirmation is allowed to name is *this* — the
/// service's own fresh observation — and never a projection the frontend was
/// already holding, which may be arbitrarily stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyTarget {
    /// Absolute path the observation was taken at.
    pub path: PathBuf,
    /// Git worktree identity at observation time.
    pub identity: String,
    /// Branch at observation time, empty when detached.
    pub branch: String,
    /// HEAD commit at observation time.
    pub head: String,
}

impl DirtyTarget {
    /// Capture the escalation identity from an observation.
    pub fn from_facts(facts: &WorktreeFacts) -> Self {
        Self {
            path: facts.path.clone(),
            identity: facts.identity.clone(),
            branch: facts.branch.clone(),
            head: facts.head.clone(),
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

/// A typed refusal or failure from a worktree operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeOpError {
    /// The addressed worktree is not present in the current observation.
    NotFound(String),
    /// A worktree for this change already exists.
    Exists(String),
    /// The worktree has uncommitted changes.
    ///
    /// Every other safety check already passed when this is returned, so it is
    /// the *only* refusal a local frontend may escalate into an explicit
    /// dirty-discard confirmation. The boxed target is the observation the
    /// confirmation must be revalidated against.
    Dirty {
        /// Operator-facing refusal text.
        message: String,
        /// Identity of the worktree as the service just observed it.
        target: Box<DirtyTarget>,
    },
    /// Dirty state could not be determined, so the operation fails closed.
    DirtyUnknown(String),
    /// The worktree is present but cannot accept this operation.
    Ineligible(String),
    /// The repository root is occupied by another operation or an unresolved merge.
    RootBusy(String),
    /// The base merge conflicted; intermediate state was preserved.
    MergeConflict {
        /// Repository-relative conflicted paths.
        files: Vec<String>,
        /// How the conflict must be resolved.
        recovery: &'static str,
    },
    /// Sanitized failure from the underlying boundary.
    Internal(String),
}

impl fmt::Display for WorktreeOpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(message)
            | Self::Exists(message)
            | Self::Dirty { message, .. }
            | Self::DirtyUnknown(message)
            | Self::Ineligible(message)
            | Self::RootBusy(message)
            | Self::Internal(message) => write!(f, "{message}"),
            Self::MergeConflict { files, recovery } => write!(
                f,
                "base merge conflicted in {} file(s) [{}]; intermediate merge state was preserved, recovery: {}",
                files.len(),
                files.join(", "),
                recovery
            ),
        }
    }
}

/// Result alias for service operations.
pub type WorktreeOpResult<T> = Result<T, WorktreeOpError>;

// ============================================================================
// Per-frontend policy
// ============================================================================

/// How a conflicting base merge is disposed of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    /// Run `git merge --abort` and report the conflict as a failure (TUI).
    AbortOnConflict,
    /// Leave `MERGE_HEAD` and the conflicted index in place (`/api/v2`).
    PreserveConflict,
}

/// Caller-declared delete policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteOptions {
    /// Skip `.wt/teardown`. Local recovery only; never reachable from `/api/v2`.
    pub skip_teardown: bool,
    /// Permission to discard *known* uncommitted work.
    ///
    /// This waives [`DirtyState::Dirty`] and nothing else. It never waives an
    /// unobservable fact, commits ahead of base, main-worktree status, or an
    /// identity mismatch, and only the local TUI's second, explicitly
    /// destructive confirmation may set it.
    pub allow_known_dirty: bool,
}

impl DeleteOptions {
    /// The fail-closed policy every remote caller uses.
    pub fn fail_closed() -> Self {
        Self {
            skip_teardown: false,
            allow_known_dirty: false,
        }
    }

    /// The ordinary local policy: teardown is the operator's choice, discard is not.
    ///
    /// `Y` and `S` both land here. Neither grants dirty discard, so a known-dirty
    /// target refuses and the TUI escalates that refusal into its destructive
    /// confirmation instead of deleting.
    pub fn local(skip_teardown: bool) -> Self {
        Self {
            skip_teardown,
            allow_known_dirty: false,
        }
    }

    /// The local policy after an explicit destructive confirmation.
    pub fn local_discarding_dirty(skip_teardown: bool) -> Self {
        Self {
            skip_teardown,
            allow_known_dirty: true,
        }
    }
}

// ============================================================================
// Eligibility classification (pure)
// ============================================================================

/// Decide whether a worktree may be deleted under the caller's policy.
///
/// Dirty is checked *last* on purpose. [`WorktreeOpError::Dirty`] is the one
/// refusal a frontend may escalate into a destructive confirmation, so it must
/// mean "nothing but uncommitted work stands in the way". A worktree that is
/// both dirty and ahead of base reports the ahead refusal, which explicit
/// discard cannot waive, and therefore never reaches that confirmation.
pub fn classify_delete_eligibility(
    facts: &WorktreeFacts,
    options: DeleteOptions,
) -> WorktreeOpResult<()> {
    if facts.is_main {
        return Err(WorktreeOpError::Ineligible(
            "the main worktree cannot be deleted".to_string(),
        ));
    }
    match facts.base_merge_in_progress {
        SafetyFact::Yes => {
            return Err(WorktreeOpError::RootBusy(format!(
            "the repository root is holding an unresolved merge; recovery: {RECOVERY_LOCAL_OR_TUI}"
        )))
        }
        SafetyFact::Unknown => {
            return Err(WorktreeOpError::Ineligible(
                "the repository root's merge state could not be determined".to_string(),
            ))
        }
        SafetyFact::No => {}
    }
    match facts.has_commits_ahead {
        SafetyFact::Yes => {
            return Err(WorktreeOpError::Ineligible(
                "the worktree has unmerged commits ahead of base".to_string(),
            ))
        }
        SafetyFact::Unknown => {
            return Err(WorktreeOpError::Ineligible(
                "the worktree's commits-ahead state could not be determined".to_string(),
            ))
        }
        SafetyFact::No => {}
    }
    match facts.dirty {
        DirtyState::Dirty if !options.allow_known_dirty => {
            return Err(WorktreeOpError::Dirty {
                message: "the worktree has uncommitted changes".to_string(),
                target: Box::new(DirtyTarget::from_facts(facts)),
            })
        }
        DirtyState::Unknown => {
            return Err(WorktreeOpError::DirtyUnknown(
                "the worktree's uncommitted-change state could not be determined".to_string(),
            ))
        }
        DirtyState::Dirty | DirtyState::Clean => {}
    }
    Ok(())
}

/// The identity a caller confirmed, revalidated before anything is mutated.
///
/// A confirmation names a worktree, not a path. Between the moment a frontend
/// confirms a deletion and the moment the service takes the mutation guard, the
/// path can be re-occupied by a different worktree, so whatever the caller can
/// name is re-checked against the fresh observation before teardown runs.
///
/// A `None` field is one the caller has nothing to revalidate against and
/// accepts as-observed. An empty [`ExpectedTarget`] accepts whatever currently
/// occupies the path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExpectedTarget {
    /// Branch the caller confirmed against.
    pub branch: Option<String>,
    /// Git worktree identity the caller confirmed against.
    pub identity: Option<String>,
    /// HEAD commit the caller confirmed against.
    pub head: Option<String>,
}

impl ExpectedTarget {
    /// Revalidate nothing: the caller accepts whatever occupies the path.
    pub fn unchecked() -> Self {
        Self::default()
    }

    /// Revalidate the confirmed branch.
    pub fn on_branch(branch: impl Into<String>) -> Self {
        Self {
            branch: Some(branch.into()),
            ..Self::default()
        }
    }

    /// Also revalidate the Git worktree identity.
    pub fn with_identity(mut self, identity: impl Into<String>) -> Self {
        self.identity = Some(identity.into());
        self
    }

    /// Also revalidate the HEAD commit.
    pub fn with_head(mut self, head: impl Into<String>) -> Self {
        self.head = Some(head.into());
        self
    }
}

/// Decide whether the freshly observed worktree is still the one the caller confirmed.
pub fn classify_delete_identity(
    facts: &WorktreeFacts,
    expected: &ExpectedTarget,
) -> WorktreeOpResult<()> {
    if let Some(expected_branch) = expected.branch.as_deref() {
        if facts.branch != expected_branch {
            let observed = if facts.branch.is_empty() {
                "<detached>"
            } else {
                facts.branch.as_str()
            };
            return Err(WorktreeOpError::NotFound(format!(
                "the confirmed worktree on branch '{expected_branch}' is no longer at this path (it is now on '{observed}'); refresh and confirm again"
            )));
        }
    }
    if let Some(expected_identity) = expected.identity.as_deref() {
        if facts.identity != expected_identity {
            return Err(WorktreeOpError::NotFound(
                "the confirmed worktree's Git identity no longer matches this path; refresh and confirm again"
                    .to_string(),
            ));
        }
    }
    if let Some(expected_head) = expected.head.as_deref() {
        if facts.head != expected_head {
            return Err(WorktreeOpError::NotFound(format!(
                "the confirmed worktree moved from HEAD '{expected_head}' to '{}'; refresh and confirm again",
                facts.head
            )));
        }
    }
    Ok(())
}

/// Decide whether two observations of the same deletion still describe one target.
///
/// Taken across the teardown boundary: teardown is arbitrary operator code, and
/// the mutation guard only excludes Conflux's own operations, so the facts the
/// deletion was authorized from are re-derived rather than assumed.
pub fn classify_delete_drift(
    before: &WorktreeFacts,
    after: &WorktreeFacts,
) -> WorktreeOpResult<()> {
    if before.identity != after.identity {
        return Err(WorktreeOpError::NotFound(
            "the worktree's Git identity changed while it was being torn down; nothing was removed"
                .to_string(),
        ));
    }
    if before.branch != after.branch {
        return Err(WorktreeOpError::NotFound(format!(
            "the worktree moved from branch '{}' to '{}' while it was being torn down; nothing was removed",
            before.branch, after.branch
        )));
    }
    if before.head != after.head {
        return Err(WorktreeOpError::NotFound(format!(
            "the worktree moved from HEAD '{}' to '{}' while it was being torn down; nothing was removed",
            before.head, after.head
        )));
    }
    Ok(())
}

/// Decide whether a worktree may be merged into base under the caller's policy.
pub fn classify_merge_eligibility(
    facts: &WorktreeFacts,
    policy: ConflictPolicy,
) -> WorktreeOpResult<()> {
    if facts.is_main {
        return Err(WorktreeOpError::Ineligible(
            "the main worktree cannot be merged into itself".to_string(),
        ));
    }
    match facts.base_merge_in_progress {
        SafetyFact::Yes => {
            return Err(WorktreeOpError::RootBusy(format!(
            "the repository root is holding an unresolved merge; recovery: {RECOVERY_LOCAL_OR_TUI}"
        )))
        }
        SafetyFact::Unknown => {
            return Err(WorktreeOpError::Ineligible(
                "the repository root's merge state could not be determined".to_string(),
            ))
        }
        SafetyFact::No => {}
    }
    if facts.is_detached || facts.branch.is_empty() {
        return Err(WorktreeOpError::Ineligible(
            "a detached worktree has no branch to merge".to_string(),
        ));
    }
    // A pre-detected conflict refuses up front only when the caller would abort
    // anyway. The preserving caller runs the merge so the evidence it promises
    // its clients actually exists in the repository.
    if policy == ConflictPolicy::AbortOnConflict && !facts.conflict_files.is_empty() {
        return Err(WorktreeOpError::Ineligible(
            "the worktree conflicts with base".to_string(),
        ));
    }
    if !facts.has_commits_ahead.is_known_yes() {
        return Err(WorktreeOpError::Ineligible(
            "the worktree has no commits ahead of base".to_string(),
        ));
    }
    Ok(())
}

/// Server-derived branch name for a change worktree.
///
/// Clients never supply a branch: this is the single derivation both frontends
/// use, so a change ID cannot be smuggled into an unrelated ref.
pub fn branch_name_for_change(change_id: &str) -> String {
    change_id.replace(['/', '\\', ' '], "-")
}

/// Server-derived worktree path for a change worktree.
pub fn worktree_path_for_change(workspace_base_dir: &Path, change_id: &str) -> PathBuf {
    workspace_base_dir.join(branch_name_for_change(change_id))
}

/// True when two paths name the same worktree.
///
/// Git reports canonical paths, while a server-derived path is whatever the
/// configured workspace root produced. On platforms with symlinked temp or home
/// directories those differ textually for the same directory, so a plain `==`
/// would report a freshly created worktree as unobservable.
fn same_path(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}

// ============================================================================
// Boundaries
// ============================================================================

/// Result of a base merge attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeAttempt {
    /// The merge produced a merge commit.
    Merged,
    /// The merge conflicted. Whether state was preserved is the policy's business.
    Conflict {
        /// Repository-relative conflicted paths.
        files: Vec<String>,
    },
}

/// Every Git/filesystem/hook side effect the service performs.
#[async_trait]
pub trait WorktreeBackend: Send + Sync {
    /// Observe every current worktree, including dirty and conflict facts.
    async fn observe(&self) -> WorktreeOpResult<Vec<WorktreeFacts>>;
    /// Current managed base HEAD. Clients never choose a base commit.
    async fn base_head(&self) -> WorktreeOpResult<String>;
    /// Create a worktree at `path` on a new `branch` from `base_commit`.
    async fn create(&self, path: &Path, branch: &str, base_commit: &str) -> WorktreeOpResult<()>;
    /// Run `.wt/teardown` for a worktree without removing anything.
    ///
    /// Separate from [`Self::remove_worktree`] so the service can re-observe in
    /// between: teardown runs operator-supplied code that may change the very
    /// facts the removal was authorized from.
    async fn teardown(&self, path: &Path) -> WorktreeOpResult<()>;
    /// Force-remove a worktree's Git registration and directory.
    async fn remove_worktree(&self, path: &Path) -> WorktreeOpResult<()>;
    /// Commit a local branch ref currently points at, or `None` when it is gone.
    ///
    /// An `Err` means the ref state could not be read, which a caller must not
    /// treat as "already deleted".
    async fn branch_ref(&self, branch: &str) -> WorktreeOpResult<Option<String>>;
    /// Delete a branch only if Git can still reach its commits from elsewhere.
    ///
    /// Best-effort: a refusal retains the branch and is reported, not fatal.
    async fn delete_branch_if_merged(&self, branch: &str) -> WorktreeOpResult<()>;
    /// Merge `branch` into base under the given conflict policy.
    async fn merge_into_base(
        &self,
        branch: &str,
        policy: ConflictPolicy,
    ) -> WorktreeOpResult<MergeAttempt>;
    /// Run the `on_merged` hook exactly once for a completed merge.
    async fn run_on_merged(&self, change_id: &str, worktree_path: &Path) -> WorktreeOpResult<()>;
    /// True when `change_id` is a managed, non-archived change eligible for a worktree.
    async fn change_is_eligible(&self, change_id: &str) -> WorktreeOpResult<()>;
}

/// A worktree operation event, published identically by every frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeOperationEvent {
    /// A worktree was created.
    Created {
        /// Branch that was created.
        branch: String,
    },
    /// A worktree was removed after teardown.
    Deleted {
        /// Branch the removed worktree was on.
        branch: String,
    },
    /// A base merge started.
    MergeStarted {
        /// Branch being merged into base.
        branch: String,
    },
    /// A base merge completed, including its `on_merged` hook.
    MergeCompleted {
        /// Branch merged into base.
        branch: String,
    },
    /// A base merge failed or conflicted.
    MergeFailed {
        /// Branch that failed to merge.
        branch: String,
        /// Sanitized failure detail.
        error: String,
    },
    /// The worktree list changed and observers should refresh.
    Refreshed,
}

/// Where operation events are published.
#[async_trait]
pub trait WorktreeEventSink: Send + Sync {
    /// Publish one operation event.
    async fn emit(&self, event: WorktreeOperationEvent);
}

/// Sink that drops every event, for callers with no observers.
// Constructed by the heavy real-Git suite; the binary crate recompiles this tree
// without those tests and sees no caller.
#[cfg_attr(not(test), allow(dead_code))]
pub struct NullEventSink;

#[async_trait]
impl WorktreeEventSink for NullEventSink {
    async fn emit(&self, _event: WorktreeOperationEvent) {}
}

// ============================================================================
// Service
// ============================================================================

/// What a completed operation did, for the caller's operator-facing detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeOpOutcome {
    /// Branch the operation acted on.
    pub branch: String,
    /// Absolute path the operation acted on. Callers redact as their contract requires.
    pub path: PathBuf,
    /// Human-readable summary.
    pub detail: String,
}

/// The shared operation service.
pub struct WorktreeService {
    backend: std::sync::Arc<dyn WorktreeBackend>,
    events: std::sync::Arc<dyn WorktreeEventSink>,
    workspace_base_dir: PathBuf,
    /// Serializes repository-mutating operations within this process.
    ///
    /// Held with `try_lock`, not `lock`: a caller that would have to wait is told
    /// `root_busy` immediately instead of queueing behind an operation it cannot
    /// see, which is what lets a remote client retry against a fresh revision.
    root_guard: Mutex<()>,
}

impl WorktreeService {
    /// Build a service over a backend, an event sink, and the managed workspace root.
    pub fn new(
        backend: std::sync::Arc<dyn WorktreeBackend>,
        events: std::sync::Arc<dyn WorktreeEventSink>,
        workspace_base_dir: PathBuf,
    ) -> Self {
        Self {
            backend,
            events,
            workspace_base_dir,
            root_guard: Mutex::new(()),
        }
    }

    /// Current worktree observations. Read-only; takes no mutation guard.
    pub async fn observe(&self) -> WorktreeOpResult<Vec<WorktreeFacts>> {
        self.backend.observe().await
    }

    /// Create the managed worktree for an eligible change.
    ///
    /// Branch, path, and base commit are all server-derived: the only input is
    /// the change ID.
    pub async fn create_change_worktree(&self, change_id: &str) -> WorktreeOpResult<WorktreeFacts> {
        let _guard = self.acquire_root()?;

        self.backend.change_is_eligible(change_id).await?;

        let branch = branch_name_for_change(change_id);
        let path = worktree_path_for_change(&self.workspace_base_dir, change_id);

        let observed = self.backend.observe().await?;
        if observed
            .iter()
            .any(|facts| facts.branch == branch || same_path(&facts.path, &path))
        {
            return Err(WorktreeOpError::Exists(format!(
                "a worktree for change '{change_id}' already exists"
            )));
        }

        let base_commit = self.backend.base_head().await?;
        self.backend.create(&path, &branch, &base_commit).await?;

        self.events
            .emit(WorktreeOperationEvent::Created {
                branch: branch.clone(),
            })
            .await;
        self.events.emit(WorktreeOperationEvent::Refreshed).await;

        // Identity is only meaningful once the resource actually exists, so it is
        // read back from a fresh observation rather than predicted.
        self.backend
            .observe()
            .await?
            .into_iter()
            .find(|facts| same_path(&facts.path, &path))
            .ok_or_else(|| {
                WorktreeOpError::Internal(format!(
                    "worktree for change '{change_id}' was created but is not observable"
                ))
            })
    }

    /// Delete a managed worktree after teardown.
    ///
    /// `expected` is the identity the caller confirmed. It is revalidated against
    /// the fresh observation taken under the mutation guard, so a path that was
    /// re-occupied since the confirmation refuses instead of deleting whichever
    /// worktree now happens to live there.
    ///
    /// Teardown and Git removal are two phases with a second observation between
    /// them. The mutation guard serializes Conflux's own operations; it does not
    /// stop the teardown script — or anything else outside this process — from
    /// moving the target, so every safety fact is re-derived immediately before
    /// the irreversible step rather than carried across it.
    pub async fn delete_worktree(
        &self,
        path: &Path,
        expected: &ExpectedTarget,
        options: DeleteOptions,
    ) -> WorktreeOpResult<WorktreeOpOutcome> {
        let _guard = self.acquire_root()?;

        let before = self.locate(path).await?;
        classify_delete_identity(&before, expected)?;
        classify_delete_eligibility(&before, options)?;

        // Teardown must succeed before the resource is retired: a failed delete
        // leaves both the worktree and its identity binding intact.
        if !options.skip_teardown {
            self.backend.teardown(&before.path).await?;
        }

        let after = self.locate(&before.path).await?;
        classify_delete_identity(&after, expected)?;
        classify_delete_drift(&before, &after)?;
        classify_delete_eligibility(&after, options)?;
        self.confirm_branch_ref(&after).await?;

        if options.allow_known_dirty {
            warn!(
                path = %after.path.display(),
                branch = %after.branch,
                dirty_discard = true,
                skip_teardown = options.skip_teardown,
                "discarding known uncommitted changes: forcing removal of a dirty worktree on explicit operator confirmation"
            );
        }

        self.backend.remove_worktree(&after.path).await?;

        let branch_note = self.cleanup_branch(&after).await;

        self.events
            .emit(WorktreeOperationEvent::Deleted {
                branch: after.branch.clone(),
            })
            .await;
        self.events.emit(WorktreeOperationEvent::Refreshed).await;

        let teardown_note = if options.skip_teardown {
            "removed without teardown"
        } else {
            "torn down and removed"
        };
        Ok(WorktreeOpOutcome {
            detail: format!(
                "worktree on branch '{}' was {teardown_note}{branch_note}",
                after.branch
            ),
            branch: after.branch,
            path: after.path,
        })
    }

    /// Reconfirm the target's branch ref immediately before Git removal.
    ///
    /// `observe()` reports the worktree's HEAD; it does not prove the branch ref
    /// still names that commit. Forcing removal of a dirty worktree discards its
    /// uncommitted work, so the only thing that keeps its committed work
    /// recoverable is the branch pointing where the deletion was authorized
    /// from. A ref that moved, vanished, or cannot be read is unknown ref safety
    /// state, and the whole operation refuses rather than removing the directory
    /// and discovering the problem in best-effort branch cleanup — by then the
    /// worktree is already gone.
    ///
    /// A detached target has no branch ref to reconfirm; HEAD drift is already
    /// covered by [`classify_delete_drift`].
    async fn confirm_branch_ref(&self, facts: &WorktreeFacts) -> WorktreeOpResult<()> {
        if facts.branch.is_empty() {
            return Ok(());
        }
        match self.backend.branch_ref(&facts.branch).await {
            Ok(Some(oid)) if oid == facts.head => Ok(()),
            Ok(Some(oid)) => Err(WorktreeOpError::NotFound(format!(
                "branch '{}' moved from '{}' to '{oid}' after the deletion was authorized; nothing was removed",
                facts.branch, facts.head
            ))),
            Ok(None) => Err(WorktreeOpError::NotFound(format!(
                "branch '{}' no longer exists; nothing was removed",
                facts.branch
            ))),
            Err(error) => Err(WorktreeOpError::Ineligible(format!(
                "branch '{}' ref state could not be determined ({error}); nothing was removed",
                facts.branch
            ))),
        }
    }

    /// Best-effort branch cleanup that refuses to act on a stale decision.
    ///
    /// Worktree removal and branch deletion are distinct outcomes. The branch is
    /// deleted only when its ref still points at the OID the deletion was
    /// authorized from *and* Git can still reach those commits from elsewhere;
    /// anything else retains the branch, because an unreachable commit is not
    /// something a cleanup step gets to decide on someone's behalf.
    ///
    /// [`Self::confirm_branch_ref`] already established that OID immediately
    /// before removal. This reads it again rather than trusting that answer: the
    /// removal itself is a window in which the ref can still move.
    ///
    /// Returns the operator-facing suffix for the outcome detail.
    async fn cleanup_branch(&self, facts: &WorktreeFacts) -> String {
        if facts.branch.is_empty() {
            return String::new();
        }

        let retained = match self.backend.branch_ref(&facts.branch).await {
            Ok(None) => return String::new(),
            Ok(Some(oid)) if oid == facts.head => {
                match self.backend.delete_branch_if_merged(&facts.branch).await {
                    Ok(()) => return format!("; branch '{}' was deleted", facts.branch),
                    Err(error) => {
                        format!("its commits are not reachable from elsewhere ({error})")
                    }
                }
            }
            Ok(Some(oid)) => format!(
                "its ref moved from '{}' to '{oid}' after the deletion was authorized",
                facts.head
            ),
            Err(error) => format!("its ref state could not be reconfirmed ({error})"),
        };

        warn!(
            branch = %facts.branch,
            "branch was retained after worktree removal: {retained}"
        );
        format!(
            "; branch '{}' was retained because {retained}",
            facts.branch
        )
    }

    /// Merge a managed worktree's branch into base.
    pub async fn merge_worktree(
        &self,
        path: &Path,
        policy: ConflictPolicy,
    ) -> WorktreeOpResult<WorktreeOpOutcome> {
        let _guard = self.acquire_root()?;

        let facts = self.locate(path).await?;
        // Once a branch is known, every refusal is reported through the same
        // event a backend failure would use, so a frontend that renders merge
        // outcomes from events never has to special-case eligibility.
        if let Err(error) = classify_merge_eligibility(&facts, policy) {
            self.events
                .emit(WorktreeOperationEvent::MergeFailed {
                    branch: facts.branch.clone(),
                    error: error.to_string(),
                })
                .await;
            return Err(error);
        }

        self.events
            .emit(WorktreeOperationEvent::MergeStarted {
                branch: facts.branch.clone(),
            })
            .await;

        let attempt = match self.backend.merge_into_base(&facts.branch, policy).await {
            Ok(attempt) => attempt,
            Err(error) => {
                self.events
                    .emit(WorktreeOperationEvent::MergeFailed {
                        branch: facts.branch.clone(),
                        error: error.to_string(),
                    })
                    .await;
                return Err(error);
            }
        };

        if let MergeAttempt::Conflict { files } = attempt {
            let error = WorktreeOpError::MergeConflict {
                files,
                recovery: RECOVERY_LOCAL_OR_TUI,
            };
            self.events
                .emit(WorktreeOperationEvent::MergeFailed {
                    branch: facts.branch.clone(),
                    error: error.to_string(),
                })
                .await;
            // Deliberately no abort and no `on_merged`: the conflicted index is
            // the evidence a local resolve needs.
            return Err(error);
        }

        // `on_merged` runs after the merge and before completion is announced, so
        // a hook failure blocks the merged transition exactly as it does in TUI.
        if let Some(change_id) =
            crate::vcs::GitWorkspaceManager::extract_change_id_from_worktree_name(&facts.branch)
        {
            if let Err(error) = self.backend.run_on_merged(&change_id, &facts.path).await {
                let message = format!(
                    "on_merged hook failed for '{change_id}'; branch merged transition blocked: {error}"
                );
                self.events
                    .emit(WorktreeOperationEvent::MergeFailed {
                        branch: facts.branch.clone(),
                        error: message.clone(),
                    })
                    .await;
                return Err(WorktreeOpError::Internal(message));
            }
        } else {
            debug!(
                branch = %facts.branch,
                "no change_id could be extracted from the branch name; skipping on_merged"
            );
        }

        self.events
            .emit(WorktreeOperationEvent::MergeCompleted {
                branch: facts.branch.clone(),
            })
            .await;
        self.events.emit(WorktreeOperationEvent::Refreshed).await;

        Ok(WorktreeOpOutcome {
            detail: format!("branch '{}' was merged into base", facts.branch),
            branch: facts.branch,
            path: facts.path,
        })
    }

    /// Take the repository mutation guard, to model a concurrent operation.
    ///
    /// Exists so a test in another module can prove that two frontends sharing
    /// one service really do contend for one guard.
    #[cfg(test)]
    pub fn acquire_root_for_test(&self) -> WorktreeOpResult<tokio::sync::MutexGuard<'_, ()>> {
        self.acquire_root()
    }

    fn acquire_root(&self) -> WorktreeOpResult<tokio::sync::MutexGuard<'_, ()>> {
        self.root_guard.try_lock().map_err(|_| {
            WorktreeOpError::RootBusy(
                "another worktree operation is already mutating this repository".to_string(),
            )
        })
    }

    async fn locate(&self, path: &Path) -> WorktreeOpResult<WorktreeFacts> {
        self.backend
            .observe()
            .await?
            .into_iter()
            .find(|facts| same_path(&facts.path, path))
            .ok_or_else(|| {
                WorktreeOpError::NotFound(
                    "the addressed worktree is not present in the current observation".to_string(),
                )
            })
    }
}

#[cfg(test)]
mod tests;
