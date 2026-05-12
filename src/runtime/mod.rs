//! In-memory three-level runtime state model for Conflux orchestration.
//!
//! This module models runtime state as `Orchestrator > Project > Proposal` for
//! active orchestration observability and reducer-local execution bookkeeping.
//! It is intentionally additive and is not wired into production dispatch yet.
//!
//! ## Constitution compliance
//!
//! The runtime state in this module is in-memory orchestration and observability
//! state only. It MUST NOT become durable workflow-control input for resume,
//! acceptance, archive, merge, resolve, rejection review, or next-action routing.
//! Authoritative workflow-control decisions must remain derivable from workspace
//! file state, workspace git state, and base-branch tree comparison. Deleting
//! `~/.local/state/cflx/**` must not change the next action chosen for the same
//! workspace contents.

pub mod ids;
pub mod orchestrator;
pub mod project;
pub mod proposal;
pub mod reducer;
pub mod snapshot;

pub use ids::{OrchestratorId, ProjectId, ProposalId};
pub use orchestrator::{OrchestratorLifecycleStatus, OrchestratorRuntimeState};
pub use project::{BaseLaneOwner, BaseLaneOwnerKind, ProjectRuntimeState, ProjectStatus};
pub use proposal::{
    BlockerInfo, ProposalRuntimeState, ProposalStatus, RuntimeRevision, WorkspaceRef,
};
pub use reducer::{OrchestratorEvent, ProjectEvent, ProposalEvent, RuntimeEvent};
pub use snapshot::{
    OrchestratorSnapshot, ProjectSnapshot, ProposalSnapshot, RuntimeCompatibilityView,
};
