//! Boundaries the upstream coordinator drives.
//!
//! The coordinator owns every routing decision; these ports only *observe* and
//! *execute*. Splitting them out is what makes the workflow unit-testable: unit
//! coverage drives the coordinator through in-memory doubles, while real Git,
//! process, and network behavior is exercised by heavy E2E tests.

use async_trait::async_trait;

use super::classify::MergeRepositoryState;
use super::spine::SpineCommit;

/// Errors surfaced by an upstream port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamPortError {
    /// Operation that failed, for operator-visible diagnostics.
    pub operation: String,
    /// Sanitized message. Never used for workflow routing.
    pub message: String,
}

impl UpstreamPortError {
    pub fn new(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for UpstreamPortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} failed: {}", self.operation, self.message)
    }
}

impl std::error::Error for UpstreamPortError {}

pub type PortResult<T> = std::result::Result<T, UpstreamPortError>;

/// Raw result of a native merge command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeCommandResult {
    pub exit_success: bool,
    /// Repository evidence observed immediately after the command.
    pub state: MergeRepositoryState,
}

/// Raw result of `git push --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushCommandResult {
    pub exit_success: bool,
    /// Machine-readable porcelain stdout. Stderr is deliberately not captured
    /// for routing; only sanitized diagnostics may reference it.
    pub porcelain_stdout: String,
}

/// Native Git operations owned by Conflux.
///
/// Every method here runs outside `AgentRunner` and the AI command harness.
#[async_trait]
pub trait UpstreamGit: Send + Sync {
    /// Whether the named remote is configured locally.
    async fn remote_configured(&self, remote: &str) -> PortResult<bool>;

    /// Branch HEAD is attached to, or `None` for a detached HEAD.
    async fn current_branch(&self) -> PortResult<Option<String>>;

    /// Fetch the selected remote's same-name branch.
    async fn fetch(&self, remote: &str, branch: &str) -> PortResult<()>;

    /// Resolve the fetched revision, or `None` when the remote branch is missing.
    async fn fetched_sha(&self, remote: &str, branch: &str) -> PortResult<Option<String>>;

    /// Current cumulative base HEAD.
    async fn head_sha(&self) -> PortResult<String>;

    /// `git merge-base --is-ancestor`.
    async fn is_ancestor(&self, ancestor: &str, descendant: &str) -> PortResult<bool>;

    /// `git merge-base`.
    async fn merge_base(&self, a: &str, b: &str) -> PortResult<String>;

    /// Non-fast-forward merge of `sha` with the supplied trailer-bearing message.
    async fn merge_no_ff(&self, sha: &str, message: &str) -> PortResult<MergeCommandResult>;

    /// `MERGE_HEAD` / unmerged-index evidence.
    async fn merge_repository_state(&self) -> PortResult<MergeRepositoryState>;

    /// Whether the worktree and index are clean.
    async fn is_working_tree_clean(&self) -> PortResult<bool>;

    /// `git status --porcelain=v2` output.
    async fn status_porcelain_v2(&self) -> PortResult<String>;

    /// Raw message of a commit.
    async fn commit_message(&self, sha: &str) -> PortResult<String>;

    /// Parent SHAs of a commit, in Git order.
    async fn commit_parents(&self, sha: &str) -> PortResult<Vec<String>>;

    /// First-parent commits from `from_exclusive` (exclusive) to `to` (inclusive),
    /// oldest first, with each commit's own tree evidence attached.
    ///
    /// `from_exclusive` is `None` for the bounded offline recovery scan that runs
    /// before a remote has been selected; `limit` bounds that walk.
    async fn first_parent_commits(
        &self,
        from_exclusive: Option<&str>,
        to: &str,
        limit: Option<usize>,
    ) -> PortResult<Vec<SpineCommit>>;

    /// Resolve a local ref (including remote-tracking refs) without network access.
    async fn local_ref_sha(&self, reference: &str) -> PortResult<Option<String>>;

    /// Non-force `git push --porcelain <remote> HEAD:<branch>`.
    async fn push_porcelain(&self, remote: &str, branch: &str) -> PortResult<PushCommandResult>;

    /// Observe the remote branch through `git ls-remote`.
    async fn ls_remote_sha(&self, remote: &str, branch: &str) -> PortResult<Option<String>>;
}

/// Result of running the operator-supplied complete verification command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationOutcome {
    pub success: bool,
    /// Bounded failure output handed to semantic repair.
    pub output_tail: String,
}

impl VerificationOutcome {
    pub fn passed() -> Self {
        Self {
            success: true,
            output_tail: String::new(),
        }
    }

    pub fn failed(output_tail: impl Into<String>) -> Self {
        Self {
            success: false,
            output_tail: output_tail.into(),
        }
    }
}

/// Executes the complete repository verification command.
#[async_trait]
pub trait UpstreamVerifier: Send + Sync {
    /// Run the configured command from the cumulative base root.
    async fn verify(&self) -> PortResult<VerificationOutcome>;
}

/// Why the bounded repair agent is being invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairCause {
    /// Repository state proves a textual conflict or unfinished merge.
    TextualConflict,
    /// The complete verification command failed after a clean merge.
    SemanticVerification,
    /// A native push failed with repository-repairable local mutation.
    PushRepository,
}

/// Context handed to the bounded repair agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairRequest {
    pub cause: RepairCause,
    pub remote: String,
    pub branch: String,
    /// Cumulative revision before integration started.
    pub local_revision_before: String,
    /// Fetched remote SHA under integration.
    pub fetched_sha: String,
    /// Unmerged paths, when repository state reports any.
    pub conflict_files: Vec<String>,
    /// `git status` output for context.
    pub status: String,
    /// Complete verification command, for semantic repair.
    pub verify_command: String,
    /// Bounded verification failure output, for semantic repair.
    pub verify_output_tail: String,
    /// Sanitized push diagnostics, for push repair.
    pub push_diagnostics: String,
}

/// One bounded repair invocation's raw result.
///
/// Agent narrative output never establishes success; the coordinator revalidates
/// repository state after every attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairAttemptResult {
    pub command_success: bool,
}

/// Runs the existing bounded `resolve_command` agent with an upstream goal.
#[async_trait]
pub trait UpstreamRepairAgent: Send + Sync {
    /// Maximum number of attempts Conflux may make. Owned by Conflux, not the agent.
    fn max_attempts(&self) -> u32;

    /// Invoke one repair attempt.
    async fn repair(&self, request: &RepairRequest) -> PortResult<RepairAttemptResult>;
}

/// Non-authoritative operator-visible upstream lifecycle evidence.
///
/// Observability MUST NOT become routing authority: the coordinator emits these
/// after it has already decided, and never reads them back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamEvent {
    CheckpointStarted {
        remote: String,
        branch: String,
        trigger: String,
    },
    CheckpointDeferred {
        reason: String,
    },
    FetchCompleted {
        remote: String,
        branch: String,
        fetched_sha: String,
        local_sha: String,
    },
    NoOp {
        fetched_sha: String,
    },
    IntegrationStarted {
        fetched_sha: String,
    },
    IntegrationCompleted {
        merge_sha: String,
    },
    Resolving {
        cause: String,
        attempt: u32,
    },
    Reverifying {
        command: String,
    },
    VerificationFailed {
        output_tail: String,
    },
    Pushing {
        remote: String,
        branch: String,
        head: String,
    },
    PushFailed {
        classification: String,
    },
    PushConfirmed {
        remote: String,
        branch: String,
        head: String,
    },
    Stalled {
        reason: String,
    },
    Completed,
}

/// Sink for non-authoritative upstream lifecycle evidence.
#[async_trait]
pub trait UpstreamObserver: Send + Sync {
    async fn observe(&self, event: UpstreamEvent);
}

/// Observer that drops every event. Used where reporting is not wired.
pub struct NoopUpstreamObserver;

#[async_trait]
impl UpstreamObserver for NoopUpstreamObserver {
    async fn observe(&self, _event: UpstreamEvent) {}
}
