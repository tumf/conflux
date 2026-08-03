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
            | Self::TargetIneligible
            | Self::RootBusy
            | Self::IdempotencyMismatch
            | Self::WorktreeExists
            | Self::WorktreeDirty
            | Self::WorktreeDirtyUnknown
            | Self::MergeConflict => StatusCode::CONFLICT,
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
            Self::InternalError => "internal_error",
        }
    }
}

/// Every error code, in the order advertised by `/api/v2/capabilities`.
pub const ALL_ERROR_CODES: [ErrorCode; 16] = [
    ErrorCode::Unauthorized,
    ErrorCode::Forbidden,
    ErrorCode::NotFound,
    ErrorCode::StaleRevision,
    ErrorCode::LifecycleConflict,
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
    ErrorCode::InternalError,
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
pub fn new_hex_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}

// ============================================================================
// Commands
// ============================================================================

/// The closed v2 command set.
///
/// Adding a variant is a spec change: an envelope naming any other type fails
/// typed validation before a service call can happen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
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
    /// Request merge resolution for a change waiting on a merge.
    ResolveMerge {
        /// Target change.
        change_id: String,
    },
    /// Turn parallel execution on or off for the whole process.
    ///
    /// Accepted only in Select or Stopped mode. Enabling it also clears the mark
    /// and queue intent of every change parallel mode refuses, and the outcome
    /// names them.
    SetParallelMode {
        /// Requested toggle value.
        enabled: bool,
    },
    /// Apply one derived execution-mark state to every eligible change.
    ///
    /// Deliberately parameterless: the target state is derived from the eligible
    /// rows at the admitted revision, exactly as the TUI's bulk toggle does, so
    /// a client cannot ask for a target set the server never classified.
    SetAllExecutionMarks,
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

/// A parameter object that accepts nothing.
///
/// Present so the wire shape is stable if a parameter is ever added, and so that
/// every unrecognized parameter fails validation today.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

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
            Self::ResolveMerge { .. } => "resolve_merge",
            Self::SetParallelMode { .. } => "set_parallel_mode",
            Self::SetAllExecutionMarks => "set_all_execution_marks",
            Self::CreateWorktree { .. } => "create_worktree",
            Self::DeleteWorktree { .. } => "delete_worktree",
            Self::MergeWorktree { .. } => "merge_worktree",
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
            | Self::ResolveMerge { change_id } => Some(change_id),
            Self::CreateWorktree { target, .. } => Some(&target.change_id),
            Self::Start | Self::Stop | Self::CancelStop | Self::ForceStop => None,
            Self::RetryErrors { .. } => None,
            // Process-wide mutations: they address the whole target set, never
            // one change.
            Self::SetParallelMode { .. } | Self::SetAllExecutionMarks => None,
            // Worktree mutations are addressed by opaque ID, not by change.
            Self::DeleteWorktree { .. } | Self::MergeWorktree { .. } => None,
        }
    }
}

/// Every supported command type, in the order advertised by capabilities.
pub const SUPPORTED_COMMANDS: [&str; 15] = [
    "start",
    "stop",
    "cancel_stop",
    "force_stop",
    "set_execution_mark",
    "set_queue_intent",
    "retry_change",
    "retry_errors",
    "stop_and_dequeue",
    "resolve_merge",
    "set_parallel_mode",
    "set_all_execution_marks",
    "create_worktree",
    "delete_worktree",
    "merge_worktree",
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
    /// The complete worktree surface, including its conflict-recovery boundary.
    pub worktrees: super::worktrees::WorktreeCapabilities,
    /// The parallel execution surface, including its blocked-reason vocabulary.
    pub parallel: ParallelCapabilities,
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
    /// Parallel mode refuses a change that is not committed cleanly yet.
    ParallelIneligible,
}

impl ActionBlockedReason {
    /// Project a shared bulk-mark exclusion onto the wire vocabulary.
    ///
    /// One vocabulary, not two: the reason a bulk mutation skipped a row is the
    /// same reason the row's own `actions` block reports.
    pub fn from_mark_exclusion(
        exclusion: crate::orchestration::operator_command::MarkExclusion,
    ) -> Self {
        use crate::orchestration::operator_command::MarkExclusion as E;
        match exclusion {
            E::FinalStatus => Self::FinalStatus,
            E::RetryRequired => Self::RetryRequired,
            E::StopPending => Self::StopPending,
            E::ChangeActive => Self::ChangeActive,
            E::StatusImmutable => Self::StatusImmutable,
            E::ParallelIneligible => Self::ParallelIneligible,
        }
    }

    /// Stable wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FinalStatus => "final_status",
            Self::RetryRequired => "retry_required",
            Self::StopPending => "stop_pending",
            Self::StatusImmutable => "status_immutable",
            Self::ModeHasNoQueue => "mode_has_no_queue",
            Self::NoRetryableEvidence => "no_retryable_evidence",
            Self::HoldNotResumable => "hold_not_resumable",
            Self::ChangeActive => "change_active",
            Self::NotMergeWaiting => "not_merge_waiting",
            Self::ParallelIneligible => "parallel_ineligible",
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
    /// `resolve_merge`.
    pub resolve_merge: ActionEligibility,
}

/// Why a change cannot take part in parallel execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParallelBlockedReason {
    /// The change does not exist in the base commit tree.
    NotCommitted,
    /// The change has uncommitted or untracked files under its directory.
    UncommittedChanges,
}

/// Every parallel-eligibility blocked reason, in capability-advertised order.
pub const ALL_PARALLEL_BLOCKED_REASONS: [&str; 2] = ["not_committed", "uncommitted_changes"];

/// The sequential/parallel execution mode a run would use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParallelMode {
    /// One change at a time, in the repository root.
    #[default]
    Sequential,
    /// Multiple changes concurrently, each in its own worktree.
    Parallel,
}

impl ParallelMode {
    /// Resolve the mode from the shared runtime toggle.
    pub fn from_enabled(enabled: bool) -> Self {
        if enabled {
            Self::Parallel
        } else {
            Self::Sequential
        }
    }

    /// True when parallel execution is the active mode.
    pub fn is_parallel(self) -> bool {
        matches!(self, Self::Parallel)
    }
}

/// Process-wide parallel execution runtime facts.
///
/// A client reads this instead of inferring the mode from how many changes
/// happen to be running: `available` distinguishes "parallel is off" from
/// "parallel cannot be turned on here", which are different operator problems.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ParallelRuntimeState {
    /// Active execution mode.
    pub mode: ParallelMode,
    /// True when parallel execution can be enabled at all (requires Git).
    pub available: bool,
    /// Maximum number of concurrently executing changes.
    pub max_concurrent: usize,
    /// VCS backend a run would use.
    pub vcs_backend: String,
}

/// Parallel execution surface advertised by `/api/v2/capabilities`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ParallelCapabilities {
    /// True when parallel execution can be enabled at all.
    pub available: bool,
    /// Active execution mode at the moment capabilities were read.
    pub mode: ParallelMode,
    /// Maximum number of concurrently executing changes.
    pub max_concurrent: usize,
    /// VCS backend a run would use.
    pub vcs_backend: String,
    /// Every machine-readable per-change eligibility blocked reason.
    pub blocked_reasons: Vec<String>,
    /// Modes that accept `set_parallel_mode`.
    pub toggle_modes: Vec<String>,
}

/// Server-observed parallel-execution eligibility.
///
/// Present so a client never has to run Git itself to decide whether a change
/// can be queued in parallel mode.
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChangeTiming {
    /// When the current or most recent run started (RFC 3339).
    pub started_at: Option<String>,
    /// When it finished (RFC 3339); `null` while it is still running.
    pub completed_at: Option<String>,
    /// Duration of the finished run in milliseconds.
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
}

impl InstanceSnapshot {
    /// The snapshot of a process that has not observed any state yet.
    pub fn empty() -> Self {
        Self {
            app_mode: "select".to_string(),
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
