//! Opaque worktree identity, redacted worktree DTOs, and the `/api/v2` worktree
//! operation port.
//!
//! Three things are deliberately separated here:
//!
//! * [`WorktreeRegistry`] — the *only* thing a remote client may use to address a
//!   worktree. IDs are random, process-local, allocated on first observation,
//!   and retired when the resource disappears. A path, a branch, or a repository
//!   correlation ID is never accepted as mutation identity, so a stale ID cannot
//!   land on a different resource that happens to occupy the same directory.
//! * The DTOs — repository-relative display paths and a *correlation* repository
//!   ID. Neither the canonical nor the absolute repository root is serialized.
//! * [`WorktreeOperations`] — the late-bound port the read routes and the command
//!   executor both call, so remote reads and remote mutations agree about what
//!   exists.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::worktree_ops::service::{
    classify_delete_eligibility, classify_merge_eligibility, ConflictPolicy, DeleteOptions,
    WorktreeFacts, WorktreeOpError, WorktreeService, RECOVERY_LOCAL_OR_TUI,
};

use super::dto::{new_hex_id, ErrorCode};
use super::executor::{CommandFailure, ExecutionSummary};

/// The worktree operations `/api/v2` advertises. Nothing else is reachable.
pub const WORKTREE_OPERATIONS: [&str; 5] = ["list", "detail", "create", "delete", "merge"];

// ============================================================================
// Opaque process-local identity
// ============================================================================

/// What a binding is anchored to.
///
/// Both the path and the Git identity participate: a directory that is torn down
/// and rebuilt is a different resource even when it lands on the same path, and
/// treating it as the same one is exactly the stale-targeting bug the opaque ID
/// exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorktreeKey {
    /// Absolute filesystem path. Registry-internal; never serialized.
    pub path: PathBuf,
    /// Git worktree identity (`gitdir:` pointer).
    pub identity: String,
}

impl WorktreeKey {
    /// Build a key from an observation.
    pub fn from_facts(facts: &WorktreeFacts) -> Self {
        Self {
            path: facts.path.clone(),
            identity: facts.identity.clone(),
        }
    }
}

#[derive(Default)]
struct RegistryInner {
    by_id: HashMap<String, WorktreeKey>,
    by_key: HashMap<WorktreeKey, String>,
    retired: HashSet<String>,
}

/// Process-local allocation of opaque worktree IDs.
#[derive(Default)]
pub struct WorktreeRegistry {
    inner: Mutex<RegistryInner>,
}

impl WorktreeRegistry {
    /// A registry with no bindings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconcile the registry with a complete current observation.
    ///
    /// Returns the ID for each observed key in the order given. Keys that were
    /// bound and are absent from this observation are retired; their IDs are
    /// never handed out again.
    pub fn sync(&self, observed: &[WorktreeKey]) -> Vec<String> {
        let mut inner = self.lock();

        let present: HashSet<&WorktreeKey> = observed.iter().collect();
        let disappeared: Vec<WorktreeKey> = inner
            .by_key
            .keys()
            .filter(|key| !present.contains(*key))
            .cloned()
            .collect();
        for key in disappeared {
            if let Some(id) = inner.by_key.remove(&key) {
                inner.by_id.remove(&id);
                inner.retired.insert(id);
            }
        }

        observed
            .iter()
            .map(|key| match inner.by_key.get(key) {
                Some(id) => id.clone(),
                None => {
                    let id = Self::allocate(&inner.retired);
                    inner.by_key.insert(key.clone(), id.clone());
                    inner.by_id.insert(id.clone(), key.clone());
                    id
                }
            })
            .collect()
    }

    /// Resolve a live ID to the resource it is bound to.
    pub fn resolve(&self, worktree_id: &str) -> Option<WorktreeKey> {
        self.lock().by_id.get(worktree_id).cloned()
    }

    /// Retire an ID after its resource was successfully removed.
    pub fn retire(&self, worktree_id: &str) {
        let mut inner = self.lock();
        if let Some(key) = inner.by_id.remove(worktree_id) {
            inner.by_key.remove(&key);
        }
        inner.retired.insert(worktree_id.to_string());
    }

    /// True when this ID was allocated and can no longer address anything.
    pub fn is_retired(&self, worktree_id: &str) -> bool {
        self.lock().retired.contains(worktree_id)
    }

    /// Number of live bindings.
    // Observed by tests; the binary crate recompiles this tree and sees no caller.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.lock().by_id.len()
    }

    /// True when nothing is bound.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn allocate(retired: &HashSet<String>) -> String {
        // 128 random bits make a collision vanishingly unlikely; the loop makes
        // "never reused" a property of the code rather than of the odds.
        loop {
            let id = new_hex_id();
            if !retired.contains(&id) {
                return id;
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RegistryInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

// ============================================================================
// Redaction
// ============================================================================

/// 64-bit FNV-1a offset basis.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// 64-bit FNV-1a prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Correlation ID for a repository: 16 lowercase hexadecimal characters.
///
/// This lets an authenticated client group resources without the response ever
/// carrying the canonical repository root. It is **not** a secret and **not** an
/// authorization or mutation input: FNV-1a is neither collision- nor
/// inference-resistant, so a client that already guesses a likely path can
/// confirm it. Mutation identity is the opaque worktree ID and nothing else.
pub fn repository_correlation_id(repo_root: &Path) -> String {
    let identity = crate::parallel::acceptance_state::repository_identity(repo_root);
    let mut hash = FNV_OFFSET_BASIS;
    for byte in identity.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// Repository-relative display path for a worktree.
///
/// Purely lexical over the two inputs, with `/` separators so the value does not
/// change shape per platform. Managed worktrees usually live outside the
/// repository, so an escaping result is normal and expected — what matters is
/// that the canonical repository root itself is never part of the value.
pub fn repository_relative_display(repo_root: &Path, target: &Path) -> String {
    let root: Vec<Component<'_>> = repo_root
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();
    let path: Vec<Component<'_>> = target
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();

    let shared = root
        .iter()
        .zip(path.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut parts: Vec<String> =
        std::iter::repeat_n("..".to_string(), root.len() - shared).collect();
    parts.extend(
        path[shared..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().to_string()),
    );

    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

// ============================================================================
// DTOs
// ============================================================================

/// Conflict evidence attached to a worktree or a failed merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorktreeConflict {
    /// Repository-relative conflicted paths.
    pub files: Vec<String>,
    /// How the conflict must be resolved. Always `local_or_tui_required`.
    pub recovery: String,
}

impl WorktreeConflict {
    /// Build conflict evidence with the only recovery path v2 offers.
    pub fn new(files: Vec<String>) -> Self {
        Self {
            files,
            recovery: RECOVERY_LOCAL_OR_TUI.to_string(),
        }
    }
}

/// Which operations this worktree can currently accept, and why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorktreeEligibility {
    /// True when `delete_worktree` would pass its guards right now.
    pub deletable: bool,
    /// True when `merge_worktree` would pass its guards right now.
    pub mergeable: bool,
    /// Why deletion is refused, when it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_blocked_reason: Option<String>,
    /// Why merging is refused, when it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_blocked_reason: Option<String>,
}

/// One worktree as `/api/v2` exposes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorktreeResource {
    /// Random 128-bit process-local hexadecimal ID. The only mutation identity.
    pub worktree_id: String,
    /// 16-hex FNV-1a repository correlation ID. Not a secret, not authorization.
    pub repository_id: String,
    /// Repository-relative display path. Never the canonical or absolute root.
    pub path: String,
    /// Checked-out branch, empty when detached.
    pub branch: String,
    /// Current HEAD commit.
    pub head: String,
    /// True for the repository's main worktree.
    pub is_main: bool,
    /// True when HEAD is detached.
    pub is_detached: bool,
    /// Uncommitted-change state. `null` means it could not be determined, which
    /// makes the worktree undeletable rather than deletable.
    pub dirty: Option<bool>,
    /// True when this branch has commits base does not have.
    pub has_commits_ahead: bool,
    /// Conflict evidence, when a base merge would conflict or already has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<WorktreeConflict>,
    /// Current operation eligibility.
    pub operations: WorktreeEligibility,
}

impl WorktreeResource {
    /// Project an observation into the redacted remote resource.
    pub fn project(
        worktree_id: String,
        repository_id: String,
        repo_root: &Path,
        facts: &WorktreeFacts,
    ) -> Self {
        let delete = classify_delete_eligibility(facts, DeleteOptions::fail_closed());
        let merge = classify_merge_eligibility(facts, ConflictPolicy::PreserveConflict);
        Self {
            worktree_id,
            repository_id,
            path: repository_relative_display(repo_root, &facts.path),
            branch: facts.branch.clone(),
            head: facts.head.clone(),
            is_main: facts.is_main,
            is_detached: facts.is_detached,
            dirty: facts.dirty.as_option(),
            has_commits_ahead: facts.has_commits_ahead,
            conflict: if facts.conflict_files.is_empty() {
                None
            } else {
                Some(WorktreeConflict::new(facts.conflict_files.clone()))
            },
            operations: WorktreeEligibility {
                deletable: delete.is_ok(),
                mergeable: merge.is_ok(),
                delete_blocked_reason: delete.err().map(|e| e.to_string()),
                merge_blocked_reason: merge.err().map(|e| e.to_string()),
            },
        }
    }
}

/// `GET /api/v2/worktrees` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorktreesResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the list was read at.
    pub state_revision: u64,
    /// Repository correlation ID shared by every listed worktree.
    pub repository_id: String,
    /// Current worktrees.
    pub worktrees: Vec<WorktreeResource>,
}

/// `GET /api/v2/worktrees/{worktree_id}` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorktreeResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the worktree was read at.
    pub state_revision: u64,
    /// The requested worktree.
    pub worktree: WorktreeResource,
}

/// Worktree section of `GET /api/v2/capabilities`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorktreeCapabilities {
    /// Advertised operations. Nothing outside this list is reachable.
    pub operations: Vec<String>,
    /// How a merge conflict is recovered from. Always `local_or_tui_required`.
    pub merge_conflict_recovery: String,
    /// True when a conflicting merge leaves the intermediate Git state in place.
    pub merge_conflict_preserves_state: bool,
    /// True when deletion always runs managed teardown with no bypass.
    pub delete_requires_teardown: bool,
}

impl Default for WorktreeCapabilities {
    fn default() -> Self {
        Self {
            operations: WORKTREE_OPERATIONS.iter().map(|o| o.to_string()).collect(),
            merge_conflict_recovery: RECOVERY_LOCAL_OR_TUI.to_string(),
            merge_conflict_preserves_state: true,
            delete_requires_teardown: true,
        }
    }
}

// ============================================================================
// Operation port
// ============================================================================

/// The late-bound worktree port shared by the v2 read routes and the executor.
#[async_trait]
pub trait WorktreeOperations: Send + Sync {
    /// Current worktrees with their live opaque IDs.
    async fn list(&self) -> Result<WorktreeListing, CommandFailure>;
    /// Create the managed worktree for an eligible change.
    async fn create(&self, change_id: &str) -> Result<ExecutionSummary, CommandFailure>;
    /// Delete a worktree addressed by its opaque ID.
    async fn delete(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure>;
    /// Merge a worktree addressed by its opaque ID into base.
    async fn merge(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure>;
}

/// A complete worktree observation plus the repository it belongs to.
#[derive(Debug, Clone)]
pub struct WorktreeListing {
    /// Repository correlation ID.
    pub repository_id: String,
    /// Current worktrees.
    pub worktrees: Vec<WorktreeResource>,
}

/// The port before an orchestration runtime has bound a real one.
pub struct UnboundWorktreeOperations;

fn unbound() -> CommandFailure {
    CommandFailure::new(
        ErrorCode::LifecycleConflict,
        "this instance has no worktree runtime bound yet",
    )
}

#[async_trait]
impl WorktreeOperations for UnboundWorktreeOperations {
    async fn list(&self) -> Result<WorktreeListing, CommandFailure> {
        Err(unbound())
    }
    async fn create(&self, _change_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        Err(unbound())
    }
    async fn delete(&self, _worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        Err(unbound())
    }
    async fn merge(&self, _worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        Err(unbound())
    }
}

/// Map a shared-service refusal onto a v2 error code.
pub fn map_worktree_error(error: &WorktreeOpError) -> CommandFailure {
    let code = match error {
        WorktreeOpError::NotFound(_) => ErrorCode::WorktreeNotFound,
        WorktreeOpError::Exists(_) => ErrorCode::WorktreeExists,
        WorktreeOpError::Dirty(_) => ErrorCode::WorktreeDirty,
        WorktreeOpError::DirtyUnknown(_) => ErrorCode::WorktreeDirtyUnknown,
        WorktreeOpError::Ineligible(_) => ErrorCode::TargetIneligible,
        WorktreeOpError::RootBusy(_) => ErrorCode::RootBusy,
        WorktreeOpError::MergeConflict { .. } => ErrorCode::MergeConflict,
        WorktreeOpError::Internal(_) => ErrorCode::InternalError,
    };
    CommandFailure::new(code, error.to_string())
}

/// Production port: the shared operation service plus this process's ID registry.
pub struct RemoteWorktreeOperations {
    service: Arc<WorktreeService>,
    registry: Arc<WorktreeRegistry>,
    repo_root: PathBuf,
}

impl RemoteWorktreeOperations {
    /// Wire the port to the shared service and a fresh identity registry.
    pub fn new(
        service: Arc<WorktreeService>,
        registry: Arc<WorktreeRegistry>,
        repo_root: PathBuf,
    ) -> Self {
        Self {
            service,
            registry,
            // Git reports canonical worktree paths. Relativizing against a
            // non-canonical root would produce a long `../` escape that walks
            // back out through the real root — the opposite of redaction — so
            // both sides are put in the same namespace once, here.
            repo_root: std::fs::canonicalize(&repo_root).unwrap_or(repo_root),
        }
    }

    /// Observe, reconcile identity, and project — the one path every read and
    /// every post-mutation refresh goes through.
    async fn observe(&self) -> Result<(WorktreeListing, Vec<WorktreeFacts>), CommandFailure> {
        let facts = self
            .service
            .observe()
            .await
            .map_err(|error| map_worktree_error(&error))?;
        let keys: Vec<WorktreeKey> = facts.iter().map(WorktreeKey::from_facts).collect();
        let ids = self.registry.sync(&keys);
        let repository_id = repository_correlation_id(&self.repo_root);

        let worktrees = ids
            .iter()
            .zip(facts.iter())
            .map(|(id, facts)| {
                WorktreeResource::project(id.clone(), repository_id.clone(), &self.repo_root, facts)
            })
            .collect();

        Ok((
            WorktreeListing {
                repository_id,
                worktrees,
            },
            facts,
        ))
    }

    /// Resolve an opaque ID, distinguishing "never existed" from "retired".
    fn resolve(&self, worktree_id: &str) -> Result<WorktreeKey, CommandFailure> {
        self.registry.resolve(worktree_id).ok_or_else(|| {
            let detail = if self.registry.is_retired(worktree_id) {
                "was retired when its worktree disappeared"
            } else {
                "is not known to this instance"
            };
            CommandFailure::new(
                ErrorCode::WorktreeNotFound,
                format!("worktree '{worktree_id}' {detail}"),
            )
        })
    }
}

#[async_trait]
impl WorktreeOperations for RemoteWorktreeOperations {
    async fn list(&self) -> Result<WorktreeListing, CommandFailure> {
        Ok(self.observe().await?.0)
    }

    async fn create(&self, change_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        let created = self
            .service
            .create_change_worktree(change_id)
            .await
            .map_err(|error| map_worktree_error(&error))?;

        // Identity is allocated only after creation and refresh, so the ID a
        // client receives always names a resource that actually exists.
        let (listing, _) = self.observe().await?;
        let created_path = repository_relative_display(&self.repo_root, &created.path);
        let worktree_id = listing
            .worktrees
            .iter()
            .find(|resource| resource.path == created_path)
            .map(|resource| resource.worktree_id.clone())
            .ok_or_else(|| {
                CommandFailure::new(
                    ErrorCode::InternalError,
                    "the created worktree could not be observed for identity allocation",
                )
            })?;

        Ok(ExecutionSummary::changed(format!(
            "worktree for change '{change_id}' created as worktree_id {worktree_id}"
        )))
    }

    async fn delete(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        let key = self.resolve(worktree_id)?;
        let outcome = self
            .service
            // Fail-closed on purpose: teardown is mandatory and an unobservable
            // dirty state refuses. Neither is a client-supplied parameter.
            .delete_worktree(&key.path, DeleteOptions::fail_closed())
            .await
            .map_err(|error| map_worktree_error(&error))?;

        self.registry.retire(worktree_id);
        let _ = self.observe().await;

        Ok(ExecutionSummary::changed(format!(
            "{}; worktree_id {worktree_id} retired",
            outcome.detail
        )))
    }

    async fn merge(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        let key = self.resolve(worktree_id)?;
        let outcome = self
            .service
            .merge_worktree(&key.path, ConflictPolicy::PreserveConflict)
            .await
            .map_err(|error| map_worktree_error(&error))?;

        let _ = self.observe().await;
        Ok(ExecutionSummary::changed(outcome.detail))
    }
}
