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
            | Self::ExecutionBindingMismatch => StatusCode::CONFLICT,
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
            Self::ExecutionBindingMismatch => "execution_binding_mismatch",
            Self::TransportNotPermitted => "transport_not_permitted",
            Self::InternalError => "internal_error",
        }
    }
}

/// Every error code, in the order advertised by `/api/v2/capabilities`.
pub const ALL_ERROR_CODES: [ErrorCode; 19] = [
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
    ErrorCode::ExecutionBindingMismatch,
    ErrorCode::TransportNotPermitted,
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
            Self::SetAllExecutionMarks { .. } => "set_all_execution_marks",
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
            // A process-wide mutation: it addresses the whole target set, never
            // one change.
            Self::SetAllExecutionMarks { .. } => None,
            // Worktree mutations are addressed by opaque ID, not by change.
            Self::DeleteWorktree { .. } | Self::MergeWorktree { .. } => None,
        }
    }
}

/// Every supported command type, in the order advertised by capabilities.
pub const SUPPORTED_COMMANDS: [&str; 14] = [
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
    /// Retired: the server no longer refuses an action for this reason.
    ///
    /// The Apply-iteration ceiling is diagnostic evidence about the invocation
    /// that stopped, not an operator-action block, so no current projection or
    /// command admission produces this token. The variant remains published so a
    /// client generated against an older contract — or one still handling a
    /// recorded snapshot that carries it — keeps deserializing rather than
    /// failing on an unknown reason.
    ApplyIterationLimitActive,
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
