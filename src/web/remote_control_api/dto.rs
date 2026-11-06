//! Typed `/api/v2` resource, command, event, and error DTOs.
//!
//! Everything a remote controller can say or observe is described here as a
//! typed value. Deserializing into these types is what makes the contract
//! *structural*: JSON member order and insignificant whitespace disappear, and
//! schema defaults are applied before the value is used for idempotency
//! identity, so two textually different bodies that mean the same thing are the
//! same command.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Namespace version advertised by every v2 resource.
pub const API_VERSION: &str = "v2";

/// Retained latest events.
pub const MAX_EVENTS: usize = 1000;

/// Retained latest logs.
pub const MAX_LOGS: usize = 1000;

/// Admission limit for the command registry and the idempotency registry.
pub const MAX_COMMAND_RECORDS: usize = 1000;

/// How long a completed command/idempotency record stays replayable.
pub const COMMAND_RECORD_TTL_SECS: i64 = 24 * 60 * 60;

/// Maximum accepted length of a caller-supplied correlation ID.
pub const MAX_CORRELATION_ID_LEN: usize = 64;

// ============================================================================
// Errors
// ============================================================================

/// Stable, machine-actionable v2 error codes.
///
/// The HTTP status classifies the transport failure; the client branches on
/// `error_code`. Two different causes may share status 409, so the code — not
/// the status — is the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Missing or invalid bearer credentials.
    Unauthorized,
    /// Authenticated but not permitted (for example a denied cross origin).
    Forbidden,
    /// No such resource in this process incarnation.
    NotFound,
    /// `expected_revision` no longer matches the current revision.
    StaleRevision,
    /// The current lifecycle state does not accept this command.
    LifecycleConflict,
    /// This process serves reads but has no command executor bound, so no
    /// mutation can be executed and none is queued for a future one.
    ///
    /// Distinct from [`Self::LifecycleConflict`] on purpose: a lifecycle
    /// conflict says "not now, in this state", while this says "not by this
    /// process, ever". A client that confused the two would keep retrying a
    /// headless `cflx run` forever.
    CommandExecutorUnbound,
    /// The named target cannot accept this command right now.
    TargetIneligible,
    /// The workspace root is busy with another operation.
    RootBusy,
    /// The idempotency key is bound to a different typed command identity.
    IdempotencyMismatch,
    /// No record slot could be reserved without evicting in-progress work.
    RegistryCapacity,
    /// The request failed syntax or typed schema validation.
    ValidationFailed,
    /// A worktree already exists for the addressed change.
    WorktreeExists,
    /// No live worktree is bound to the addressed opaque ID.
    WorktreeNotFound,
    /// The worktree has uncommitted changes.
    WorktreeDirty,
    /// The worktree's dirty state could not be determined, so it fails closed.
    WorktreeDirtyUnknown,
    /// The base merge conflicted and its intermediate state was preserved.
    MergeConflict,
    /// `merge_worktree_abort_on_conflict` refused a conflict the fresh
    /// targeted inspection predicted, before any Git mutation.
    MergeConflictPredicted,
    /// `merge_worktree_abort_on_conflict` hit a real conflict, ran
    /// `git merge --abort`, and proved the root restored: `HEAD` is the
    /// pre-merge commit, no `MERGE_HEAD` exists, and the root is clean.
    MergeConflictAborted,
    /// `merge_worktree_abort_on_conflict` hit a real conflict and restoration
    /// was not proven: the abort failed or a verification could not be read.
    /// Recovery is local or TUI; nothing claims state was preserved on purpose.
    MergeAbortFailed,
    /// `merge_worktree_abort_on_conflict` merged into base and the `on_merged`
    /// hook then failed. The base moved; the record's `result.merge` says
    /// whether the merge commit is confirmed on base, re-read from Git, and
    /// `result.hook` what the hook did. Nothing re-merged, aborted, reset, or
    /// re-ran the hook; recovery is local or TUI.
    MergeHookFailed,
    /// The addressed execution exists, but the presented instance/change
    /// binding is not the one it belongs to.
    ///
    /// Its own code rather than [`Self::NotFound`]: "no such execution" and
    /// "that execution is not yours" ask different things of a client, and a
    /// caller that confused them would rebind a sink onto someone else's work.
    ExecutionBindingMismatch,
    /// The request is legitimate but this transport may not carry it.
    ///
    /// Registering an executable argv is accepted only over the owner-only Unix
    /// socket, so an authenticated TCP client is refused here rather than at the
    /// bearer gate: the credentials were fine, the channel was not.
    TransportNotPermitted,
    /// A candidate proposal refresh was refused at command time.
    ///
    /// Distinct from [`Self::TargetIneligible`], which is the published
    /// lifecycle table: this says the target was eligible and the repository
    /// itself refused — a moved ref, a dirty candidate, a hook. The record's
    /// `result` carries the stable reason and the effects re-read from Git.
    RefreshRefused,
    /// `delete_worktree_confirmed` found that the echoed consent no longer
    /// describes the worktree: a consented fact or a recomputed content
    /// identity differs. Nothing was removed; the record's `result.refusal`
    /// carries the stable reason and the freshly observed facts.
    DeleteConsentStale,
    /// A text write named content the file no longer holds.
    ///
    /// Nothing was written. The record's `result.current_content_id` carries the
    /// identity the file holds now (`null` when it is absent), so a client keeps
    /// its buffer and re-reads instead of being told to discard anything.
    TextStale,
    /// The addressed path is outside the bounded editable scope.
    ///
    /// The `reason` (on a read refusal's message, on a command's `result`) is
    /// one stable token — `git_metadata`, `conflux_owned_path`, `unsafe_path`,
    /// `nested_repository`, `not_regular_file`, `executable`, `too_large`,
    /// `not_text`, or `root_missing`.
    TextNotEditable,
    /// The configured-command session surface cannot launch here: no
    /// `worktree_command` is configured, the project is not a Git repository,
    /// or the owner is shutting down. Refused before any Git or process effect.
    WorktreeSessionUnavailable,
    /// A session is already running for that worktree (`reason=worktree_busy`)
    /// or the running-session limit is reached (`reason=capacity`). Refused
    /// before any Git or process effect.
    WorktreeSessionBusy,
    /// `.wt/setup` failed for a provisioned session worktree; the worktree was
    /// cleaned up and nothing was launched. The record's result carries the
    /// typed cleanup outcome.
    WorktreeSessionSetupFailed,
    /// Sanitized server-side failure.
    InternalError,
}

impl ErrorCode {
    /// Transport classification for this error code.
    pub fn http_status(self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound | Self::WorktreeNotFound => StatusCode::NOT_FOUND,
            Self::StaleRevision
            | Self::LifecycleConflict
            | Self::CommandExecutorUnbound
            | Self::TargetIneligible
            | Self::RootBusy
            | Self::IdempotencyMismatch
            | Self::WorktreeExists
            | Self::WorktreeDirty
            | Self::WorktreeDirtyUnknown
            | Self::MergeConflict
            | Self::MergeConflictPredicted
            | Self::MergeConflictAborted
            | Self::MergeAbortFailed
            | Self::MergeHookFailed
            | Self::ExecutionBindingMismatch
            | Self::RefreshRefused
            | Self::DeleteConsentStale
            | Self::TextStale
            | Self::TextNotEditable
            | Self::WorktreeSessionUnavailable
            | Self::WorktreeSessionBusy
            | Self::WorktreeSessionSetupFailed => StatusCode::CONFLICT,
            Self::TransportNotPermitted => StatusCode::FORBIDDEN,
            Self::RegistryCapacity => StatusCode::SERVICE_UNAVAILABLE,
            Self::ValidationFailed => StatusCode::UNPROCESSABLE_ENTITY,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Wire representation, used by capability discovery and tests.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::StaleRevision => "stale_revision",
            Self::LifecycleConflict => "lifecycle_conflict",
            Self::CommandExecutorUnbound => "command_executor_unbound",
            Self::TargetIneligible => "target_ineligible",
            Self::RootBusy => "root_busy",
            Self::IdempotencyMismatch => "idempotency_mismatch",
            Self::RegistryCapacity => "registry_capacity",
            Self::ValidationFailed => "validation_failed",
            Self::WorktreeExists => "worktree_exists",
            Self::WorktreeNotFound => "worktree_not_found",
            Self::WorktreeDirty => "worktree_dirty",
            Self::WorktreeDirtyUnknown => "worktree_dirty_unknown",
            Self::MergeConflict => "merge_conflict",
            Self::MergeConflictPredicted => "merge_conflict_predicted",
            Self::MergeConflictAborted => "merge_conflict_aborted",
            Self::MergeAbortFailed => "merge_abort_failed",
            Self::MergeHookFailed => "merge_hook_failed",
            Self::ExecutionBindingMismatch => "execution_binding_mismatch",
            Self::TransportNotPermitted => "transport_not_permitted",
            Self::RefreshRefused => "refresh_refused",
            Self::DeleteConsentStale => "delete_consent_stale",
            Self::TextStale => "text_stale",
            Self::TextNotEditable => "text_not_editable",
            Self::WorktreeSessionUnavailable => "worktree_session_unavailable",
            Self::WorktreeSessionBusy => "worktree_session_busy",
            Self::WorktreeSessionSetupFailed => "worktree_session_setup_failed",
            Self::InternalError => "internal_error",
        }
    }
}

/// Every error code, in the order advertised by `/api/v2/capabilities`.
pub const ALL_ERROR_CODES: [ErrorCode; 30] = [
    ErrorCode::Unauthorized,
    ErrorCode::Forbidden,
    ErrorCode::NotFound,
    ErrorCode::StaleRevision,
    ErrorCode::LifecycleConflict,
    ErrorCode::CommandExecutorUnbound,
    ErrorCode::TargetIneligible,
    ErrorCode::RootBusy,
    ErrorCode::IdempotencyMismatch,
    ErrorCode::RegistryCapacity,
    ErrorCode::ValidationFailed,
    ErrorCode::WorktreeExists,
    ErrorCode::WorktreeNotFound,
    ErrorCode::WorktreeDirty,
    ErrorCode::WorktreeDirtyUnknown,
    ErrorCode::MergeConflict,
    ErrorCode::MergeConflictPredicted,
    ErrorCode::MergeConflictAborted,
    ErrorCode::MergeAbortFailed,
    ErrorCode::ExecutionBindingMismatch,
    ErrorCode::TransportNotPermitted,
    ErrorCode::RefreshRefused,
    ErrorCode::DeleteConsentStale,
    ErrorCode::TextStale,
    ErrorCode::TextNotEditable,
    ErrorCode::WorktreeSessionUnavailable,
    ErrorCode::WorktreeSessionBusy,
    ErrorCode::WorktreeSessionSetupFailed,
    ErrorCode::InternalError,
    ErrorCode::MergeHookFailed,
];

/// Structured error body returned by every non-success v2 response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    /// Stable code the client branches on.
    pub error_code: ErrorCode,
    /// Sanitized human-readable detail.
    pub message: String,
    /// Correlation ID of the failing request (caller-supplied or generated).
    pub correlation_id: String,
    /// Current state revision, when the failure is revision-related.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_revision: Option<u64>,
}

impl ApiError {
    /// Build an error without revision context.
    pub fn new(error_code: ErrorCode, message: impl Into<String>, correlation_id: &str) -> Self {
        Self {
            error_code,
            message: message.into(),
            correlation_id: correlation_id.to_string(),
            current_revision: None,
        }
    }

    /// Attach the current revision so a client can resynchronize.
    pub fn with_revision(mut self, revision: u64) -> Self {
        self.current_revision = Some(revision);
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.error_code.http_status(), Json(self)).into_response()
    }
}

// ============================================================================
// Correlation IDs
// ============================================================================

/// True when a caller-supplied correlation ID is an accepted opaque label.
///
/// Correlation IDs are trace labels only: they never authorize, never identify a
/// resource, and never participate in idempotency identity. The character and
/// length bounds exist so a hostile label cannot forge log lines or bloat
/// records.
pub fn is_valid_correlation_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CORRELATION_ID_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

/// Generate a random 128-bit lowercase hexadecimal identifier.
///
/// Re-exported rather than reimplemented: the orchestration-side execution
/// registry mints the same shape in builds that have no web feature, and two
/// generators would eventually disagree about width or case.
pub use crate::ids::new_hex_id;

// ============================================================================
// Commands
// ============================================================================

/// The closed v2 command set.
///
/// Adding a variant is a spec change: an envelope naming any other type fails
/// typed validation before a service call can happen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandSpec {
    /// Start or resume processing.
    Start,
    /// Request a graceful stop.
    Stop,
    /// Cancel a pending graceful stop.
    CancelStop,
    /// Stop immediately.
    ForceStop,
    /// Set the process-local execution mark for a change.
    SetExecutionMark {
        /// Target change.
        change_id: String,
        /// Requested mark value.
        marked: bool,
    },
    /// Set dynamic queue intent for a change.
    SetQueueIntent {
        /// Target change.
        change_id: String,
        /// Requested queue membership.
        queued: bool,
    },
    /// Retry one change using its reconciled evidence.
    RetryChange {
        /// Target change.
        change_id: String,
    },
    /// Retry every listed change that carries retryable evidence.
    RetryErrors {
        /// Candidate changes; unsupported entries are skipped, not rejected.
        #[serde(default)]
        change_ids: Vec<String>,
    },
    /// Stop an in-flight change and dequeue it after confirmed termination.
    StopAndDequeue {
        /// Target change.
        change_id: String,
    },
    /// Immediately kill one change's managed process group and dequeue it.
    ///
    /// Exactly one target, and no target *list*: process-wide `force_stop` and
    /// this are deliberately different commands, so a caller can never widen a
    /// targeted kill into a process-wide one by adding IDs.
    ForceStopChange {
        /// Target change.
        change_id: String,
    },
    /// Request merge resolution for a change waiting on a merge.
    ResolveMerge {
        /// Target change.
        change_id: String,
    },
    /// Apply one derived execution-mark state to every eligible change.
    ///
    /// Deliberately parameterless: the target state is derived from the eligible
    /// rows at the admitted revision, exactly as the TUI's bulk toggle does, so
    /// a client cannot ask for a target set the server never classified.
    SetAllExecutionMarks {},
    /// Create the managed worktree for an eligible change.
    ///
    /// The change ID is the *only* input: branch, path, and base commit are all
    /// server-derived, so no client can steer where the worktree lands or what
    /// it is cut from.
    CreateWorktree {
        /// The change to create a worktree for.
        target: ChangeTarget,
        /// Reserved and always empty.
        #[serde(default)]
        params: EmptyParams,
    },
    /// Delete a worktree addressed by its opaque ID.
    DeleteWorktree {
        /// The worktree to delete.
        target: WorktreeTarget,
        /// Reserved and always empty. There is no teardown bypass or force flag.
        #[serde(default)]
        params: EmptyParams,
    },
    /// Merge a worktree's branch into base.
    MergeWorktree {
        /// The worktree to merge.
        target: WorktreeTarget,
        /// Reserved and always empty.
        #[serde(default)]
        params: EmptyParams,
    },
    /// Merge a worktree's branch into base, aborting on conflict.
    ///
    /// A separate command rather than a parameter on `merge_worktree`, so that
    /// command's conflict-preserving contract holds for every call of it. A
    /// predicted conflict is refused before any Git mutation; a real one is
    /// aborted and the restoration is verified from the repository.
    MergeWorktreeAbortOnConflict {
        /// The worktree to merge.
        target: WorktreeTarget,
        /// Reserved and always empty.
        #[serde(default)]
        params: EmptyParams,
    },
    /// Delete a worktree on explicit, re-verified consent.
    ///
    /// A separate command rather than parameters on `delete_worktree`, so
    /// that command's fail-closed contract — and the published
    /// `delete_requires_teardown: true` — hold for every call of it. The three
    /// destructive permissions are independent choices, and each is honored
    /// only while every consented fact and both content identities still
    /// match a fresh observation under the repository root guard.
    DeleteWorktreeConfirmed {
        /// The worktree to delete.
        target: WorktreeTarget,
        /// The explicit permissions and the echoed consent.
        params: ConfirmedDeleteParams,
    },
    /// Refresh one candidate's `proposal.md` from an explicit source commit.
    ///
    /// Takes the two full commit IDs a `refresh-preview` read returned. Exactly
    /// one target; it never starts, retries, queues, marks, or stops anything.
    RefreshProposal {
        /// Target change.
        change_id: String,
        /// Explicit source ref; there is no default branch.
        source_ref: String,
        /// Full commit ID the source ref must still resolve to.
        expected_source: String,
        /// Full commit ID the candidate `HEAD` must still be at.
        expected_head: String,
    },
    /// Launch the configured `worktree_command` in an existing worktree.
    ///
    /// The command text is always the owner's configured template with
    /// `{workspace_dir}` and `{repo_root}` expanded server-side. No request field
    /// carries a command, argv, environment, path, or branch.
    RunWorktreeCommand {
        /// The worktree to run in.
        target: WorktreeTarget,
        /// Reserved and always empty.
        #[serde(default)]
        params: EmptyParams,
    },
    /// Create a `ws-session-*` worktree from base `HEAD` exactly as the TUI `+`
    /// key does — setup, cleanup on setup failure, revalidation — and launch
    /// the configured `worktree_command` there.
    CreateSessionWorktree {
        /// Reserved and always empty.
        #[serde(default)]
        params: EmptyParams,
    },
    /// Stop one session through the runner's own cancellation path and settle
    /// after the leader is reaped and the cleanup report is read.
    StopWorktreeSession {
        /// The session to stop.
        target: SessionTarget,
        /// Reserved and always empty.
        #[serde(default)]
        params: EmptyParams,
    },
    /// Replace or create one text file inside a change's own directory.
    ///
    /// The root is server-derived — `openspec/changes/<change_id>` of the
    /// project — and `params.expected` must name what the caller read: there is
    /// no unconditional write. Saving never commits, stages, or pushes.
    WriteChangeText {
        /// The change whose directory is the root.
        target: ChangeTarget,
        /// The relative path, the expected identity, and the new content.
        params: TextWriteParams,
    },
    /// Replace or create one text file inside a worktree addressed by opaque ID.
    ///
    /// The root is the directory the ID is bound to, and nothing else: no path,
    /// branch, or ref is accepted as identity.
    WriteWorktreeText {
        /// The worktree whose directory is the root.
        target: WorktreeTarget,
        /// The relative path, the expected identity, and the new content.
        params: TextWriteParams,
    },
}

/// Parameters of a bounded text write.
///
/// `deny_unknown_fields`, so a smuggled root, mode, or force flag is a schema
/// error instead of a silently ignored member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TextWriteParams {
    /// Root-relative UTF-8 path, `/`-separated, at most 1024 bytes.
    pub path: String,
    /// What the caller read. A write proceeds only while it still holds.
    pub expected: TextExpectation,
    /// The complete new file content, at most 1 MiB of UTF-8 without NUL.
    pub content: String,
}

/// The content a write expects the file to hold right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextExpectation {
    /// The file holds exactly these bytes, by `sha256:` identity from a read.
    Content {
        /// `sha256:` followed by 64 lowercase hexadecimal digits.
        content_id: String,
    },
    /// The file does not exist yet; this write creates it.
    ///
    /// An empty struct variant rather than a unit one, so `deny_unknown_fields`
    /// refuses a `content_id` smuggled next to `kind: absent`.
    Absent {},
}

/// Change-addressed command target.
///
/// `deny_unknown_fields` is the security property, not a nicety: it is what makes
/// a smuggled `path`, `branch`, or `base_commit` a schema failure instead of a
/// silently ignored field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeTarget {
    /// Managed, non-archived change ID.
    pub change_id: String,
}

/// Worktree-addressed command target.
///
/// Only the opaque process-local ID addresses a worktree. A path, a branch, or a
/// repository correlation ID is never accepted as mutation identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WorktreeTarget {
    /// Opaque 128-bit hexadecimal worktree ID from a v2 read.
    pub worktree_id: String,
}

/// Session-addressed command target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionTarget {
    /// Opaque 128-bit hexadecimal session ID from a v2 read or launch result.
    pub session_id: String,
}

/// True for a 32-character lowercase hexadecimal opaque ID.
pub fn is_opaque_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A parameter object that accepts nothing.
///
/// Present so the wire shape is stable if a parameter is ever added, and so that
/// every unrecognized parameter fails validation today.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

/// Typed parameters of `delete_worktree_confirmed`.
///
/// Three independent booleans — none implies another — plus the `consent`
/// object a `delete-preview` read returned, echoed verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfirmedDeleteParams {
    /// Skip `.wt/teardown`. The recovery path for a failing teardown script.
    pub skip_teardown: bool,
    /// Discard known uncommitted changes and known ignored content.
    pub discard_known_dirty: bool,
    /// Discard commits ahead of base and force-delete the branch at the
    /// consented HEAD.
    pub discard_commits_ahead: bool,
    /// The preview's `consent` object, unmodified.
    pub consent: crate::worktree_ops::consent::DeleteConsent,
}

impl ConfirmedDeleteParams {
    /// Field-level validation serde cannot express.
    pub fn validate(&self) -> Result<(), String> {
        use crate::orchestration::proposal_refresh::is_full_oid;
        use crate::worktree_ops::consent::is_content_id;

        let consent = &self.consent;
        if consent.branch.trim().is_empty() {
            return Err(
                "delete_worktree_confirmed requires a non-empty consent.branch".to_string(),
            );
        }
        if consent.head.len() != 40 || !is_full_oid(&consent.head) {
            return Err(
                "consent.head must be a 40-character lowercase hexadecimal commit ID".to_string(),
            );
        }
        if let Some(base) = consent.merge_base.as_deref() {
            if base.len() != 40 || !is_full_oid(base) {
                return Err(
                    "consent.merge_base must be a 40-character lowercase hexadecimal commit ID"
                        .to_string(),
                );
            }
        }
        for (name, value) in [
            ("discardable_content_id", &consent.discardable_content_id),
            ("ignored_content_id", &consent.ignored_content_id),
        ] {
            if value.as_deref().is_some_and(|id| !is_content_id(id)) {
                return Err(format!(
                    "consent.{name} must be sha256: followed by 64 lowercase hexadecimal digits"
                ));
            }
        }
        // Independent of `consent.dirty`: a discard permission is only ever
        // bound to named content, never to an undefined comparison.
        if (self.discard_known_dirty || self.discard_commits_ahead)
            && (consent.discardable_content_id.is_none() || consent.ignored_content_id.is_none())
        {
            return Err(
                "a discard permission requires both consent.discardable_content_id and consent.ignored_content_id"
                    .to_string(),
            );
        }
        if self.discard_commits_ahead && consent.merge_base.is_none() {
            return Err("discard_commits_ahead requires consent.merge_base".to_string());
        }
        Ok(())
    }

    /// The shared delete policy these permissions map onto, one-to-one.
    pub fn options(&self) -> crate::worktree_ops::service::DeleteOptions {
        crate::worktree_ops::service::DeleteOptions {
            skip_teardown: self.skip_teardown,
            allow_known_dirty: self.discard_known_dirty,
            allow_commits_ahead: self.discard_commits_ahead,
        }
    }
}

impl CommandSpec {
    /// Wire discriminant of this command.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::CancelStop => "cancel_stop",
            Self::ForceStop => "force_stop",
            Self::SetExecutionMark { .. } => "set_execution_mark",
            Self::SetQueueIntent { .. } => "set_queue_intent",
            Self::RetryChange { .. } => "retry_change",
            Self::RetryErrors { .. } => "retry_errors",
            Self::StopAndDequeue { .. } => "stop_and_dequeue",
            Self::ForceStopChange { .. } => "force_stop_change",
            Self::ResolveMerge { .. } => "resolve_merge",
            Self::SetAllExecutionMarks { .. } => "set_all_execution_marks",
            Self::CreateWorktree { .. } => "create_worktree",
            Self::DeleteWorktree { .. } => "delete_worktree",
            Self::MergeWorktree { .. } => "merge_worktree",
            Self::MergeWorktreeAbortOnConflict { .. } => "merge_worktree_abort_on_conflict",
            Self::DeleteWorktreeConfirmed { .. } => "delete_worktree_confirmed",
            Self::RefreshProposal { .. } => "refresh_proposal",
            Self::WriteChangeText { .. } => "write_change_text",
            Self::WriteWorktreeText { .. } => "write_worktree_text",
            Self::RunWorktreeCommand { .. } => "run_worktree_command",
            Self::CreateSessionWorktree { .. } => "create_session_worktree",
            Self::StopWorktreeSession { .. } => "stop_worktree_session",
        }
    }

    /// Field-level validation serde cannot express.
    ///
    /// Run before admission, so a malformed command is a `validation_failed`
    /// envelope rather than a record that settles as a refusal.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::RefreshProposal {
                change_id,
                source_ref,
                expected_source,
                expected_head,
            } => {
                if change_id.trim().is_empty() {
                    return Err("refresh_proposal requires a non-empty change_id".to_string());
                }
                if source_ref.trim().is_empty() {
                    return Err("refresh_proposal requires a non-empty source_ref".to_string());
                }
                for (name, value) in [
                    ("expected_source", expected_source),
                    ("expected_head", expected_head),
                ] {
                    if !crate::orchestration::proposal_refresh::is_full_oid(value) {
                        return Err(format!(
                            "{name} must be a full-length lowercase hexadecimal commit ID"
                        ));
                    }
                }
                Ok(())
            }
            Self::DeleteWorktreeConfirmed { params, .. } => params.validate(),
            Self::WriteChangeText { target, params } => {
                if target.change_id.trim().is_empty() {
                    return Err("write_change_text requires a non-empty change_id".to_string());
                }
                params.validate()
            }
            Self::WriteWorktreeText { target, params } => {
                if target.worktree_id.trim().is_empty() {
                    return Err("write_worktree_text requires a non-empty worktree_id".to_string());
                }
                params.validate()
            }
            Self::RunWorktreeCommand { target, .. } => {
                if is_opaque_id(&target.worktree_id) {
                    Ok(())
                } else {
                    Err(
                        "run_worktree_command requires a 32-character lowercase hexadecimal \
                         target.worktree_id"
                            .to_string(),
                    )
                }
            }
            Self::StopWorktreeSession { target, .. } => {
                if is_opaque_id(&target.session_id) {
                    Ok(())
                } else {
                    Err(
                        "stop_worktree_session requires a 32-character lowercase hexadecimal \
                         target.session_id"
                            .to_string(),
                    )
                }
            }
            _ => Ok(()),
        }
    }

    /// Single change this command targets, when it has one.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn target(&self) -> Option<&str> {
        match self {
            Self::SetExecutionMark { change_id, .. }
            | Self::SetQueueIntent { change_id, .. }
            | Self::RetryChange { change_id }
            | Self::StopAndDequeue { change_id }
            | Self::ForceStopChange { change_id }
            | Self::ResolveMerge { change_id }
            | Self::RefreshProposal { change_id, .. } => Some(change_id),
            Self::CreateWorktree { target, .. } | Self::WriteChangeText { target, .. } => {
                Some(&target.change_id)
            }
            Self::Start | Self::Stop | Self::CancelStop | Self::ForceStop => None,
            Self::RetryErrors { .. } => None,
            // A process-wide mutation: it addresses the whole target set, never
            // one change.
            Self::SetAllExecutionMarks { .. } => None,
            // Worktree mutations are addressed by opaque ID, not by change.
            Self::DeleteWorktree { .. }
            | Self::DeleteWorktreeConfirmed { .. }
            | Self::MergeWorktree { .. }
            | Self::MergeWorktreeAbortOnConflict { .. }
            | Self::WriteWorktreeText { .. } => None,
            // Sessions are addressed by opaque worktree or session ID, never by change.
            Self::RunWorktreeCommand { .. }
            | Self::CreateSessionWorktree { .. }
            | Self::StopWorktreeSession { .. } => None,
        }
    }
}

impl TextWriteParams {
    /// Lexical path rules, the size ceiling, NUL, and identity shape.
    ///
    /// Everything here is decided without touching the filesystem, so a
    /// violation is a `validation_failed` envelope with no record.
    pub fn validate(&self) -> Result<(), String> {
        super::text_edit::validate_path(&self.path)?;
        if self.content.len() as u64 > super::text_edit::MAX_TEXT_BYTES {
            return Err(format!(
                "content exceeds the {} byte text ceiling",
                super::text_edit::MAX_TEXT_BYTES
            ));
        }
        if self.content.contains('\0') {
            return Err("content must not contain NUL".to_string());
        }
        if let TextExpectation::Content { content_id } = &self.expected {
            if !super::text_edit::is_content_id(content_id) {
                return Err(
                    "expected.content_id must be `sha256:` followed by 64 lowercase hex digits"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

/// Every supported command type, in the order advertised by capabilities.
pub const SUPPORTED_COMMANDS: [&str; 23] = [
    "start",
    "stop",
    "cancel_stop",
    "force_stop",
    "set_execution_mark",
    "set_queue_intent",
    "retry_change",
    "retry_errors",
    "stop_and_dequeue",
    "force_stop_change",
    "resolve_merge",
    "set_all_execution_marks",
    "create_worktree",
    "delete_worktree",
    "merge_worktree",
    "refresh_proposal",
    "merge_worktree_abort_on_conflict",
    "delete_worktree_confirmed",
    "write_change_text",
    "write_worktree_text",
    "run_worktree_command",
    "create_session_worktree",
    "stop_worktree_session",
];

/// `POST /api/v2/commands` request envelope.
///
/// Every command is side-effecting, so `expected_revision` and
/// `idempotency_key` are mandatory for all of them without exception.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CommandRequest {
    /// Discriminated command payload.
    #[serde(flatten)]
    pub command: CommandSpec,
    /// Revision the caller believes is current.
    pub expected_revision: u64,
    /// Caller-chosen replay key, unique per intended side effect.
    pub idempotency_key: String,
    /// Optional opaque trace label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl CommandRequest {
    /// Structural identity of the intended side effect.
    ///
    /// This is the normalized typed tuple `(type, target, params, expected_revision)`:
    /// it comes from the *typed* value, so member order and whitespace in the
    /// original body cannot change it, and it deliberately excludes
    /// `idempotency_key` and `correlation_id`.
    pub fn identity(&self) -> CommandIdentity {
        CommandIdentity {
            command: self.command.clone(),
            expected_revision: self.expected_revision,
        }
    }
}

/// Normalized typed identity used for idempotency comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandIdentity {
    /// Typed command including its target and parameters.
    pub command: CommandSpec,
    /// Revision the command was admitted against.
    pub expected_revision: u64,
}

/// Lifecycle of a submitted command within this process incarnation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandState {
    /// Accepted and still executing. Never evicted.
    Running,
    /// Completed with an effect.
    Succeeded,
    /// Completed without an effect because nothing needed to change.
    NoOp,
    /// Completed with a typed failure.
    Failed,
}

impl CommandState {
    /// True while the command is pinned against eviction.
    pub fn is_in_progress(self) -> bool {
        matches!(self, Self::Running)
    }
}

/// Command record returned by `POST /api/v2/commands` and by status lookup.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CommandRecord {
    /// Random 128-bit hexadecimal command ID.
    pub command_id: String,
    /// Process incarnation that owns this record.
    pub instance_id: String,
    /// Wire discriminant of the submitted command.
    #[serde(rename = "type")]
    pub command_type: String,
    /// Current lifecycle state.
    pub state: CommandState,
    /// Revision the command was admitted against.
    pub expected_revision: u64,
    /// Revision observed once the command finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_revision: Option<u64>,
    /// Correlation label of the submitting request.
    pub correlation_id: String,
    /// Replay key this record is bound to.
    pub idempotency_key: String,
    /// Submission time (RFC 3339).
    pub created_at: String,
    /// Completion time (RFC 3339), when finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// Sanitized outcome detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Typed failure code when `state` is `failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ErrorCode>,
    /// Typed settlement evidence, for the commands that produce it.
    ///
    /// Written once when the command settles and never recomputed: an exact
    /// idempotent replay returns the original value even after later lifecycle
    /// or Git state has moved on. Absent for every command that has no typed
    /// result, which keeps existing clients unaffected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<CommandResult>,
}

impl CommandRecord {
    /// HTTP status for returning this record from the command endpoint.
    ///
    /// A still-running command — including a replayed one — is `202`; a settled
    /// command reports its own outcome.
    pub fn http_status(&self) -> StatusCode {
        match self.state {
            CommandState::Running => StatusCode::ACCEPTED,
            CommandState::Succeeded | CommandState::NoOp => StatusCode::OK,
            CommandState::Failed => self
                .error_code
                .map(ErrorCode::http_status)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

// ============================================================================
// Read resources
// ============================================================================

/// `GET /api/v2/health` body. This resource is always unauthenticated.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    /// Always `ok` while the process serves requests.
    pub status: String,
    /// API namespace version.
    pub api_version: String,
    /// Build version string.
    pub version: String,
}

/// Retention and admission limits advertised to clients.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CapabilityLimits {
    /// Retained events.
    pub max_events: usize,
    /// Retained logs.
    pub max_logs: usize,
    /// Command registry admission limit.
    pub max_commands: usize,
    /// Idempotency registry admission limit.
    pub max_idempotency_records: usize,
    /// Completed-record expiry in seconds.
    pub command_record_ttl_secs: i64,
    /// Maximum caller correlation ID length.
    pub max_correlation_id_len: usize,
}

/// `GET /api/v2/capabilities` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CapabilitiesResponse {
    /// API namespace version.
    pub api_version: String,
    /// Process incarnation ID.
    pub instance_id: String,
    /// Closed command set accepted by `POST /api/v2/commands`.
    pub commands: Vec<String>,
    /// Supported event transports and their required client style.
    pub transports: Vec<TransportDescriptor>,
    /// Every error code this API can return.
    pub error_codes: Vec<String>,
    /// Retention and admission limits.
    pub limits: CapabilityLimits,
    /// True when bearer authentication is enforced.
    pub authentication_required: bool,
    /// Whether this process can execute a command at all.
    pub command_execution: CommandExecutionCapability,
    /// Whether this process serves execution-scoped completion sinks.
    ///
    /// `#[serde(default)]` so a *client* built against this contract can still
    /// read an older owner that predates the surface: the absent field means
    /// "unavailable", which is exactly what such an owner is. The server always
    /// writes it.
    #[serde(default)]
    pub execution_sinks: ExecutionSinkCapability,
    /// Whether this process serves proposal-scoped subscriptions.
    ///
    /// `#[serde(default)]` for the same reason as `execution_sinks`: a client
    /// built against this contract must be able to read an older owner, and the
    /// absent field means "unavailable", which is exactly what such an owner is.
    #[serde(default)]
    pub proposal_subscriptions: ProposalSubscriptionCapability,
    /// The complete worktree surface, including its conflict-recovery boundary.
    pub worktrees: super::worktrees::WorktreeCapabilities,
    /// The parallel execution surface, including its blocked-reason vocabulary.
    pub parallel: ParallelCapabilities,
    /// The candidate proposal refresh surface.
    ///
    /// `#[serde(default)]` so a client can still read an older owner, where the
    /// absent field means "not served".
    #[serde(default)]
    pub proposal_refresh: ProposalRefreshCapability,
    /// The bounded remote text edit surface.
    ///
    /// `#[serde(default)]` so a client can still read an older owner, where the
    /// absent field decodes as unavailable — never as available.
    #[serde(default)]
    pub text_edit: TextEditCapability,
    /// The configured-command worktree session surface.
    ///
    /// `#[serde(default)]` so a client can still read an older owner, where the
    /// absent field decodes as `available: false` — exactly what such an owner is.
    #[serde(default)]
    pub worktree_sessions: WorktreeSessionCapability,
}

/// Whether configured-command worktree sessions can be launched here, and the
/// limits that bound them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct WorktreeSessionCapability {
    /// True only when `worktree_command` is configured, the project is a Git
    /// repository, and a command executor is bound.
    pub available: bool,
    /// The three session command types.
    pub commands: Vec<String>,
    /// The two session read routes.
    pub routes: Vec<String>,
    /// Sessions that may run at once.
    pub max_running: usize,
    /// Finished sessions retained, oldest evicted first.
    pub max_retained: usize,
    /// Retained output tail per session, in bytes.
    pub max_output_bytes: usize,
    /// Retained runner notices per session, oldest dropped first. `null` from
    /// an older owner, which publishes no notices.
    #[serde(default)]
    pub max_runner_notices: Option<usize>,
    /// What the command's stdin is. Always `null`: there is no stdin relay.
    pub stdin: String,
    /// Every stable refusal reason.
    pub reasons: Vec<String>,
    /// The state events a session publishes. Output is never an event.
    #[serde(default)]
    pub event_types: Vec<WorktreeSessionEventType>,
}

/// State events a configured-command worktree session publishes.
///
/// Payload `{session_id, worktree_id, state, exit_code}`, `change_id: null`. They
/// take an event sequence but never advance `state_revision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeSessionEventType {
    /// A session was launched.
    WorktreeSessionStarted,
    /// A session settled.
    WorktreeSessionFinished,
}

/// Whether a mutation can be executed by this process at all.
///
/// A separate typed fact from lifecycle eligibility: a headless `cflx run`
/// serves every read resource and publishes ordinary action eligibility, but has
/// no command executor bound, so *every* mutation is refused with
/// [`ErrorCode::CommandExecutorUnbound`] and none is queued for a future
/// executor. A client reads this before it prepares a mutation, rather than
/// discovering it from a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CommandExecutionCapability {
    /// True once a command executor is bound to this process incarnation.
    pub available: bool,
}

/// Whether execution-scoped completion sinks can be registered at all.
///
/// Published as its own typed fact so a client discovers the surface instead of
/// inferring it from a 404: an older owner has no `/api/v2/executions/...`
/// route, and "the route is missing" and "this owner refuses sinks" would
/// otherwise be indistinguishable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct ExecutionSinkCapability {
    /// True when this build serves the execution-sink resources.
    pub available: bool,
    /// Longest accepted callback argv.
    pub max_command_args: usize,
    /// Longest accepted single argv element, in bytes.
    pub max_command_arg_len: usize,
    /// Wall-clock ceiling one callback may run for, in milliseconds.
    pub callback_timeout_ms: u64,
    /// Captured-output ceiling per callback stream, in bytes.
    pub max_callback_output_bytes: usize,
}

/// One supported event transport.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransportDescriptor {
    /// Transport name (`sse` or `websocket`).
    pub name: String,
    /// Path the transport is served on.
    pub path: String,
    /// Client style this transport supports.
    ///
    /// Authenticated v2 has no browser-native client: SSE requires `fetch()`
    /// response streaming and the WebSocket requires an Authorization header
    /// during upgrade, which browsers cannot set.
    pub client: String,
    /// Whether the browser-native API for this transport is supported.
    pub browser_native_supported: bool,
}

/// `GET /api/v2/instance` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InstanceResponse {
    /// Random 128-bit hexadecimal process incarnation ID.
    pub instance_id: String,
    /// Process start time (RFC 3339).
    pub started_at: String,
    /// Operating-system process ID.
    pub pid: u32,
    /// Build version string.
    pub version: String,
    /// API namespace version.
    pub api_version: String,
}

// ============================================================================
// Authoritative operator state
// ============================================================================
//
// Everything below exists so a controller never has to *infer* an operator
// decision. Each value is either reducer-owned, owned by the process-local
// operator intent store, or a server-side observation — and each one is present
// on every change, with an explicit empty value rather than an omitted key, so a
// single `GET /api/v2/state` is a complete replacement for whatever a client had
// before.

/// Reducer-owned queue membership intent.
///
/// Deliberately separate from `display_status`: a change can be `Queued` while
/// its display status reads `blocked`, and collapsing the two would make a
/// client guess which one the operator actually asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueueIntent {
    /// The change has not been requested to run.
    #[default]
    NotQueued,
    /// The change has been requested to run.
    Queued,
}

/// Operator attention state for a change.
///
/// Process-local and non-durable: a restart re-observes the workspace and
/// nothing carries a prior incarnation's attention forward.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttentionState {
    /// Nothing to draw the operator's eye.
    #[default]
    None,
    /// Newly detected in this process incarnation and not yet acted on.
    New,
}

/// Machine-readable reason a change is `blocked`.
///
/// Mirrors [`crate::orchestration::state::BlockerKind`]; `none` is the value a
/// `stalled` execution hold carries, because a stop is not a wait on a named
/// prerequisite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockerKind {
    /// Not blocked on a named prerequisite.
    #[default]
    None,
    /// Waiting on an unarchived proposal dependency.
    Dependency,
    /// Waiting on a validated non-repository prerequisite.
    External,
}

/// Reducer-derived detail for a `blocked` or `stalled` change.
///
/// `null` on every other status, so a client can never render a blocker badge on
/// a change that is not actually held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChangeBlocker {
    /// Reducer display status this blocker belongs to: `blocked` or `stalled`.
    pub status: String,
    /// Machine-readable kind; `none` for a stalled execution hold.
    pub kind: BlockerKind,
    /// Machine-readable blocker category, when the reporter supplied one.
    pub category: Option<String>,
    /// One-line operator-facing explanation.
    pub detail: Option<String>,
    /// Verifiable condition that clears an external wait.
    pub unblock_condition: Option<String>,
    /// Owning team or role for an external wait.
    pub prerequisite_owner: Option<String>,
    /// Phase that observed the prerequisite.
    pub origin: Option<String>,
    /// Whether work resumes once the prerequisite is satisfied.
    pub resumable: bool,
    /// Current unresolved dependency IDs for a `dependency` wait.
    ///
    /// Empty for every other kind. Published as its own list so a client that
    /// has to know *which* proposal it is waiting on never parses `detail`.
    ///
    /// `#[serde(default)]` so a client built against this contract can still
    /// read an owner that predates structured dependency projection.
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Why an operator action is refused right now.
///
/// These are stable tokens, not prose: a client branches on them and never
/// parses a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionBlockedReason {
    /// The change reached a final outcome and accepts no operator mutation.
    FinalStatus,
    /// Recovery in this mode is owned by the retry commands.
    RetryRequired,
    /// A graceful stop is in flight; intent changes wait for it.
    StopPending,
    /// The mode/status pair refuses this mutation.
    StatusImmutable,
    /// This mode has no runtime queue, so queue intent cannot be mutated.
    ModeHasNoQueue,
    /// The change carries no retryable evidence.
    NoRetryableEvidence,
    /// The hold is not resumable, so its blocker evidence is retained.
    HoldNotResumable,
    /// The change is executing and must be stopped rather than mutated.
    ChangeActive,
    /// The change is not waiting on a merge.
    NotMergeWaiting,
    /// Worktree execution refuses a change that is not committed cleanly yet.
    ParallelIneligible,
    /// The change carries no admission, so there is no episode to end.
    NotAdmitted,
    /// This owner holds no live managed process group for the change.
    ///
    /// The refusal a targeted force-stop reports for a row that presents as
    /// active without owning a killable identity — inline worktree preparation
    /// is the ordinary way to reach it — and for a base-lane or resolve wait,
    /// which owns no process by construction.
    NoManagedProcess,
    /// Retired: the server no longer refuses an action for this reason.
    ///
    /// The Apply-iteration ceiling is diagnostic evidence about the invocation
    /// that stopped, not an operator-action block, so no current projection or
    /// command admission produces this token. The variant remains published so a
    /// client generated against an older contract — or one still handling a
    /// recorded snapshot that carries it — keeps deserializing rather than
    /// failing on an unknown reason.
    ApplyIterationLimitActive,
    /// `refresh_proposal`: the owner does not track the change.
    UnknownTarget,
    /// `refresh_proposal`: no managed candidate worktree is registered.
    NoCandidateWorktree,
    /// `refresh_proposal`: archived, merged, pushed, or rejected.
    TerminalTarget,
    /// `refresh_proposal`: executing, or owning a live managed process.
    TargetActive,
    /// `refresh_proposal`: admitted (`queued`, or `blocked` on a dependency).
    TargetAdmitted,
    /// `refresh_proposal`: parked on a base-lane merge wait.
    MergeWait,
    /// `refresh_proposal`: waiting for manual resolution.
    ResolveWait,
    /// `refresh_proposal`: waiting on a rejection decision.
    RejectWait,
}

impl ActionBlockedReason {
    /// Project a shared refresh lifecycle reason onto the wire vocabulary.
    ///
    /// One-to-one by token: the reason `actions.refresh_proposal` publishes is
    /// spelled exactly the way the shared transaction refuses. A command-time
    /// reason never reaches here — it is not a property of the row.
    pub fn from_refresh_reason(
        reason: crate::orchestration::proposal_refresh::RefreshReason,
    ) -> Self {
        use crate::orchestration::proposal_refresh::RefreshReason as R;
        match reason {
            R::UnknownTarget => Self::UnknownTarget,
            R::NoCandidateWorktree => Self::NoCandidateWorktree,
            R::TerminalTarget => Self::TerminalTarget,
            R::TargetActive => Self::TargetActive,
            R::MergeWait => Self::MergeWait,
            R::ResolveWait => Self::ResolveWait,
            R::RejectWait => Self::RejectWait,
            // Every other lifecycle reason is an admission, and a command-time
            // reason never classifies a row.
            _ => Self::TargetAdmitted,
        }
    }

    /// Project a shared targeted-force-stop exclusion onto the wire vocabulary.
    ///
    /// One vocabulary, not two: the reason the shared transaction refuses a
    /// targeted force-stop is the reason the change's `actions` block publishes.
    pub fn from_force_stop_exclusion(
        exclusion: crate::orchestration::operator_command::ForceStopExclusion,
    ) -> Self {
        use crate::orchestration::operator_command::ForceStopExclusion as E;
        match exclusion {
            E::TerminalTarget => Self::FinalStatus,
            // An untracked target has no projected row at all, so this arm is
            // only reachable from a command refusal; "no admission" is the
            // truthful wire spelling of both.
            E::UnknownTarget | E::NotAdmitted => Self::NotAdmitted,
            E::MergeWait | E::ResolveWait | E::NoLiveProcess => Self::NoManagedProcess,
        }
    }

    /// Project a shared bulk-mark exclusion onto the wire vocabulary.
    ///
    /// One vocabulary, not two: the reason a bulk mutation skipped a row is the
    /// same reason the row's own `actions` block reports.
    pub fn from_mark_exclusion(
        exclusion: crate::orchestration::operator_command::MarkExclusion,
    ) -> Self {
        use crate::orchestration::operator_command::MarkExclusion as E;
        match exclusion {
            // A reducer-recorded archive completion blocks exactly what a final
            // status blocks, and it is reported on the wire as the same reason:
            // publishing a new variant would change the state payload's schema
            // for a fact the archive milestone already implies.
            E::ArchiveComplete | E::FinalStatus => Self::FinalStatus,
            E::RetryRequired => Self::RetryRequired,
            E::StopPending => Self::StopPending,
            E::ChangeActive => Self::ChangeActive,
            E::StatusImmutable => Self::StatusImmutable,
            // Both parallel refusals block the same actions; the reason they
            // differ on is published per change in `parallel.blocked_reason`.
            E::ParallelIneligible | E::ParallelProposalAbsent => Self::ParallelIneligible,
        }
    }
}

/// Whether one operator action is currently permitted, and why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ActionEligibility {
    /// True when the command would pass its lifecycle guards right now.
    pub allowed: bool,
    /// Stable reason the command is refused; `null` when it is allowed.
    pub blocked_reason: Option<ActionBlockedReason>,
}

impl ActionEligibility {
    /// The action is permitted.
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            blocked_reason: None,
        }
    }

    /// The action is refused for the given stable reason.
    pub fn blocked(reason: ActionBlockedReason) -> Self {
        Self {
            allowed: false,
            blocked_reason: Some(reason),
        }
    }
}

/// Per-change eligibility for every change-addressed operator command.
///
/// Derived server-side from the one shared lifecycle matrix the TUI uses, so a
/// remote frontend and a keypress can never disagree about what is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChangeActions {
    /// `set_execution_mark`.
    pub set_execution_mark: ActionEligibility,
    /// `set_queue_intent`.
    pub set_queue_intent: ActionEligibility,
    /// `retry_change`.
    pub retry_change: ActionEligibility,
    /// `stop_and_dequeue`.
    pub stop_and_dequeue: ActionEligibility,
    /// `force_stop_change`.
    ///
    /// Published separately from `stop_and_dequeue` because the two answer
    /// different questions: a graceful stop is offered for every non-final row,
    /// while an immediate kill is offered only where this owner really holds a
    /// managed process group — or where the row is admitted with nothing
    /// running and can be dequeued outright.
    pub force_stop_change: ActionEligibility,
    /// `resolve_merge`.
    pub resolve_merge: ActionEligibility,
    /// `refresh_proposal`.
    ///
    /// Lifecycle eligibility only, from the shared refresh table: a candidate
    /// that is allowed here can still be refused at command time by its own
    /// repository state, which the `refresh-preview` read reports.
    ///
    /// `Option` with `#[serde(default)]` because the member is additive: an
    /// owner built before the candidate refresh surface publishes every other
    /// member of this table and not this one, and a client reading such an
    /// owner must keep decoding `/api/v2/state` for `status`, `wait`, `mark`,
    /// and every other verb that never consults refresh eligibility. `None`
    /// means exactly "this owner published no refresh table for the row" — it
    /// is never `allowed`, and it is distinct from a published refusal. The
    /// server always writes `Some`, so a `None` decoded from an owner whose
    /// capabilities advertise the surface is a contract mismatch, not an
    /// eligibility. The remaining members stay required: a payload missing
    /// one of them is malformed and is rejected as such. The published
    /// contract keeps the member required and non-null, because that is what
    /// this server serves; the `Option` is a decoding affordance for older
    /// owners, not a licence to omit.
    #[serde(default)]
    #[schema(required, nullable = false)]
    pub refresh_proposal: Option<ActionEligibility>,
}

/// Why a change cannot take part in parallel execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParallelBlockedReason {
    /// The change does not exist in the base commit tree.
    NotCommitted,
    /// The change has uncommitted or untracked files under its directory.
    UncommittedChanges,
    /// The change is currently held by an unresolved proposal dependency.
    ///
    /// Unlike the two workspace observations, this one is transient and clears
    /// itself: the owner resumes the change as soon as repository-visible
    /// evidence proves every dependency integrated. It is published so an empty
    /// execution slot next to a `blocked` row is explained rather than left
    /// looking like a ready change nobody dispatched.
    DependencyBlocked,
}

/// Every parallel-eligibility blocked reason, in capability-advertised order.
pub const ALL_PARALLEL_BLOCKED_REASONS: [&str; 3] =
    ["not_committed", "uncommitted_changes", "dependency_blocked"];

/// Process-wide worktree execution runtime facts.
///
/// There is one execution model, so nothing here names a mode: a client reads
/// the concurrency and backend a run would use.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ParallelRuntimeState {
    /// Maximum number of concurrently executing changes.
    pub max_concurrent: usize,
    /// VCS backend a run would use.
    pub vcs_backend: String,
}

/// Worktree execution surface advertised by `/api/v2/capabilities`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ParallelCapabilities {
    /// Maximum number of concurrently executing changes.
    pub max_concurrent: usize,
    /// VCS backend a run would use.
    pub vcs_backend: String,
    /// Every machine-readable per-change eligibility blocked reason.
    pub blocked_reasons: Vec<String>,
}

/// Server-observed parallel-execution eligibility.
///
/// Present so a client never has to run Git itself to decide whether a change
/// can be queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ParallelEligibility {
    /// True when the change may take part in parallel execution.
    pub eligible: bool,
    /// Stable reason it may not; `null` when it may.
    pub blocked_reason: Option<ParallelBlockedReason>,
}

impl Default for ParallelEligibility {
    fn default() -> Self {
        Self {
            eligible: true,
            blocked_reason: None,
        }
    }
}

/// Active-run timing for one change.
///
/// Only boundary instants are published. A live elapsed counter in the snapshot
/// would make every projection look changed and advance the revision forever, so
/// a client derives "running for" from `started_at` itself.
///
/// Per-run boundaries of the current or most recent run; `elapsed_ms` is that
/// run's duration and is not cumulative — see `net_active`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChangeTiming {
    /// When the current or most recent run started (RFC 3339).
    pub started_at: Option<String>,
    /// When it finished (RFC 3339); `null` while it is still running.
    pub completed_at: Option<String>,
    /// Duration of the finished run in milliseconds.
    pub elapsed_ms: Option<u64>,
}

/// Cumulative net-active execution time of one change in this owner process.
///
/// Driven by the same shared clock the TUI row's net execution time uses: an
/// interval opens on a transition into an active display status (`preparing`,
/// `applying`, `accepting`, `rejecting`, `archiving`, `resolving`), stays open
/// across repeated or successive active statuses, and closes exactly once on a
/// transition to any other status or on process `Stopped`. Closed intervals
/// accumulate and are never reset while the owner runs; a restarted owner
/// starts from zero. Unlike `timing`, which is per run, this is cumulative.
///
/// No live counter is published: a client derives the current total as
/// `accumulated_ms + (now - active_since)` while `active_since` is set, so the
/// snapshot never changes merely because time passed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NetActiveTiming {
    /// Sum of closed active intervals, in milliseconds.
    pub accumulated_ms: u64,
    /// When the open active interval started (RFC 3339); `null` while closed.
    #[schema(required)]
    pub active_since: Option<String>,
    /// Whether this change was ever active in this owner process.
    pub has_history: bool,
}

/// Run-level elapsed clock, the projection of the TUI `Elapsed` line.
///
/// Opened by an accepted Start while the owner is not running; left open by a
/// persistent-idle park, persistent-idle re-admission, `ResolveReserved`, and a
/// fatal process error; frozen by every `Stopped` and every `AllCompleted`
/// delivery and by a resolve-side settlement that leaves nothing processing; a
/// repeated terminal delivery re-freezes at the later instant. The value is
/// retained until the next opening Start. All three members are `null` before
/// the first Start in this process. Process-local presentation only.
///
/// While `ended_at` is `null` a client derives the live value as
/// `now - started_at`; no server-side counter is ever published.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RunClock {
    /// When the current or most recent run was opened (RFC 3339).
    #[schema(required)]
    pub started_at: Option<String>,
    /// When the run clock was last frozen (RFC 3339); `null` while open.
    #[schema(required)]
    pub ended_at: Option<String>,
    /// Monotonic duration from the open to the latest freeze, in whole
    /// milliseconds; `null` while open.
    #[schema(required)]
    pub elapsed_ms: Option<u64>,
}

/// The latest lifecycle-significant activity observed for a change.
///
/// Fed from the state-event projection, never from the log ring: a log must not
/// advance the revision, and a per-chunk output event would churn it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChangeActivity {
    /// Wire event type, identical to the stream's `event_type`.
    pub event_type: String,
    /// Observation time (RFC 3339).
    pub timestamp: String,
    /// Sanitized detail, when the event carried one.
    pub detail: Option<String>,
}

/// Server-stated relation between a change and its managed worktree.
///
/// The path is repository-relative and produced by the same redaction the
/// worktree resources use, so joining a change to `/api/v2/worktrees` is an
/// exact match rather than a client-side guess. Absolute and canonical roots are
/// never serialized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChangeWorktree {
    /// Repository-relative worktree path.
    pub path: String,
    /// Checked-out branch, when it is known.
    pub branch: Option<String>,
}

/// One change as projected into the v2 snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ChangeResource {
    /// Change ID.
    pub id: String,
    /// Reducer-derived display status (canonical taxonomy).
    pub display_status: String,
    /// Task-progress status (`pending`, `in_progress`, `complete`).
    pub progress_status: String,
    /// Completed tasks.
    pub completed_tasks: u32,
    /// Total tasks.
    pub total_tasks: u32,
    /// Progress percentage (0-100).
    pub progress_percent: f32,
    /// Declared dependencies.
    pub dependencies: Vec<String>,
    /// Apply/archive iteration number, when a loop is running.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_number: Option<u32>,
    /// Process-local execution mark. Never durable; `false` after a restart.
    pub execution_marked: bool,
    /// Reducer-owned queue intent, distinct from `display_status`.
    pub queue_intent: QueueIntent,
    /// Operator attention state.
    pub attention: AttentionState,
    /// Blocker detail for a `blocked` or `stalled` change; `null` otherwise.
    pub blocker: Option<ChangeBlocker>,
    /// Sanitized change-local error detail; `null` when the change has no error.
    pub error_detail: Option<String>,
    /// Eligibility for every change-addressed command.
    pub actions: ChangeActions,
    /// Parallel-execution eligibility.
    pub parallel: ParallelEligibility,
    /// Run timing boundaries.
    pub timing: ChangeTiming,
    /// Latest lifecycle-significant activity; `null` when nothing was observed.
    pub latest_activity: Option<ChangeActivity>,
    /// Managed worktree relation; `null` when no worktree exists.
    pub worktree: Option<ChangeWorktree>,
    /// Who decided the latest Acceptance settlement this owner incarnation
    /// observed, and about which candidate.
    ///
    /// Additive and nullable: `null` for a change with no settlement in this
    /// incarnation and after owner restart. Identities, counts, environment
    /// names, and closed tokens only — never a command argument value, an
    /// environment value, prompt text, or process output.
    ///
    /// `decision_source`: `native_verification` | `reviewer`.
    /// `reason_code`: `declaration_invalid` | `gate_failed` | `tool_unavailable` |
    /// `gate_deadline_exhausted` | `state_path_unavailable` |
    /// `legacy_target_evidence` | `manifest_malformed` | `runtime_defect` |
    /// `reviewer_pass` | `reviewer_fail` | `reviewer_stalled` |
    /// `reviewer_permission_stalled` | `reviewer_command_failed` |
    /// `reviewer_missing_verdict` | `review_deadline_exhausted` |
    /// `review_cleanup_unproven` | `cancelled`.
    /// `remediation`: `fix_declaration` | `satisfy_environment` |
    /// `repair_candidate` | `retry` | `refresh_candidate` | `none`.
    /// `candidate_divergence.status`: `identical` | `diverged` | `unknown`.
    ///
    /// Observability only: no command admission, retry classification,
    /// archive, merge, or next-action decision reads it.
    #[serde(default)]
    pub acceptance_provenance:
        Option<crate::orchestration::acceptance::provenance::AcceptanceProvenance>,
    /// Whether the reducer recorded archive completion for this change in this
    /// owner process.
    ///
    /// Read in the same reducer hold as `display_status` and distinct from it:
    /// stays `true` while the row advances to `resolving`, `resolve pending`,
    /// `merge wait`, `merged`, or `pushed`, and is `false` for a fresh-process
    /// resolve retry with no archive on record. Not merge or push success.
    /// Process-local; never a workflow input.
    ///
    /// `Option` with `#[serde(default)]` only so an owner built before this
    /// member decodes as `None` ("unpublished"), never as `false`. This server
    /// always writes it.
    #[serde(default)]
    #[schema(required, nullable = false)]
    pub archive_complete: Option<bool>,
    /// Cumulative net-active execution time; see `NetActiveTiming`.
    ///
    /// `Option` with `#[serde(default)]` only so an owner built before this
    /// member decodes as `None` ("unpublished"), never as zero. This server
    /// always writes it.
    #[serde(default)]
    #[schema(required, nullable = false)]
    pub net_active: Option<NetActiveTiming>,
}

/// Aggregate counts over the projected changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SnapshotTotals {
    /// Number of projected changes.
    pub total: usize,
    /// Changes in a completed terminal status.
    pub completed: usize,
    /// Changes actively executing.
    pub in_progress: usize,
    /// Changes queued but not started.
    pub pending: usize,
}

/// Coherent projection of everything a controller needs to decide the next command.
///
/// Deliberately excludes logs and timestamps: logs are an observational
/// resource, and a wall-clock field would make every projection look changed and
/// advance the revision forever.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct InstanceSnapshot {
    /// Operator-facing application mode.
    pub app_mode: String,
    /// Whether `app_mode: select` is backed by a live persistent-scheduler idle
    /// episode rather than ordinary pre-run selection.
    ///
    /// True means the scheduler task is still alive and parked with nothing to
    /// execute, so `start`, `stop`, and `force_stop` all remain meaningful. It is
    /// process-local presentation state: it defaults to false, resets on
    /// restart, and authorizes nothing — shared run control revalidates
    /// scheduler liveness before executing any command.
    ///
    /// A client that replaced its state after a replay gap can read this field
    /// to rebuild the idle Ready controls without replaying events or reading
    /// logs.
    #[serde(default)]
    pub persistent_scheduler_idle: bool,
    /// Whether merge resolution is currently running.
    pub is_resolving: bool,
    /// Sanitized detail of a fatal process-level error; `null` when there is none.
    ///
    /// Kept separate from a change's `error_detail` so "this run died" and "this
    /// one change failed" stay distinguishable without reading `app_mode` prose.
    pub process_error: Option<String>,
    /// Process-wide parallel execution runtime facts.
    pub parallel: ParallelRuntimeState,
    /// Projected changes.
    pub changes: Vec<ChangeResource>,
    /// Aggregate counts.
    pub totals: SnapshotTotals,
    /// Run-level elapsed clock; see `RunClock`.
    ///
    /// `Option` with `#[serde(default)]` only so an owner built before this
    /// member decodes as `None` ("unpublished"). This server always writes it.
    #[serde(default)]
    #[schema(required, nullable = false)]
    pub run_clock: Option<RunClock>,
}

impl InstanceSnapshot {
    /// The snapshot of a process that has not observed any state yet.
    pub fn empty() -> Self {
        Self {
            app_mode: "select".to_string(),
            persistent_scheduler_idle: false,
            is_resolving: false,
            process_error: None,
            parallel: ParallelRuntimeState::default(),
            changes: Vec::new(),
            totals: SnapshotTotals {
                total: 0,
                completed: 0,
                in_progress: 0,
                pending: 0,
            },
            run_clock: Some(RunClock::default()),
        }
    }
}

/// `GET /api/v2/state` body: a coherent snapshot with its revision and cursor.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StateResponse {
    /// Process incarnation ID this snapshot belongs to.
    pub instance_id: String,
    /// Revision of the returned snapshot.
    pub state_revision: u64,
    /// Latest allocated event sequence; a valid replay cursor.
    pub event_sequence: u64,
    /// The snapshot itself.
    pub snapshot: InstanceSnapshot,
}

/// `GET /api/v2/changes` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChangesResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the list was read at.
    pub state_revision: u64,
    /// Projected changes.
    pub changes: Vec<ChangeResource>,
}

/// `GET /api/v2/changes/{change_id}` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChangeResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the change was read at.
    pub state_revision: u64,
    /// The projected change.
    pub change: ChangeResource,
}

/// `GET /api/v2/logs` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LogsResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the logs were read at (logs never advance it).
    pub state_revision: u64,
    /// Latest allocated event sequence.
    pub event_sequence: u64,
    /// Retained log entries, oldest first.
    pub logs: Vec<crate::events::LogEntry>,
}

// ============================================================================
// Execution status
// ============================================================================
//
// `/state` answers "what may I command"; `/execution-status` answers "what is
// actually happening". The two are separate resources because they have
// different truth conditions: the snapshot is the authoritative operator
// decision state at `state_revision`, while everything below is explanatory
// process-local observation that must never become workflow-control authority.
//
// Every instant here is an absolute UTC RFC 3339 string. No elapsed counter, age
// counter, or relative-time phrase appears anywhere in this resource: a server
// that published one would advance its own revision forever, and a client that
// consumed one could not tell a stale response from a live one.

/// Closed per-change lifecycle phase vocabulary.
///
/// `merge` is reachable only as a *last completed* phase, because the reducer
/// has no merging activity to observe; `unknown` is the explicit value for
/// typed evidence that cannot be classified, never a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPhase {
    /// Admitted to a slot and preparing its managed workspace.
    Preparing,
    /// Running Apply.
    Apply,
    /// Running acceptance.
    Acceptance,
    /// Running dedicated rejection review.
    RejectionReview,
    /// Running archive.
    Archive,
    /// Running merge resolution.
    Resolve,
    /// A typed push episode is open.
    Push,
    /// A typed per-change merge completed. Never a current phase.
    Merge,
    /// No phase is active.
    None,
    /// Typed evidence exists but cannot be classified.
    Unknown,
}

impl ExecutionPhase {
    /// Project the shared orchestration phase onto the wire vocabulary.
    pub fn from_shared(phase: crate::orchestration::execution_facts::ExecutionPhase) -> Self {
        use crate::orchestration::execution_facts::ExecutionPhase as P;
        match phase {
            P::Preparing => Self::Preparing,
            P::Apply => Self::Apply,
            P::Acceptance => Self::Acceptance,
            P::RejectionReview => Self::RejectionReview,
            P::Archive => Self::Archive,
            P::Resolve => Self::Resolve,
            P::Push => Self::Push,
            P::Merge => Self::Merge,
            P::None => Self::None,
            P::Unknown => Self::Unknown,
        }
    }
}

/// Every phase value, in the order the contract advertises them.
#[allow(dead_code)] // Read by the OpenAPI contract assertions, not by the binary.
pub const ALL_EXECUTION_PHASES: [&str; 10] = [
    "preparing",
    "apply",
    "acceptance",
    "rejection_review",
    "archive",
    "resolve",
    "push",
    "merge",
    "none",
    "unknown",
];

/// Closed per-change execution-state vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeExecutionState {
    /// Requested to run and waiting for a slot.
    Queued,
    /// The reducer holds an active execution stage.
    Active,
    /// Held on a wait or blocker condition.
    Waiting,
    /// Active while a graceful process stop is in flight.
    Stopping,
    /// Stopped by operator request.
    Stopped,
    /// Reached a terminal failure or rejection.
    Failed,
    /// Reached a terminal success.
    Completed,
    /// Tracked, but no typed evidence classifies it.
    Unknown,
}

impl ChangeExecutionState {
    /// Project the shared orchestration state onto the wire vocabulary.
    pub fn from_shared(state: crate::orchestration::execution_facts::ChangeExecutionState) -> Self {
        use crate::orchestration::execution_facts::ChangeExecutionState as S;
        match state {
            S::Queued => Self::Queued,
            S::Active => Self::Active,
            S::Waiting => Self::Waiting,
            S::Stopping => Self::Stopping,
            S::Stopped => Self::Stopped,
            S::Failed => Self::Failed,
            S::Completed => Self::Completed,
            S::Unknown => Self::Unknown,
        }
    }
}

/// Every execution-state value, in the order the contract advertises them.
#[allow(dead_code)] // Read by the OpenAPI contract assertions, not by the binary.
pub const ALL_CHANGE_EXECUTION_STATES: [&str; 8] = [
    "queued",
    "active",
    "waiting",
    "stopping",
    "stopped",
    "failed",
    "completed",
    "unknown",
];

/// A closed, path-free projection of one retained structured log entry.
///
/// Deliberately *not* [`crate::events::LogEntry`]: that shape carries a
/// display-formatted `timestamp`, an epoch-seconds `created_at`, and a
/// `workspace_path`. This resource publishes an absolute RFC 3339 instant and no
/// filesystem locator at all, so a client can render a log line without ever
/// learning where the process keeps its files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LatestLogProjection {
    /// Sanitized, length-bounded message.
    pub message: String,
    /// Log level.
    pub level: crate::events::LogLevel,
    /// Operation the line came from, when the producer supplied one.
    pub operation: Option<String>,
    /// Iteration the line came from, when the producer supplied one.
    pub iteration: Option<u32>,
    /// Creation instant (absolute UTC RFC 3339).
    pub created_at: String,
}

impl LatestLogProjection {
    /// Project one retained entry, dropping every field that is not observable
    /// content.
    pub fn from_entry(entry: &crate::events::LogEntry) -> Self {
        Self {
            message: entry.message.clone(),
            level: entry.level,
            operation: entry.operation.clone(),
            iteration: entry.iteration,
            created_at: entry.created_at.to_rfc3339(),
        }
    }
}

/// Process-level execution facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProcessExecutionStatus {
    /// Operator-facing application mode.
    pub app_mode: String,
    /// Whether the scheduler task owning the current run state is alive.
    ///
    /// Read from the same process-local liveness authority operator snapshot
    /// eligibility and command admission use. A live scheduler parked with
    /// nothing to execute reports `true` here and `false` in `has_active_work`.
    pub scheduler_running: bool,
    /// Whether any per-change phase or process-level episode is running.
    pub has_active_work: bool,
    /// Closed process-level episodes that have started and not terminated.
    pub active_activities: Vec<String>,
    /// Latest retained structured log line, regardless of change; `null` when
    /// the ring is empty.
    pub latest_log: Option<LatestLogProjection>,
    /// Configured-command worktree sessions currently running.
    ///
    /// Computed on read. Independent of `has_active_work` and
    /// `scheduler_running`: a session is not lifecycle work.
    #[serde(default)]
    pub worktree_sessions_running: u32,
}

/// One change's execution facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChangeExecutionStatus {
    /// Change ID.
    pub id: String,
    /// Process-local identity of the most recent admitted execution episode.
    ///
    /// `null` until this incarnation admitted the change. Retained after the
    /// episode settles so a late subscriber can still address it, and replaced
    /// — never reused — by the next admission, which is what makes a retry a
    /// distinct execution rather than a continuation.
    ///
    /// Observability only: nothing in scheduling, acceptance, archive, merge,
    /// retry, or completion classification reads it.
    ///
    /// `#[serde(default)]` so a client built against this contract can still
    /// read an owner that predates execution identity.
    #[serde(default)]
    pub execution_id: Option<String>,
    /// Closed execution state.
    pub execution_state: ChangeExecutionState,
    /// Phase the reducer currently holds.
    pub current_phase: ExecutionPhase,
    /// Last phase that published its own typed completion fact; `null` when
    /// none did in this incarnation.
    pub last_completed_phase: Option<ExecutionPhase>,
    /// Apply/archive iteration number, when a loop is running.
    pub iteration: Option<u32>,
    /// When the current phase became active (absolute UTC RFC 3339).
    pub phase_started_at: Option<String>,
    /// When the last completed phase completed (absolute UTC RFC 3339).
    pub last_completed_at: Option<String>,
    /// When the current or most recent run started (absolute UTC RFC 3339).
    pub run_started_at: Option<String>,
    /// When that run finished (absolute UTC RFC 3339); `null` while running.
    pub run_completed_at: Option<String>,
    /// Latest lifecycle-significant activity; `null` when nothing was observed.
    pub latest_activity: Option<ChangeActivity>,
    /// Latest retained log whose structured `change_id` exactly equals `id`.
    pub latest_log: Option<LatestLogProjection>,
}

/// `GET /api/v2/execution-status` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExecutionStatusResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision of the snapshot this observation was joined against.
    ///
    /// A log-only observation changes `latest_log` and `event_sequence` while
    /// leaving this value alone: logs are observational and must never
    /// invalidate a client's optimistic concurrency token.
    pub state_revision: u64,
    /// Latest allocated event sequence; the observation cursor.
    pub event_sequence: u64,
    /// Server observation instant (absolute UTC RFC 3339).
    ///
    /// Present so a client can render relative time against the *server's*
    /// clock rather than its own. The API returns no relative time itself.
    pub observed_at: String,
    /// Process-level facts.
    pub process: ProcessExecutionStatus,
    /// Per-change facts, in snapshot order.
    pub changes: Vec<ChangeExecutionStatus>,
}

// ============================================================================
// Execution-scoped completion sinks
// ============================================================================
//
// A sink is a bounded argv command the owner runs *once* when one admitted
// execution reaches a typed terminal classification. It is observability, not
// control: nothing here is read back to decide a workflow action, delivery
// failure cannot roll anything back, and the whole registry dies with the
// process. What makes it useful is that it is scoped to one *execution* rather
// than to a change or to the process — a long-lived TUI stays alive after work
// finishes, so process exit was never a completion signal.

/// Closed vocabulary of events a completion sink can receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEventType {
    /// Repository evidence proved this owner's declared terminal mode.
    Completed,
    /// The owner reached a typed terminal unsuccessful state.
    Failed,
    /// A settled stop or dequeue removed the execution.
    Stopped,
    /// Optional, non-terminal attention: the execution entered a blocked state.
    Blocked,
    /// The owner is shutting down gracefully with this execution still live.
    OwnerStopping,
}

impl ExecutionEventType {
    /// Wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Blocked => "blocked",
            Self::OwnerStopping => "owner_stopping",
        }
    }

    /// Whether this event ends the execution's delivery budget.
    ///
    /// `blocked` and `owner_stopping` do not: the first is an attention edge a
    /// recovery can follow, and the second says the owner is leaving, not that
    /// the work reached an outcome.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Stopped)
    }
}

/// Every event type, in the order the contract advertises them.
#[allow(dead_code)] // Read by the OpenAPI contract assertions, not by the binary.
pub const ALL_EXECUTION_EVENT_TYPES: [ExecutionEventType; 5] = [
    ExecutionEventType::Completed,
    ExecutionEventType::Failed,
    ExecutionEventType::Stopped,
    ExecutionEventType::Blocked,
    ExecutionEventType::OwnerStopping,
];

/// The bounded callback attached to one execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExecutionSinkSpec {
    /// Callback argv, executed directly.
    ///
    /// Data, never shell source: element 0 is the program and the rest are its
    /// arguments, so no quoting, expansion, or `sh -c` interpretation exists to
    /// be exploited by a change ID or an event path.
    pub command: Vec<String>,
    /// Deliver the optional non-terminal `blocked` attention edge as well.
    ///
    /// Terminal events are never opt-out: a caller that could disable them
    /// would be registering a sink that can silently never fire.
    pub notify_blocked: bool,
}

/// `PUT /api/v2/executions/{execution_id}/sink` request body.
///
/// The binding fields are mandatory and are *checked*, not trusted: a sink that
/// attached itself to whatever execution currently holds an ID would silently
/// follow a retry it was never told about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSinkRequest {
    /// Owner incarnation the caller believes it is talking to.
    pub instance_id: String,
    /// Change the caller believes this execution belongs to.
    pub change_id: String,
    /// Callback argv.
    pub command: Vec<String>,
    /// Opt in to the non-terminal blocked attention edge.
    #[serde(default)]
    pub notify_blocked: bool,
}

/// Binding a `GET` or `DELETE` sink request asserts.
///
/// Both members are optional only at the parsing layer, so an omission is
/// reported as the typed validation refusal it is rather than as a malformed
/// query string. Every sink request — inspection included — has to present the
/// complete `(instance_id, execution_id, change_id)` binding.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExecutionSinkParams {
    /// Owner incarnation the caller believes it is talking to.
    pub instance_id: Option<String>,
    /// Change the caller believes this execution belongs to.
    pub change_id: Option<String>,
}

/// `GET`/`PUT`/`DELETE /api/v2/executions/{execution_id}/sink` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExecutionSinkResponse {
    /// Owner incarnation that answered.
    pub instance_id: String,
    /// The addressed execution.
    pub execution_id: String,
    /// Change the execution belongs to.
    pub change_id: String,
    /// Currently attached sink, argv included.
    ///
    /// `null` when none is attached — *and* when the request did not arrive on
    /// the owner's Unix socket. Reaching that socket already means local
    /// filesystem access to the owner's repository; a bearer token over TCP is a
    /// weaker claim than that, and the argv is a command this process will
    /// execute. Read [`ExecutionSinkResponse::sink_registered`] to tell "no sink"
    /// apart from "not disclosed here".
    pub sink: Option<ExecutionSinkSpec>,
    /// Whether a sink is attached at all.
    ///
    /// Always answered, on either transport, because subscription *presence* is
    /// not the secret — the argv is.
    #[serde(default)]
    pub sink_registered: bool,
    /// The execution's current closed state.
    pub execution_state: ChangeExecutionState,
    /// True once a terminal event has been dispatched for this execution.
    pub terminal_dispatched: bool,
    /// Event types already delivered, in delivery order.
    ///
    /// Typed rather than free strings: a client deciding whether it still owes
    /// a resume should branch on the vocabulary, not on spelling.
    pub delivered_events: Vec<ExecutionEventType>,
}

// ============================================================================
// Proposal-scoped subscriptions
// ============================================================================
//
// The wire identifier stays `change_id`; user-facing text calls the addressed
// change a *proposal*. Renaming the field would have broken every existing
// reader for a word, and the callback environment keeps `CFLX_CHANGE_ID` for
// the same reason.
//
// Why this is a different resource from the execution-scoped sink: an execution
// ID does not exist until the owner admits work, so an agent that wants to be
// told when its proposal finishes had no way to say so *before* admission. A
// proposal subscription is registered against the thing the operator actually
// names, and the owner binds each new execution episode of that proposal to it.

/// `PUT /api/v2/proposals/{change_id}/subscription` request body.
///
/// The instance binding is mandatory and checked rather than trusted: a
/// subscription that attached itself to whatever process currently answers the
/// socket would silently follow an owner the caller never observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalSubscriptionRequest {
    /// Owner incarnation the caller believes it is talking to.
    pub instance_id: String,
    /// Callback argv, executed directly and never as shell source.
    pub command: Vec<String>,
    /// Opt in to the non-terminal blocked attention edge.
    #[serde(default)]
    pub notify_blocked: bool,
}

/// Binding a `GET` or `DELETE` proposal-subscription request asserts.
///
/// Optional only at the parsing layer, so an omission is reported as the typed
/// validation refusal it is rather than as a malformed query string.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProposalSubscriptionParams {
    /// Owner incarnation the caller believes it is talking to.
    pub instance_id: Option<String>,
}

/// `GET`/`PUT`/`DELETE /api/v2/proposals/{change_id}/subscription` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProposalSubscriptionResponse {
    /// Owner incarnation that answered.
    pub instance_id: String,
    /// Proposal the subscription is keyed by. The wire name stays `change_id`.
    pub change_id: String,
    /// Currently registered callback, argv included.
    ///
    /// `null` when none is registered — *and* when the request did not arrive
    /// on the owner's Unix socket. Read [`ProposalSubscriptionResponse::subscribed`]
    /// to tell "no subscription" apart from "not disclosed here".
    pub sink: Option<ExecutionSinkSpec>,
    /// Whether a subscription is registered at all.
    ///
    /// Answered on either transport, because presence is not the secret — the
    /// argv is.
    #[serde(default)]
    pub subscribed: bool,
    /// Latest execution episode this owner bound to the proposal, when one exists.
    ///
    /// Absent before the first admission. A subscription is legal there: that
    /// absence is exactly the gap an execution-scoped registration could not
    /// cover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    /// The proposal's current closed execution state.
    pub execution_state: ChangeExecutionState,
    /// True once a terminal event was dispatched for the latest episode.
    pub terminal_dispatched: bool,
    /// Event types already delivered for the latest episode, in delivery order.
    pub delivered_events: Vec<ExecutionEventType>,
}

/// Whether proposal-scoped subscriptions can be registered at all.
///
/// Published as its own typed fact so a client discovers the surface instead of
/// inferring it from a 404: an owner that predates it has no
/// `/api/v2/proposals/...` route, and "the route is missing" and "this owner
/// refuses subscriptions" would otherwise be indistinguishable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct ProposalSubscriptionCapability {
    /// True when this build serves the proposal-subscription resources.
    pub available: bool,
    /// Most proposals one atomic request may address.
    pub max_targets: usize,
}

/// The versioned payload written to `CFLX_EVENT_PATH` for one delivery.
///
/// Bounded typed data and paths only. No prompt, terminal capture, environment
/// dump, credential, or unrestricted error body may appear here: the file is
/// handed to an arbitrary callback, and everything in it is data that callback
/// will treat as untrusted input.
///
/// Deliberately *not* an OpenAPI schema: it is never an HTTP request or response
/// body. It is an on-disk artifact handed to a local subprocess, and registering
/// an unreachable schema would make the published contract claim a surface no
/// operation serves. Its shape is documented in the API description instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionEventFile {
    /// Payload schema version.
    pub schema_version: u32,
    /// What happened.
    pub event_type: ExecutionEventType,
    /// Owner incarnation that produced the event.
    pub instance_id: String,
    /// Execution the event belongs to.
    pub execution_id: String,
    /// Change the execution belongs to.
    pub change_id: String,
    /// When the owner classified the event (absolute UTC RFC 3339).
    pub emitted_at: String,
    /// Whether this event ends the execution's delivery budget.
    pub terminal: bool,
    /// How this owner finishes changes, when it had published a contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_mode: Option<TerminalMode>,
    /// Bounded server-authored statement of the repository evidence that
    /// certified a `completed` event. Absent for every other type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Version of [`ExecutionEventFile`].
pub const EXECUTION_EVENT_SCHEMA_VERSION: u32 = 1;

// ============================================================================
// Owner execution contract
// ============================================================================
//
// The minimum a client needs to know *what would prove* that a change finished.
// It is deliberately not a completion record: it names the base branch, the
// terminal mode, and the remote/branch identity that mode publishes to, and
// stops there. Current Git and OpenSpec evidence certifies completion; nothing
// here is durable, per-change, or authoritative for routing.

/// How this owner finishes a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerminalMode {
    /// The change branch is merged into the local base branch.
    Merged,
    /// The verified cumulative base is additionally published to a remote.
    BasePublished,
    /// The change branch is pushed to a remote instead of merged to base.
    ///
    /// Publication, never base integration: a pushed branch proves the work
    /// left the machine, not that base contains it.
    BranchPushed,
}

impl TerminalMode {
    /// Wire representation, used by contract assertions.
    #[allow(dead_code)] // Read by the OpenAPI contract assertions, not by the binary.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merged => "merged",
            Self::BasePublished => "base_published",
            Self::BranchPushed => "branch_pushed",
        }
    }
}

/// Every terminal mode, in the order the contract advertises them.
#[allow(dead_code)] // Read by the OpenAPI contract assertions, not by the binary.
pub const ALL_TERMINAL_MODES: [&str; 3] = ["merged", "base_published", "branch_pushed"];

/// The minimal typed execution contract of one owner incarnation.
///
/// Inapplicable fields are *omitted*, not nulled: a `merged` owner has no
/// selected remote, and publishing an empty one would invite a client to build a
/// remote-ref check that can never pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OwnerExecutionContract {
    /// Base branch this owner integrates into and verifies completion against.
    pub base_branch: String,
    /// How a change reaches its terminal success.
    pub terminal_mode: TerminalMode,
    /// Remote selected for publication. Present for `base_published` and
    /// `branch_pushed` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// Server-derived local change branch that `branch_pushed` publishes.
    ///
    /// Present only when the request named a change *and* the mode is
    /// `branch_pushed`. The derivation stays server-side for the same reason
    /// worktree branches do: a client must never be able to point terminal proof
    /// at a ref it chose itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pushed_branch: Option<String>,
}

impl OwnerExecutionContract {
    /// Classify the terminal mode from the two mutually exclusive publication
    /// options an entrypoint resolved.
    ///
    /// The frontends already reject `--push` together with upstream
    /// integration, so the two are disjoint by construction; upstream wins the
    /// impossible case because the upstream lane is what would actually own the
    /// terminal boundary if it ever existed.
    pub fn resolve(
        base_branch: impl Into<String>,
        push_remote: Option<&str>,
        upstream_remote: Option<&str>,
    ) -> Self {
        let (terminal_mode, remote) = match (upstream_remote, push_remote) {
            (Some(remote), _) => (TerminalMode::BasePublished, Some(remote.to_string())),
            (None, Some(remote)) => (TerminalMode::BranchPushed, Some(remote.to_string())),
            (None, None) => (TerminalMode::Merged, None),
        };
        Self {
            base_branch: base_branch.into(),
            terminal_mode,
            remote,
            // Change-scoped and therefore resolved per request, never stored.
            pushed_branch: None,
        }
    }
}

/// `GET /api/v2/execution-contract` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExecutionContractResponse {
    /// Process incarnation this contract belongs to.
    pub instance_id: String,
    /// Revision the contract was read at, for joining with `/api/v2/state`.
    pub state_revision: u64,
    /// The contract; `null` while no orchestration runtime has published one.
    pub contract: Option<OwnerExecutionContract>,
}

// ============================================================================
// Typed command results
// ============================================================================

/// Proof that the final managed-worktree Apply commit exists.
///
/// `present` is nullable so unreadable or ambiguous evidence stays unknown
/// rather than collapsing into `false`, which a client would read as "Apply
/// definitely produced nothing".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ApplyCommitEvidence {
    /// True when the retained completion OID was proven to be in the managed
    /// worktree's history; `null` when the evidence could not be read.
    pub present: Option<bool>,
    /// The proven commit OID. Present only when `present` is `true`.
    ///
    /// A commit OID is repository identity, not a filesystem locator: no branch,
    /// worktree path, repository root, or log path accompanies it.
    pub oid: Option<String>,
}

/// The closed set of typed command results.
///
/// Optional on [`CommandRecord`] because most commands settle with their detail
/// alone. A variant is added only when a machine consumer would otherwise have
/// to parse prose or re-observe the system to learn what a command did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandResult {
    /// Settlement evidence for a successful `stop_and_dequeue`.
    StopAndDequeue {
        /// The typed phase active at settlement, immediately before dequeue.
        ///
        /// `none` for an already-terminated target or one with no active phase.
        /// It is never the phase seen at the originally admitted revision.
        cancelled_phase: ExecutionPhase,
        /// The last phase that published a typed completion fact.
        last_completed_phase: Option<ExecutionPhase>,
        /// Final Apply commit evidence for the managed worktree.
        apply_commit: ApplyCommitEvidence,
        /// Always false. Dequeue cancels and removes; it never undoes a
        /// completed worktree effect, and a client must not infer that it does.
        effects_rolled_back: bool,
    },
    /// Settlement evidence for a successful `force_stop_change`.
    ///
    /// A separate variant rather than a flag on the stop result: a client that
    /// branches on `kind` must be able to tell "this was killed immediately"
    /// from "this was asked to stop and then confirmed", because the two answer
    /// different questions about how the worktree was left.
    ForceStopChange {
        /// The one change this command addressed.
        change_id: String,
        /// Execution episode this cancelled; `null` when the target owned none.
        ///
        /// Read before cancellation, so it names the episode that was actually
        /// interrupted rather than whatever the runtime holds afterwards.
        execution_id: Option<String>,
        /// The typed phase active at settlement, immediately before dequeue.
        cancelled_phase: ExecutionPhase,
        /// The last phase that published a typed completion fact.
        last_completed_phase: Option<ExecutionPhase>,
        /// True when a live managed process group was SIGKILLed and proven reaped.
        ///
        /// `false` is the dequeue-only settlement: the target was admitted with
        /// nothing running, so no process was signalled at all.
        terminated: bool,
        /// Final Apply commit evidence for the managed worktree.
        apply_commit: ApplyCommitEvidence,
        /// Always false. An immediate kill stops work; it never undoes a
        /// completed worktree effect.
        effects_rolled_back: bool,
    },
    /// Settlement evidence for `refresh_proposal`, successful or refused.
    ///
    /// The one result kind a *failed* record also carries: a refusal after the
    /// write (a hook rejection, an unexpected tree) left real Git state behind,
    /// and the effects object is the only truthful account of it.
    RefreshProposal(Box<RefreshProposalResult>),
    /// Launch evidence for `run_worktree_command` and `create_session_worktree`.
    ///
    /// A failed `create_session_worktree` that already created a worktree also
    /// carries it, with `session_id: null`, so the setup and cleanup facts are
    /// not lost.
    WorktreeSession(Box<WorktreeSessionResult>),
    /// Settlement evidence for `stop_worktree_session`.
    WorktreeSessionStop {
        /// The session the command addressed.
        session_id: String,
        /// True when the runner cancelled it and proved the process group
        /// quiescent. `false` for a session that had already finished.
        terminated: bool,
        /// Session state after settlement.
        state_after: String,
    },
    /// Settlement evidence for `merge_worktree_abort_on_conflict`.
    ///
    /// Present on success and on every refusal after eligibility
    /// (`merge_conflict_predicted`, `merge_conflict_aborted`,
    /// `merge_abort_failed`), because a client must be able to tell a proven
    /// restoration from an unproven one without parsing prose.
    MergeAbort(Box<MergeAbortResult>),
    /// Settlement evidence for `delete_worktree_confirmed`, success or refusal.
    ///
    /// A refused record carries the same kind, with `refusal` holding the
    /// stable reason and the freshly observed facts, so a client can re-render
    /// its confirmation without a second preview.
    DeleteWorktreeConfirmed(Box<ConfirmedDeleteResult>),
    /// Settlement evidence for `write_change_text` / `write_worktree_text`.
    ///
    /// Carried by refusals too (`text_stale`, `text_not_editable`,
    /// `target_ineligible`), so a client reads `current_content_id` from the
    /// record rather than re-observing the file.
    TextWrite(Box<TextWriteResult>),
}

/// What a session launch (or a refused provisioning) produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorktreeSessionResult {
    /// The launched session; `null` when nothing was launched.
    pub session_id: Option<String>,
    /// `existing` or `session`.
    ///
    /// Not named `kind`: that member is the `CommandResult` discriminant, and a
    /// second `kind` would overwrite it on the wire.
    pub session_kind: String,
    /// Opaque worktree ID; `null` when no live worktree could be identified.
    pub worktree_id: Option<String>,
    /// Checked-out branch.
    pub branch: String,
    /// Repository-relative display path.
    pub path: String,
    /// Setup facts of a provisioned worktree; `null` for `existing`.
    pub setup: Option<SessionSetupFacts>,
    /// Setup-failure cleanup facts; `null` when no cleanup ran.
    pub cleanup: Option<SessionCleanupFacts>,
}

/// What `.wt/setup` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SessionSetupFacts {
    /// A `.wt/setup` script existed and ran.
    pub ran: bool,
    /// Setup did not fail.
    pub succeeded: bool,
}

/// What the setup-failure cleanup did.
///
/// `removed: true` means the worktree directory and registration are gone; the
/// `ws-session-*` branch is retained exactly as after a TUI `+` setup failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SessionCleanupFacts {
    /// `removed` or `retained`.
    pub outcome: String,
    /// True when the worktree is gone.
    pub removed: bool,
    /// Why removal failed, when it did.
    pub reason: Option<String>,
}

/// One retained output line of a session: bytes the child wrote.
///
/// Never a runner diagnostic: retry, launch-failure, shutdown, limit, and
/// cleanup facts are `runner_notices`, so a line here cannot change any
/// owner-generated member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SessionOutputLine {
    /// `stdout` or `stderr`.
    pub stream: String,
    /// The command's own text after control-sequence stripping and the 8 KiB
    /// line bound. Not redacted.
    pub text: String,
}

/// One runner-generated fact about a session, from a typed runner channel.
///
/// Never derived from child output. `detail` carries numbers and fixed tokens
/// only — never the template, the expanded command, environment values, or a
/// path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SessionRunnerNotice {
    /// `retry_scheduled`, `inactivity_terminated`, `runtime_limit_reached`,
    /// `launch_failed`, `shutdown_refused`, or
    /// `process_group_cleanup_unconfirmed`.
    pub kind: String,
    /// The attempt the notice concerns (for `retry_scheduled`, the attempt
    /// scheduled next); `null` when it concerns none.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub attempt: Option<u32>,
    /// Sanitized facts, for example `cause=crash, max_attempts=1, delay_ms=0`
    /// or `pgid=4242, force_killed=true`; `null` when the kind has none.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub detail: Option<String>,
}

/// Bounded, sanitized output tail of a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SessionOutput {
    /// Retained lines, oldest first.
    pub lines: Vec<SessionOutputLine>,
    /// Bytes of line text retained (at most 65,536).
    pub retained_bytes: usize,
    /// Bytes of line text dropped to stay within the bound.
    pub truncated_bytes: u64,
}

/// One configured-command worktree session.
///
/// Owner-generated members never carry the configured template, the expanded
/// command, environment values, absolute paths, or a process-group ID.
/// `output.lines` is different in kind: it is the command's own stdout/stderr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorktreeSessionResource {
    /// Random 128-bit process-local hexadecimal session ID.
    pub session_id: String,
    /// Process incarnation that owns the session.
    pub instance_id: String,
    /// `existing` or `session`.
    pub kind: String,
    /// Opaque worktree ID.
    pub worktree_id: Option<String>,
    /// Repository-relative display path.
    pub path: String,
    /// Checked-out branch.
    pub branch: String,
    /// `running`, `succeeded`, `failed`, or `stopped`.
    pub state: String,
    /// Runner attempts started inside this one session (configured retries):
    /// `1` at launch, then the attempt of each later successful spawn the
    /// runner reported on its typed channel. A retry merely scheduled does not
    /// count, and no output line can change it.
    pub attempt: u32,
    /// Launch instant (RFC 3339).
    pub started_at: String,
    /// Settlement instant (RFC 3339), once settled.
    pub finished_at: Option<String>,
    /// Exit code of the final attempt as reaped; `null` when a signal ended
    /// it or the leader was not reaped.
    pub exit_code: Option<i32>,
    /// The signal that terminated the leader as reaped (`SIGTERM`, or
    /// `SIGKILL` after escalation); `null` when unknown or when it exited on
    /// its own.
    pub signal: Option<String>,
    /// A stop or owner shutdown was requested.
    pub stop_requested: bool,
    /// Setup facts of a provisioned session.
    pub setup: Option<SessionSetupFacts>,
    /// Setup-failure cleanup facts; always `null` for a launched session.
    pub cleanup: Option<SessionCleanupFacts>,
    /// Bounded sanitized output tail: only bytes the child wrote.
    pub output: SessionOutput,
    /// Bounded runner-generated notices, oldest first. `null` from an older
    /// owner, which publishes none ("unpublished", not "empty").
    #[serde(default)]
    pub runner_notices: Option<Vec<SessionRunnerNotice>>,
    /// Runner notices dropped to stay within `max_runner_notices`; `null`
    /// from an older owner.
    #[serde(default)]
    pub runner_notices_dropped: Option<u64>,
}

/// `GET /api/v2/worktree-sessions` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorktreeSessionsResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the list was read at. Sessions never advance it.
    pub state_revision: u64,
    /// Retained sessions, newest first.
    pub sessions: Vec<WorktreeSessionResource>,
}

/// `GET /api/v2/worktree-sessions/{session_id}` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorktreeSessionResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the session was read at.
    pub state_revision: u64,
    /// The requested session.
    pub session: WorktreeSessionResource,
}

/// Why a confirmed delete was refused, with what the service just observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConfirmedDeleteRefusalResult {
    /// The stable reason; `null` for a refusal outside the consent vocabulary.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub reason: Option<crate::worktree_ops::consent::ConfirmedDeleteReason>,
    /// True when `.wt/teardown` ran before the refusal. Its effects are not
    /// reversed; the worktree was retained.
    pub teardown_ran: bool,
    /// The freshest observation; `null` when the worktree was not observable.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub observed: Option<crate::worktree_ops::consent::ConsentObservation>,
}

/// The settled body of a `delete_worktree_confirmed` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConfirmedDeleteResult {
    /// The addressed opaque worktree ID.
    pub worktree_id: String,
    /// Branch of the worktree; the consented one when nothing was observed.
    pub branch: String,
    /// HEAD of the worktree; the consented one when nothing was observed.
    pub head: String,
    /// `deleted` on success; `null` when refused.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub outcome: Option<String>,
    /// What happened to `.wt/teardown`.
    pub teardown: crate::worktree_ops::consent::TeardownDisposition,
    /// True when known uncommitted or ignored content was discarded.
    pub dirty_discarded: bool,
    /// True when commits ahead of base were discarded with their branch.
    pub commits_discarded: bool,
    /// True when the worktree's branch was deleted.
    pub branch_deleted: bool,
    /// Why the branch was kept, when the worktree was removed but it was not.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub branch_retained_reason: Option<String>,
    /// The refusal; `null` on success.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub refusal: Option<ConfirmedDeleteRefusalResult>,
}

/// How an abort-on-conflict merge settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeAbortOutcome {
    /// The merge completed.
    Merged,
    /// Refused before any Git mutation.
    PredictedConflict,
    /// Conflicted, aborted, and restoration was confirmed.
    ConflictAborted,
    /// Conflicted, and restoration was not confirmed.
    AbortFailed,
    /// Merged into base, then the `on_merged` hook failed. Decided by the
    /// operation stage; `merge.landed` is re-read separately.
    MergedHookFailed,
    /// A Git failure after eligibility and before any merge commit (a dirty
    /// base, an unmergeable ref). `merge.landed` is re-read separately.
    MergeFailed,
}

/// Whether a base merge commit landed, re-read from the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeLanded {
    /// `HEAD` moved off `premerge_head`, descends from it, and no `MERGE_HEAD` exists.
    Confirmed,
    /// `HEAD` is still `premerge_head` and no `MERGE_HEAD` exists.
    NotLanded,
    /// A fact could not be read, or the facts support neither verdict; `detail` names which.
    Unknown,
}

impl From<crate::vcs::git::commands::Landing> for MergeLanded {
    fn from(value: crate::vcs::git::commands::Landing) -> Self {
        use crate::vcs::git::commands::Landing;
        match value {
            Landing::Confirmed => Self::Confirmed,
            Landing::NotLanded => Self::NotLanded,
            Landing::Unknown => Self::Unknown,
        }
    }
}

/// Whether the merge commit landed on base, re-read from Git after the command stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MergeLandingEvidence {
    /// The verdict derived from the facts below, never from the attempt.
    pub landed: MergeLanded,
    /// Base `HEAD` as re-read; `null` when it could not be read.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub head_after: Option<String>,
    /// Whether `MERGE_HEAD` exists; `null` when unreadable.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub merge_head_present: Option<bool>,
    /// Whether the root is clean; informational, never changes `landed`. `null` when unreadable.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub clean_after: Option<bool>,
    /// Which verification failed or contradicted when `landed` is `unknown`; else `null`.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub detail: Option<String>,
}

impl MergeLandingEvidence {
    /// Project the VCS landing report onto the wire.
    pub fn from_report(report: &crate::vcs::git::commands::LandingReport) -> Self {
        Self {
            landed: report.landed.into(),
            head_after: report.head_after.clone(),
            merge_head_present: report.merge_head_present,
            clean_after: report.clean_after,
            detail: report.detail.clone(),
        }
    }
}

/// What the `on_merged` hook did during an abort-on-conflict merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MergeHookEvidence {
    /// True when the hook was invoked (exactly once).
    pub ran: bool,
    /// Whether the hook succeeded; `null` when it did not run.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub exit_ok: Option<bool>,
    /// Bounded, sanitized hook failure text; `null` unless the hook failed.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub detail: Option<String>,
}

impl MergeHookEvidence {
    /// The hook was not invoked.
    pub fn not_run() -> Self {
        Self {
            ran: false,
            exit_ok: None,
            detail: None,
        }
    }
}

/// How far the root was proven restored after `git merge --abort`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeAbortRestoration {
    /// `HEAD` is the pre-merge commit, no `MERGE_HEAD`, and the root is clean.
    Confirmed,
    /// Every verification was read and at least one failed.
    Failed,
    /// At least one verification could not be read.
    Unknown,
}

impl From<crate::vcs::git::commands::Restoration> for MergeAbortRestoration {
    fn from(value: crate::vcs::git::commands::Restoration) -> Self {
        use crate::vcs::git::commands::Restoration;
        match value {
            Restoration::Confirmed => Self::Confirmed,
            Restoration::Failed => Self::Failed,
            Restoration::Unknown => Self::Unknown,
        }
    }
}

/// What `git merge --abort` did, re-read from the repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MergeAbortEvidence {
    /// Exit status of `git merge --abort`.
    pub exit_ok: bool,
    /// `HEAD` after the abort; `null` when it could not be read.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub head_after: Option<String>,
    /// Whether `MERGE_HEAD` exists after the abort; `null` when unreadable.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub merge_head_present: Option<bool>,
    /// Whether the root is clean after the abort; `null` when unreadable.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub clean_after: Option<bool>,
    /// The verdict derived from the facts above, never from the attempt alone.
    pub restored: MergeAbortRestoration,
    /// Bounded, sanitized tail of the abort's own output.
    pub detail: String,
}

impl MergeAbortEvidence {
    /// Project the VCS report onto the wire.
    pub fn from_report(report: &crate::vcs::git::commands::AbortReport) -> Self {
        Self {
            exit_ok: report.exit_ok,
            head_after: report.head_after.clone(),
            merge_head_present: report.merge_head_present,
            clean_after: report.clean_after,
            restored: report.restored.into(),
            detail: report.detail.clone(),
        }
    }
}

/// The settled body of a `merge_worktree_abort_on_conflict` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MergeAbortResult {
    /// The addressed opaque worktree ID.
    pub worktree_id: String,
    /// The merged branch; empty when it could not be observed.
    pub branch: String,
    /// How the command settled.
    pub outcome: MergeAbortOutcome,
    /// `HEAD` before `git merge` ran; `null` when no merge ran.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub premerge_head: Option<String>,
    /// Repository-relative conflicting paths; empty after a clean merge.
    pub conflict_files: Vec<String>,
    /// The verified abort report; `null` when no abort ran.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub abort: Option<MergeAbortEvidence>,
    /// `local_or_tui_required` when restoration was not confirmed or a hook
    /// failed after a landed merge; else `null`.
    // Published as required: always written, `null` when it does not apply.
    #[schema(required)]
    pub recovery: Option<String>,
    /// Whether the merge commit landed on base, re-read from Git; `null` when
    /// not reported. Omitted by older owners, which decodes as `null`.
    #[serde(default)]
    pub merge: Option<MergeLandingEvidence>,
    /// What the `on_merged` hook did; `null` when not reported. Omitted by
    /// older owners, which decodes as `null`.
    #[serde(default)]
    pub hook: Option<MergeHookEvidence>,
}

/// What a candidate proposal refresh actually did, re-read from Git.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RefreshEffects {
    /// The command wrote its target file.
    pub file_written: bool,
    /// The command's staging step completed (not whether changes remain staged).
    pub staged: bool,
    /// `HEAD` was observed to have moved off `expected_head`.
    pub committed: bool,
    /// The candidate worktree was observed clean when the command finished.
    pub clean_after: bool,
}

/// The settled body of a `refresh_proposal` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RefreshProposalResult {
    /// Target change.
    pub change_id: String,
    /// `refreshed` or `unchanged`; `null` when refused.
    pub outcome: Option<String>,
    /// Stable refusal reason; `null` on success.
    pub reason: Option<String>,
    /// The ref the caller named.
    pub source_ref: String,
    /// Resolved source commit, when it resolved.
    pub source_commit: Option<String>,
    /// Source blob identity, when read.
    pub source_blob: Option<String>,
    /// Candidate `HEAD` before the command, when read.
    pub candidate_head_before: Option<String>,
    /// Candidate `HEAD` after the command, when read.
    pub candidate_head_after: Option<String>,
    /// Candidate blob identity before the command, when read.
    pub candidate_blob_before: Option<String>,
    /// Repository-relative proposal path.
    pub path: String,
    /// Observed effects.
    pub effects: RefreshEffects,
    /// Bounded tail of hook / commit output, when a commit ran.
    pub hook_output_tail: Option<String>,
    /// Bounded diagnostic detail.
    pub detail: Option<String>,
}

impl RefreshProposalResult {
    /// Project the shared report onto the wire.
    pub fn from_report(report: &crate::orchestration::proposal_refresh::RefreshReport) -> Self {
        Self {
            change_id: report.change_id.clone(),
            outcome: report.outcome.map(|outcome| outcome.as_str().to_string()),
            reason: report.refusal.map(|reason| reason.as_str().to_string()),
            source_ref: report.source_ref.clone(),
            source_commit: report.source_commit.clone(),
            source_blob: report.source_blob.clone(),
            candidate_head_before: report.candidate_head_before.clone(),
            candidate_head_after: report.candidate_head_after.clone(),
            candidate_blob_before: report.candidate_blob_before.clone(),
            path: report.path.clone(),
            effects: RefreshEffects {
                file_written: report.effects.file_written,
                staged: report.effects.staged,
                committed: report.effects.committed,
                clean_after: report.effects.clean_after,
            },
            hook_output_tail: report.hook_output_tail.clone(),
            detail: report.detail.clone(),
        }
    }
}

/// `GET /api/v2/proposals/{change_id}/refresh-preview` body.
///
/// A read: it creates no command record and advances no revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RefreshPreviewResponse {
    /// Process incarnation the preview was computed in.
    pub instance_id: String,
    /// Revision the lifecycle eligibility was read at.
    pub state_revision: u64,
    /// Target change.
    pub change_id: String,
    /// True when no lifecycle or command-time precondition refuses.
    pub eligible: bool,
    /// The first refusal found, when not eligible.
    pub refusal: Option<String>,
    /// The ref the caller named.
    pub source_ref: String,
    /// Resolved source commit — pass it back as `expected_source`.
    pub source_commit: Option<String>,
    /// Current candidate `HEAD` — pass it back as `expected_head`.
    pub candidate_head: Option<String>,
    /// Repository-relative proposal path.
    pub path: String,
    /// Source blob identity.
    pub source_blob: Option<String>,
    /// Candidate blob identity.
    pub candidate_blob: Option<String>,
    /// True when the two blobs are identical and an apply would be `unchanged`.
    pub no_op: bool,
    /// Unified patch from candidate to source; `null` when not computed.
    pub patch: Option<String>,
    /// Lines the patch adds.
    pub added_lines: usize,
    /// Lines the patch removes.
    pub removed_lines: usize,
    /// Bounded diagnostic detail for a refusal.
    pub detail: Option<String>,
}

impl RefreshPreviewResponse {
    /// Project a shared preview onto the wire at one observed identity.
    pub fn from_preview(
        preview: crate::orchestration::proposal_refresh::RefreshPreview,
        instance_id: String,
        state_revision: u64,
    ) -> Self {
        Self {
            instance_id,
            state_revision,
            change_id: preview.change_id,
            eligible: preview.eligible,
            refusal: preview.refusal.map(|reason| reason.as_str().to_string()),
            source_ref: preview.source_ref,
            source_commit: preview.source_commit,
            candidate_head: preview.candidate_head,
            path: preview.path,
            source_blob: preview.source_blob,
            candidate_blob: preview.candidate_blob,
            no_op: preview.no_op,
            patch: preview.patch,
            added_lines: preview.added_lines,
            removed_lines: preview.removed_lines,
            detail: preview.detail,
        }
    }
}

/// Whether candidate proposal refresh is served, and where.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct ProposalRefreshCapability {
    /// True when this build serves the preview resource.
    pub preview_available: bool,
    /// Route template of the read-only preview.
    pub preview_route: String,
    /// Command type an apply is submitted as.
    pub command: String,
    /// Every stable refusal reason, lifecycle and command time.
    pub reasons: Vec<String>,
}

// ============================================================================
// Bounded text edit
// ============================================================================

/// Which root a text read or write addresses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextScope {
    /// `openspec/changes/<change_id>` of the project.
    Change {
        /// Tracked, non-archived change.
        change_id: String,
    },
    /// The directory a live opaque worktree ID is bound to.
    Worktree {
        /// Opaque process-local worktree ID.
        worktree_id: String,
    },
}

/// Why a target that holds editable text cannot be written right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextEditBlockedReason {
    /// The change (for a worktree, its owning change) is in an active status.
    ChangeActive,
    /// The change is admitted: queue intent queued, or status `queued`/`blocked`.
    ChangeAdmitted,
    /// Managed writer coverage for the root is not `Clear`: a managed writer is
    /// bound here, or the owner cannot prove that none is. `edit_blocked_detail`
    /// tells the two apart.
    ManagedProcessLive,
    /// The repository root guard is held, a base merge is in progress, or a
    /// resolve is running.
    RootBusy,
}

impl TextEditBlockedReason {
    /// Wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChangeActive => "change_active",
            Self::ChangeAdmitted => "change_admitted",
            Self::ManagedProcessLive => "managed_process_live",
            Self::RootBusy => "root_busy",
        }
    }
}

/// The surface's fixed bounds, repeated on every read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TextEditLimits {
    /// Largest editable file, in bytes.
    pub max_bytes: u64,
    /// Longest accepted relative path, in bytes.
    pub max_path_bytes: usize,
    /// Algorithm of `content_id`; always `sha256`.
    pub content_id_algorithm: String,
}

/// `GET /api/v2/changes/{change_id}/text` and
/// `GET /api/v2/worktrees/{worktree_id}/text` body.
///
/// A read: no command record, no revision change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TextReadResponse {
    /// Process incarnation ID.
    pub instance_id: String,
    /// Revision the eligibility was read at.
    pub state_revision: u64,
    /// The addressed root.
    pub scope: TextScope,
    /// The root-relative path, as requested.
    pub path: String,
    /// False when the file does not exist; a write may then create it with
    /// `expected.kind = absent`.
    pub exists: bool,
    /// The file's bytes as UTF-8; `null` when absent.
    #[schema(required)]
    pub content: Option<String>,
    /// `sha256:<hex>` of the bytes; `null` when absent.
    #[schema(required)]
    pub content_id: Option<String>,
    /// Byte length; `null` when absent.
    #[schema(required)]
    pub size_bytes: Option<u64>,
    /// Octal permission bits, for example `0644`; `null` when absent.
    #[schema(required)]
    pub mode: Option<String>,
    /// True when a write would pass writer exclusion right now.
    pub editable: bool,
    /// Why not, when `editable` is false.
    #[schema(required)]
    pub edit_blocked_reason: Option<TextEditBlockedReason>,
    /// Bounded diagnostic for the refusal; never parsed by clients.
    #[schema(required)]
    pub edit_blocked_detail: Option<String>,
    /// The surface's fixed bounds.
    pub limits: TextEditLimits,
}

/// How a text write settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextWriteOutcome {
    /// The bytes were replaced (or the file created).
    Written,
    /// The new content equals the file; nothing was opened for writing.
    Unchanged,
}

/// What a text write actually did to the filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TextWriteEffects {
    /// True only when the target file now holds the new bytes.
    pub file_written: bool,
}

/// The settled body of a text write, successful or refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TextWriteResult {
    /// The addressed root.
    pub scope: TextScope,
    /// The root-relative path.
    pub path: String,
    /// `written` or `unchanged`; `null` when refused.
    #[schema(required)]
    pub outcome: Option<TextWriteOutcome>,
    /// Stable refusal reason (a `text_not_editable` or `target_ineligible`
    /// token, or `stale`); `null` on success.
    #[schema(required)]
    pub reason: Option<String>,
    /// Bounded diagnostic; for `managed_process_live`, the coverage detail.
    #[schema(required)]
    pub detail: Option<String>,
    /// Identity the file held before the write; `null` when it was absent.
    #[schema(required)]
    pub content_id_before: Option<String>,
    /// Identity re-read after the write; `null` when nothing was written.
    #[schema(required)]
    pub content_id_after: Option<String>,
    /// For `text_stale`: what the file holds now; `null` when it is absent.
    #[schema(required)]
    pub current_content_id: Option<String>,
    /// Bytes written; `null` when nothing was written.
    #[schema(required)]
    pub bytes_written: Option<u64>,
    /// Octal permission bits after the write; `null` when nothing was written.
    #[schema(required)]
    pub mode: Option<String>,
    /// What the filesystem holds as a result.
    pub effects: TextWriteEffects,
}

/// Whether the bounded text edit surface is served, and its bounds.
///
/// `Default` is the *unavailable* answer, which is what an older owner's
/// omission decodes as.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct TextEditCapability {
    /// True when this build serves the text routes and commands.
    pub available: bool,
    /// Route templates of the two reads.
    pub read_routes: Vec<String>,
    /// Command types a write is submitted as.
    pub commands: Vec<String>,
    /// Largest editable file, in bytes.
    pub max_bytes: u64,
    /// Longest accepted relative path, in bytes.
    pub max_path_bytes: usize,
    /// Algorithm of `content_id`.
    pub content_id_algorithm: String,
    /// Paths and kinds that are never editable.
    pub excluded: Vec<String>,
    /// Every stable refusal reason, containment and writer exclusion.
    pub reasons: Vec<String>,
}

// ============================================================================
// Events
// ============================================================================

/// What an event envelope carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    /// A state-affecting orchestration event.
    State,
    /// An observational log entry; never advances the revision.
    Log,
    /// The requested cursor is no longer replayable.
    Gap,
}

/// One ordered event on the v2 stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct EventEnvelope {
    /// Process incarnation that allocated this sequence.
    pub instance_id: String,
    /// Monotonic process-local sequence number.
    pub event_sequence: u64,
    /// State revision associated with this event.
    pub state_revision: u64,
    /// Envelope category.
    pub category: EventCategory,
    /// Specific event kind (for example `processing_started`).
    pub event_type: String,
    /// Emission time (RFC 3339).
    pub timestamp: String,
    /// Change this event is about, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_id: Option<String>,
    /// Kind-specific detail.
    pub payload: serde_json::Value,
}
