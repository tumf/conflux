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
//! * the TUI keeps its local recovery `skip_teardown` escape hatch and its
//!   documented fail-open behavior when dirty state cannot be observed;
//! * `/api/v2` is fail-closed on both and preserves a conflicted merge instead
//!   of aborting it, because a remote client has no way to inspect or resolve
//!   the intermediate state.

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
    /// True when this branch has commits the base does not have.
    pub has_commits_ahead: bool,
    /// Repository-relative paths that would conflict with a base merge.
    pub conflict_files: Vec<String>,
    /// Uncommitted-change observation.
    pub dirty: DirtyState,
    /// True when the base repository is sitting on an unresolved merge.
    pub base_merge_in_progress: bool,
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
            has_commits_ahead: false,
            conflict_files: Vec::new(),
            dirty: DirtyState::Clean,
            base_merge_in_progress: false,
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
    Dirty(String),
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
            | Self::Dirty(message)
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
    /// Treat an unobservable dirty state as permission to delete.
    ///
    /// The TUI keeps this for cleanup parity with its previous behavior.
    /// `/api/v2` sets it to `false`: observation failure cannot become permission.
    pub allow_unknown_dirty: bool,
}

impl DeleteOptions {
    /// The fail-closed policy every remote caller uses.
    pub fn fail_closed() -> Self {
        Self {
            skip_teardown: false,
            allow_unknown_dirty: false,
        }
    }

    /// The local TUI policy, including its recovery escape hatch.
    pub fn local(skip_teardown: bool) -> Self {
        Self {
            skip_teardown,
            allow_unknown_dirty: true,
        }
    }
}

// ============================================================================
// Eligibility classification (pure)
// ============================================================================

/// Decide whether a worktree may be deleted under the caller's policy.
pub fn classify_delete_eligibility(
    facts: &WorktreeFacts,
    options: DeleteOptions,
) -> WorktreeOpResult<()> {
    if facts.is_main {
        return Err(WorktreeOpError::Ineligible(
            "the main worktree cannot be deleted".to_string(),
        ));
    }
    if facts.base_merge_in_progress {
        return Err(WorktreeOpError::RootBusy(format!(
            "the repository root is holding an unresolved merge; recovery: {RECOVERY_LOCAL_OR_TUI}"
        )));
    }
    match facts.dirty {
        DirtyState::Dirty => {
            return Err(WorktreeOpError::Dirty(
                "the worktree has uncommitted changes".to_string(),
            ))
        }
        DirtyState::Unknown if !options.allow_unknown_dirty => {
            return Err(WorktreeOpError::DirtyUnknown(
                "the worktree's uncommitted-change state could not be determined".to_string(),
            ))
        }
        DirtyState::Unknown | DirtyState::Clean => {}
    }
    if facts.has_commits_ahead {
        return Err(WorktreeOpError::Ineligible(
            "the worktree has unmerged commits ahead of base".to_string(),
        ));
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
    if facts.base_merge_in_progress {
        return Err(WorktreeOpError::RootBusy(format!(
            "the repository root is holding an unresolved merge; recovery: {RECOVERY_LOCAL_OR_TUI}"
        )));
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
    if !facts.has_commits_ahead {
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
    /// Remove a worktree, running `.wt/teardown` unless explicitly skipped.
    async fn remove(&self, path: &Path, skip_teardown: bool) -> WorktreeOpResult<()>;
    /// Delete a branch. Best-effort; failure is reported but not fatal.
    async fn delete_branch(&self, branch: &str) -> WorktreeOpResult<()>;
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

    /// Delete a managed worktree after mandatory teardown.
    pub async fn delete_worktree(
        &self,
        path: &Path,
        options: DeleteOptions,
    ) -> WorktreeOpResult<WorktreeOpOutcome> {
        let _guard = self.acquire_root()?;

        let facts = self.locate(path).await?;
        classify_delete_eligibility(&facts, options)?;

        // Teardown must succeed before the resource is retired: a failed delete
        // leaves both the worktree and its identity binding intact.
        self.backend
            .remove(&facts.path, options.skip_teardown)
            .await?;

        if !facts.branch.is_empty() {
            if let Err(error) = self.backend.delete_branch(&facts.branch).await {
                warn!(
                    branch = %facts.branch,
                    "branch deletion failed after worktree removal: {error} (non-fatal)"
                );
            }
        }

        self.events
            .emit(WorktreeOperationEvent::Deleted {
                branch: facts.branch.clone(),
            })
            .await;
        self.events.emit(WorktreeOperationEvent::Refreshed).await;

        Ok(WorktreeOpOutcome {
            detail: format!(
                "worktree on branch '{}' was torn down and removed",
                facts.branch
            ),
            branch: facts.branch,
            path: facts.path,
        })
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
