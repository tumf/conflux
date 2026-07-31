//! Opt-in cumulative-base upstream integration for `cflx run`.
//!
//! This subsystem is installed **only** when the operator passes `-u` /
//! `--integrate-upstream` to cumulative parallel `cflx run`. When the option is
//! absent no object from this module is constructed, so the existing execution
//! path performs no additional fetch, merge, verification, lifecycle event, or
//! push.
//!
//! The module is split so that every routing decision is a pure function or a
//! port-driven state machine:
//!
//! - [`options`]: invocation-scoped CLI contract and static startup validation.
//! - [`trailers`]: `Cflx-Upstream-*` identity trailer format and validation.
//! - [`classify`]: repository-state classification (merge outcome, ancestry,
//!   `git push --porcelain` per-ref status, `git status --porcelain=v2`).
//! - [`spine`]: first-parent cumulative-history classification.
//! - [`checkpoint`]: deterministic base-lane checkpoint triggering/batching.
//! - [`ports`]: the Git/verification/repair boundaries the coordinator drives.
//! - [`coordinator`]: the Conflux-owned integration and finalization workflow.
//! - [`repair`]: dedicated upstream textual/semantic repair goal and prompt.
//! - [`git_ops`]: real Git implementation of [`ports::UpstreamGit`].
//! - [`verify`]: real verification-command implementation.
//!
//! Unit coverage drives [`coordinator::UpstreamCoordinator`] through in-memory
//! fakes; real Git/worktree/bare-remote behavior is covered by heavy E2E tests.

pub mod checkpoint;
pub mod classify;
pub mod coordinator;
pub mod git_ops;
pub mod options;
pub mod ports;
pub mod publication;
pub mod repair;
pub mod spine;
pub mod startup;
pub mod trailers;
pub mod verify;

pub use checkpoint::{CheckpointDecision, CheckpointScheduler, CheckpointTrigger};
pub use classify::{
    classify_merge_outcome, classify_push_failure, classify_upstream_revision, MergeOutcomeClass,
    PushFailureClass, UpstreamRevisionClass,
};
pub use coordinator::{SchedulerOutcome, UpstreamCoordinator, UpstreamStepOutcome};
pub use options::{
    resolve_frontend_upstream_config, resolve_upstream_config, validate_static_preconditions,
    StaticPreconditions, UpstreamIntegrationConfig, UpstreamOptionError, DEFAULT_UPSTREAM_REMOTE,
};
pub use publication::{
    format_publication_marker_message, parse_publication_trailers, PublicationEvidence,
    PublicationTrailers, TRAILER_PUBLISH_BRANCH, TRAILER_PUBLISH_CHANGE, TRAILER_PUBLISH_REMOTE,
};
pub use spine::{classify_spine_commit, SpineCommitClass};
pub use startup::{
    ensure_no_unpushed_upstream_recovery, prepare_upstream_integration, UpstreamRuntime,
    UpstreamStartupError,
};
pub use trailers::{
    format_upstream_merge_message, parse_upstream_trailers, UpstreamTrailers, TRAILER_BRANCH,
    TRAILER_REMOTE, TRAILER_SHA,
};
