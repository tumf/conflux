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

use crate::worktree_ops::consent::{
    ConfirmedDeleteReason, ContentObservationKind, DeleteConsent, CONFIRMED_DELETE_PERMISSIONS,
    CONSENT_FIELDS, CONTENT_ID_ALGORITHM,
};
use crate::worktree_ops::service::{
    classify_delete_eligibility, classify_merge_eligibility, ConflictPolicy, DeleteOptions,
    ExpectedTarget, OptionEligibility, WorktreeFacts, WorktreeOpError, WorktreeService,
    RECOVERY_LOCAL_OR_TUI,
};

use super::dto::{
    new_hex_id, CommandResult, ConfirmedDeleteParams, ConfirmedDeleteRefusalResult,
    ConfirmedDeleteResult, ErrorCode, MergeAbortEvidence, MergeAbortOutcome, MergeAbortResult,
};
use super::executor::{CommandFailure, ExecutionSummary};

/// The worktree operations `/api/v2` advertises. Nothing else is reachable.
pub const WORKTREE_OPERATIONS: [&str; 9] = [
    "list",
    "detail",
    "create",
    "delete",
    "merge",
    "merge_abort",
    "delete_confirmed",
    "text",
    "session",
];

/// Command the `delete_confirmed` operation is submitted as.
pub const CONFIRMED_DELETE_COMMAND: &str = "delete_worktree_confirmed";

/// Route template of the read-only delete preview.
pub const DELETE_PREVIEW_ROUTE: &str = "/api/v2/worktrees/{worktree_id}/delete-preview";

/// Command the `merge_abort` operation is submitted as.
pub const MERGE_ABORT_COMMAND: &str = "merge_worktree_abort_on_conflict";

/// The three repository facts a confirmed restoration requires.
pub const MERGE_ABORT_RESTORE_VERIFICATION: [&str; 3] =
    ["head_equals_premerge", "no_merge_head", "root_clean"];

/// Every stable reason an abort-on-conflict merge can settle with besides `merged`.
pub const MERGE_ABORT_REASONS: [&str; 4] = [
    "predicted_conflict",
    "conflict_aborted",
    "abort_failed",
    "restore_unknown",
];

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
    // Deliberately not a doc comment: the published description is part of the
    // v2 contract. An unobservable commits-ahead state reads as `false` here —
    // it is not evidence *for* the condition — and is reported where it matters,
    // as an undeletable `operations.deletable` with a reason.
    pub has_commits_ahead: bool,
    /// How the two fields above were obtained.
    ///
    /// `not_inspected` means periodic refresh deliberately spent no Git
    /// commands on this worktree, so `has_commits_ahead: false` and an absent
    /// `conflict` are the *absence* of evidence rather than evidence of
    /// absence. A client that renders either fact reads this first; the
    /// operations below already fail closed on it.
    #[serde(default)]
    pub inspection: crate::worktree_ops::InspectionState,
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
            has_commits_ahead: facts.has_commits_ahead.is_known_yes(),
            inspection: facts.inspection,
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

/// Whether the abort-on-conflict merge is served, and what it verifies.
///
/// `Default` is the *unavailable* answer, which is what `#[serde(default)]`
/// decodes an older owner's omission as.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct MergeAbortCapability {
    /// True when `merge_worktree_abort_on_conflict` is accepted.
    pub available: bool,
    /// Command type the operation is submitted as.
    pub command: String,
    /// True when a conflict the fresh inspection predicts is refused before
    /// any Git mutation.
    pub predicted_conflict_refused: bool,
    /// Repository facts a `confirmed` restoration requires, all of them.
    pub restore_verification: Vec<String>,
    /// Every stable non-success settlement reason.
    pub reasons: Vec<String>,
}

impl MergeAbortCapability {
    /// The capability this build serves.
    pub fn served() -> Self {
        Self {
            available: true,
            command: MERGE_ABORT_COMMAND.to_string(),
            predicted_conflict_refused: true,
            restore_verification: MERGE_ABORT_RESTORE_VERIFICATION
                .iter()
                .map(|v| v.to_string())
                .collect(),
            reasons: MERGE_ABORT_REASONS.iter().map(|r| r.to_string()).collect(),
        }
    }
}

/// Whether consent-bound destructive deletion is served, and how to use it.
///
/// `Default` is the *unavailable* answer, which is what `#[serde(default)]`
/// decodes an older owner's omission as.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct ConfirmedDeleteCapability {
    /// True when `delete_worktree_confirmed` and its preview are served.
    pub available: bool,
    /// Command type the operation is submitted as.
    pub command: String,
    /// Route template of the read-only preview whose `consent` is echoed.
    pub preview_route: String,
    /// The three independent permissions the command accepts.
    pub permissions: Vec<String>,
    /// Exactly the fields of the `consent` object.
    pub consent_fields: Vec<String>,
    /// Hash algorithm both content identities use.
    pub content_id_algorithm: String,
    /// True when ignored content is disclosed and bound by its bytes.
    pub binds_ignored_content: bool,
    /// Every stable refusal and blocked-option reason.
    pub reasons: Vec<String>,
}

impl ConfirmedDeleteCapability {
    /// The capability this build serves.
    pub fn served() -> Self {
        Self {
            available: true,
            command: CONFIRMED_DELETE_COMMAND.to_string(),
            preview_route: DELETE_PREVIEW_ROUTE.to_string(),
            permissions: CONFIRMED_DELETE_PERMISSIONS
                .iter()
                .map(|p| p.to_string())
                .collect(),
            consent_fields: CONSENT_FIELDS.iter().map(|f| f.to_string()).collect(),
            content_id_algorithm: CONTENT_ID_ALGORITHM.to_string(),
            binds_ignored_content: true,
            reasons: ConfirmedDeleteReason::ALL
                .iter()
                .map(|r| r.as_str().to_string())
                .collect(),
        }
    }
}

/// Worktree section of `GET /api/v2/capabilities`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorktreeCapabilities {
    /// Advertised operations. Nothing outside this list is reachable.
    pub operations: Vec<String>,
    /// How a `merge_worktree` conflict is recovered from. Always `local_or_tui_required`.
    pub merge_conflict_recovery: String,
    /// True when a conflicting `merge_worktree` leaves the intermediate Git state in place; the separately advertised `merge_abort` command aborts instead.
    pub merge_conflict_preserves_state: bool,
    /// True when the `delete_worktree` command always runs managed teardown with no bypass. The only bypass is the separately advertised `confirmed_delete` command, which requires explicit consent.
    pub delete_requires_teardown: bool,
    /// The explicit abort-on-conflict merge.
    ///
    /// `#[serde(default)]` so a client can still read an older owner, where the
    /// absent field means "unavailable".
    #[serde(default)]
    pub merge_abort: MergeAbortCapability,
    /// Consent-bound destructive deletion.
    ///
    /// `#[serde(default)]` so a client can still read an older owner, where the
    /// absent field means "unavailable".
    #[serde(default)]
    pub confirmed_delete: ConfirmedDeleteCapability,
}

impl Default for WorktreeCapabilities {
    fn default() -> Self {
        Self {
            operations: WORKTREE_OPERATIONS.iter().map(|o| o.to_string()).collect(),
            merge_conflict_recovery: RECOVERY_LOCAL_OR_TUI.to_string(),
            merge_conflict_preserves_state: true,
            delete_requires_teardown: true,
            merge_abort: MergeAbortCapability::served(),
            confirmed_delete: ConfirmedDeleteCapability::served(),
        }
    }
}

/// One option's eligibility on a delete preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DeleteOptionEligibility {
    /// True when the command with this option would pass its guards now.
    pub eligible: bool,
    /// Why it would not; `null` when eligible.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub blocked_reason: Option<ConfirmedDeleteReason>,
    /// Paths in the discardable identity (`discard_known_dirty` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<usize>,
    /// Commits between the merge base and HEAD (`discard_commits_ahead` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commits: Option<usize>,
    /// True when the ahead option also discards known dirty or ignored
    /// content (`discard_commits_ahead` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub also_discards_dirty: Option<bool>,
}

impl DeleteOptionEligibility {
    fn from_result(result: &OptionEligibility) -> Self {
        Self {
            eligible: result.is_ok(),
            blocked_reason: result.err(),
            entries: None,
            commits: None,
            also_discards_dirty: None,
        }
    }
}

/// Per-option eligibility of a delete preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DeletePreviewOptions {
    /// All three permissions `false`: an ordinary delete with consent revalidation.
    pub ordinary: DeleteOptionEligibility,
    /// `skip_teardown` alone.
    pub skip_teardown: DeleteOptionEligibility,
    /// `discard_known_dirty` alone.
    pub discard_known_dirty: DeleteOptionEligibility,
    /// `discard_commits_ahead`, with `discard_known_dirty` when anything is dirty.
    pub discard_commits_ahead: DeleteOptionEligibility,
}

/// `GET /api/v2/worktrees/{worktree_id}/delete-preview` body.
///
/// A read: it creates no command record and advances no revision. `consent`
/// is exactly what `delete_worktree_confirmed` must be sent, unmodified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DeletePreviewResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the preview was read at.
    pub state_revision: u64,
    /// The addressed opaque worktree ID.
    pub worktree_id: String,
    /// When the observation was taken (RFC 3339). Informational; not consent.
    pub observed_at: String,
    /// Branch, empty when detached.
    pub branch: String,
    /// Current HEAD commit.
    pub head: String,
    /// True for the main worktree, which is never deletable.
    pub is_main: bool,
    /// Uncommitted-change state (`--ignored=no`); `null` when unobservable.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub dirty: Option<bool>,
    /// Commits-ahead state; `null` when unobservable.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub has_commits_ahead: Option<bool>,
    /// How the ahead fact was obtained. Always a fresh targeted inspection.
    pub inspection: crate::worktree_ops::InspectionState,
    /// How the content observation came out.
    pub content_observation: ContentObservationKind,
    /// True when forced removal would also delete ignored content; `null`
    /// unless content was observed.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub has_ignored_content: Option<bool>,
    /// Paths in the ignored identity; `null` unless content was observed.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub ignored_entries: Option<usize>,
    /// True exactly when teardown would execute `.wt/teardown`.
    pub teardown_script_present: bool,
    /// What each permission combination would decide right now.
    pub options: DeletePreviewOptions,
    /// The object to echo as `params.consent`.
    pub consent: DeleteConsent,
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
    /// Merge a worktree into base, refusing a predicted conflict and aborting
    /// a real one with verified restoration.
    async fn merge_abort_on_conflict(
        &self,
        worktree_id: &str,
    ) -> Result<ExecutionSummary, CommandFailure>;
    /// Observe what a confirmed delete would act on. A read.
    ///
    /// `instance_id` and `state_revision` are the read route's to fill in.
    async fn delete_preview(
        &self,
        worktree_id: &str,
    ) -> Result<DeletePreviewResponse, CommandFailure>;
    /// Delete a worktree on explicit, re-verified consent.
    async fn delete_confirmed(
        &self,
        worktree_id: &str,
        params: &ConfirmedDeleteParams,
    ) -> Result<ExecutionSummary, CommandFailure>;

    /// The directory a live opaque ID is bound to, for the text surface.
    ///
    /// Registry-internal: the path is never serialized.
    async fn text_root(&self, _worktree_id: &str) -> Result<PathBuf, CommandFailure> {
        Err(unbound())
    }

    /// Whether the repository root mutation guard is held right now.
    ///
    /// A port with no service behind it holds no guard.
    async fn root_guard_is_held(&self) -> bool {
        false
    }

    /// Refresh the worktree observation after a text write, so `dirty` and the
    /// `Refreshed` worktree event follow from the file write alone.
    async fn refresh_after_text_write(&self) {}
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
    async fn merge_abort_on_conflict(
        &self,
        _worktree_id: &str,
    ) -> Result<ExecutionSummary, CommandFailure> {
        Err(unbound())
    }
    async fn delete_preview(
        &self,
        _worktree_id: &str,
    ) -> Result<DeletePreviewResponse, CommandFailure> {
        Err(unbound())
    }
    async fn delete_confirmed(
        &self,
        _worktree_id: &str,
        _params: &ConfirmedDeleteParams,
    ) -> Result<ExecutionSummary, CommandFailure> {
        Err(unbound())
    }
}

/// Map a shared-service refusal onto a v2 error code.
pub fn map_worktree_error(error: &WorktreeOpError) -> CommandFailure {
    let code = match error {
        WorktreeOpError::NotFound(_) => ErrorCode::WorktreeNotFound,
        WorktreeOpError::Exists(_) => ErrorCode::WorktreeExists,
        WorktreeOpError::Dirty { .. } => ErrorCode::WorktreeDirty,
        WorktreeOpError::DirtyUnknown(_) => ErrorCode::WorktreeDirtyUnknown,
        // Deliberately the same code an ahead worktree already produced before
        // the refusal became typed. The type exists so the *local* TUI can open
        // a confirmation; a remote client has no such path, and a new code would
        // advertise one.
        WorktreeOpError::CommitsAhead { .. } | WorktreeOpError::Ineligible(_) => {
            ErrorCode::TargetIneligible
        }
        WorktreeOpError::RootBusy(_) => ErrorCode::RootBusy,
        WorktreeOpError::MergeConflict { .. } => ErrorCode::MergeConflict,
        WorktreeOpError::PredictedConflict { .. } => ErrorCode::MergeConflictPredicted,
        // Only a confirmed restoration is `merge_conflict_aborted`. A failed or
        // unreadable verification is never reported as a successful abort.
        WorktreeOpError::MergeConflictAborted { abort, .. } => match abort.restored {
            crate::vcs::git::commands::Restoration::Confirmed => ErrorCode::MergeConflictAborted,
            crate::vcs::git::commands::Restoration::Failed
            | crate::vcs::git::commands::Restoration::Unknown => ErrorCode::MergeAbortFailed,
        },
        WorktreeOpError::Internal(_) => ErrorCode::InternalError,
    };
    CommandFailure::new(code, error.to_string())
}

/// Typed `merge_abort` evidence for a refusal, when the refusal has any.
///
/// `None` for refusals before eligibility settled (root busy, not found,
/// ineligible): those carry no merge-abort facts to report.
pub fn merge_abort_failure_result(
    worktree_id: &str,
    branch: &str,
    error: &WorktreeOpError,
) -> Option<MergeAbortResult> {
    use crate::vcs::git::commands::Restoration;
    match error {
        WorktreeOpError::PredictedConflict { files } => Some(MergeAbortResult {
            worktree_id: worktree_id.to_string(),
            branch: branch.to_string(),
            outcome: MergeAbortOutcome::PredictedConflict,
            premerge_head: None,
            conflict_files: files.clone(),
            abort: None,
            recovery: None,
        }),
        WorktreeOpError::MergeConflictAborted { files, abort } => {
            let confirmed = abort.restored == Restoration::Confirmed;
            Some(MergeAbortResult {
                worktree_id: worktree_id.to_string(),
                branch: branch.to_string(),
                outcome: if confirmed {
                    MergeAbortOutcome::ConflictAborted
                } else {
                    MergeAbortOutcome::AbortFailed
                },
                premerge_head: Some(abort.premerge_head.clone()),
                conflict_files: files.clone(),
                abort: Some(MergeAbortEvidence::from_report(abort)),
                recovery: (!confirmed).then(|| RECOVERY_LOCAL_OR_TUI.to_string()),
            })
        }
        _ => None,
    }
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

    /// Resolve a live opaque ID against a fresh observation, returning the
    /// internal key and the redacted resource it currently projects to.
    pub(crate) async fn resolve_live(
        &self,
        worktree_id: &str,
    ) -> Result<(WorktreeKey, WorktreeResource), CommandFailure> {
        let (listing, _) = self.observe().await?;
        let key = self.resolve(worktree_id)?;
        let resource = listing
            .worktrees
            .into_iter()
            .find(|resource| resource.worktree_id == worktree_id)
            .ok_or_else(|| {
                CommandFailure::new(
                    ErrorCode::WorktreeNotFound,
                    format!("worktree '{worktree_id}' is not a current resource in this instance"),
                )
            })?;
        Ok((key, resource))
    }

    /// The opaque ID a fresh observation allocates for `path`, if it is a
    /// registered worktree.
    pub(crate) async fn identify(&self, path: &Path) -> Option<String> {
        let (listing, _) = self.observe().await.ok()?;
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let display = repository_relative_display(&self.repo_root, &canonical);
        listing
            .worktrees
            .into_iter()
            .find(|resource| resource.path == display)
            .map(|resource| resource.worktree_id)
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
            // Fail-closed on purpose: teardown is mandatory, a known-dirty
            // worktree refuses, and an unobservable safety fact refuses. None of
            // the three is a client-supplied parameter, and there is no remote
            // escalation path that could make one so.
            //
            // The confirmed identity is the registry key's Git identity: an
            // opaque ID is bound to a `(path, identity)` pair, so revalidating
            // the identity under the service's own mutation guard is what makes
            // "this ID still addresses that resource" true at mutation time and
            // not merely at listing time.
            .delete_worktree(
                &key.path,
                &ExpectedTarget::unchecked().with_identity(key.identity.clone()),
                DeleteOptions::fail_closed(),
            )
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

    async fn merge_abort_on_conflict(
        &self,
        worktree_id: &str,
    ) -> Result<ExecutionSummary, CommandFailure> {
        let key = self.resolve(worktree_id)?;
        let result = self
            .service
            .merge_worktree(&key.path, ConflictPolicy::AbortOnConflict)
            .await;

        // Refresh after every outcome, so `root_busy` and conflict facts a
        // failed abort left behind are what the next read reports.
        let refreshed = self.observe().await.ok();
        match result {
            Ok(outcome) => Ok(
                ExecutionSummary::changed(outcome.detail.clone()).with_result(
                    CommandResult::MergeAbort(Box::new(MergeAbortResult {
                        worktree_id: worktree_id.to_string(),
                        branch: outcome.branch,
                        outcome: MergeAbortOutcome::Merged,
                        premerge_head: None,
                        conflict_files: Vec::new(),
                        abort: None,
                        recovery: None,
                    })),
                ),
            ),
            Err(error) => {
                let branch = refreshed
                    .as_ref()
                    .and_then(|(_, facts)| facts.iter().find(|f| f.path == key.path))
                    .map(|f| f.branch.clone())
                    .unwrap_or_default();
                let failure = map_worktree_error(&error);
                Err(
                    match merge_abort_failure_result(worktree_id, &branch, &error) {
                        Some(result) => {
                            failure.with_result(CommandResult::MergeAbort(Box::new(result)))
                        }
                        None => failure,
                    },
                )
            }
        }
    }
    async fn delete_preview(
        &self,
        worktree_id: &str,
    ) -> Result<DeletePreviewResponse, CommandFailure> {
        let key = self.resolve(worktree_id)?;
        let preview = self
            .service
            .delete_preview(&key.path, &key.identity)
            .await
            .map_err(|error| map_worktree_error(&error))?;

        let mut discard_known_dirty =
            DeleteOptionEligibility::from_result(&preview.discard_known_dirty);
        discard_known_dirty.entries = preview.content.observed().map(|c| c.entries);
        let mut discard_commits_ahead =
            DeleteOptionEligibility::from_result(&preview.discard_commits_ahead);
        discard_commits_ahead.commits = preview.commits_ahead;
        discard_commits_ahead.also_discards_dirty = Some(preview.also_discards_dirty);

        Ok(DeletePreviewResponse {
            instance_id: String::new(),
            state_revision: 0,
            worktree_id: worktree_id.to_string(),
            observed_at: chrono::Utc::now().to_rfc3339(),
            branch: preview.facts.branch.clone(),
            head: preview.facts.head.clone(),
            is_main: preview.facts.is_main,
            dirty: preview.observation.dirty,
            has_commits_ahead: preview.observation.has_commits_ahead,
            inspection: preview.facts.inspection,
            content_observation: preview.observation.content_observation,
            has_ignored_content: preview.observation.has_ignored_content,
            ignored_entries: preview.content.observed().map(|c| c.ignored_entries),
            teardown_script_present: preview.teardown_script_present,
            options: DeletePreviewOptions {
                ordinary: DeleteOptionEligibility::from_result(&preview.ordinary),
                skip_teardown: DeleteOptionEligibility::from_result(&preview.skip_teardown),
                discard_known_dirty,
                discard_commits_ahead,
            },
            consent: preview.observation.consent(),
        })
    }

    async fn delete_confirmed(
        &self,
        worktree_id: &str,
        params: &ConfirmedDeleteParams,
    ) -> Result<ExecutionSummary, CommandFailure> {
        let key = self.resolve(worktree_id)?;
        let consent = &params.consent;
        // The opaque ID pins the Git identity; the consent pins branch and
        // HEAD. All three are revalidated under the service's root guard.
        let expected = ExpectedTarget::on_branch(consent.branch.clone())
            .with_identity(key.identity.clone())
            .with_head(consent.head.clone());
        let result = self
            .service
            .delete_worktree_with_consent(&key.path, &expected, consent, params.options())
            .await;

        match result {
            Ok(deleted) => {
                self.registry.retire(worktree_id);
                let _ = self.observe().await;
                let detail = format!(
                    "{}; worktree_id {worktree_id} retired",
                    deleted.outcome.detail
                );
                Ok(ExecutionSummary::changed(detail).with_result(
                    CommandResult::DeleteWorktreeConfirmed(Box::new(ConfirmedDeleteResult {
                        worktree_id: worktree_id.to_string(),
                        branch: deleted.outcome.branch.clone(),
                        head: deleted.head.clone(),
                        outcome: Some("deleted".to_string()),
                        teardown: deleted.teardown,
                        dirty_discarded: deleted.dirty_discarded,
                        commits_discarded: deleted.commits_discarded,
                        branch_deleted: deleted.branch_deleted,
                        branch_retained_reason: deleted.branch_retained_reason.clone(),
                        refusal: None,
                    })),
                ))
            }
            Err(refusal) => {
                let failure = if refusal.stale {
                    CommandFailure::new(ErrorCode::DeleteConsentStale, refusal.error.to_string())
                } else {
                    map_worktree_error(&refusal.error)
                };
                let (branch, head) = refusal
                    .observed
                    .as_ref()
                    .map(|o| (o.branch.clone(), o.head.clone()))
                    .unwrap_or_else(|| (consent.branch.clone(), consent.head.clone()));
                Err(
                    failure.with_result(CommandResult::DeleteWorktreeConfirmed(Box::new(
                        ConfirmedDeleteResult {
                            worktree_id: worktree_id.to_string(),
                            branch,
                            head,
                            outcome: None,
                            teardown: refusal.teardown,
                            dirty_discarded: false,
                            commits_discarded: false,
                            branch_deleted: false,
                            branch_retained_reason: None,
                            refusal: Some(ConfirmedDeleteRefusalResult {
                                reason: refusal.reason,
                                teardown_ran: refusal.teardown_ran(),
                                observed: refusal.observed.as_deref().cloned(),
                            }),
                        },
                    ))),
                )
            }
        }
    }

    async fn text_root(&self, worktree_id: &str) -> Result<PathBuf, CommandFailure> {
        Ok(self.resolve(worktree_id)?.path)
    }

    async fn root_guard_is_held(&self) -> bool {
        self.service.root_guard_is_held()
    }

    async fn refresh_after_text_write(&self) {
        let _ = self.observe().await;
        self.service.announce_refreshed().await;
    }
}
