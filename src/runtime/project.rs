use std::collections::BTreeMap;

use crate::runtime::ids::{ProjectId, ProposalId};
use crate::runtime::proposal::{ProposalRuntimeState, ProposalStatus};

/// Project-level lifecycle independent of individual proposal lifecycle details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectStatus {
    #[default]
    Idle,
    Running,
    Stopping,
    Stopped,
    Error,
}

/// Project base-lane work that must be serialized per project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseLaneOwnerKind {
    Merge,
    Resolve,
    Rejecting,
}

/// Single project-level base-lane owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseLaneOwner {
    pub proposal_id: ProposalId,
    pub kind: BaseLaneOwnerKind,
}

/// Runtime state for one project and its proposal collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeState {
    pub id: ProjectId,
    pub status: ProjectStatus,
    pub proposals: BTreeMap<ProposalId, ProposalRuntimeState>,
    pub base_lane_owner: Option<BaseLaneOwner>,
}

impl ProjectRuntimeState {
    pub fn new(id: impl Into<ProjectId>) -> Self {
        Self {
            id: id.into(),
            status: ProjectStatus::Idle,
            proposals: BTreeMap::new(),
            base_lane_owner: None,
        }
    }

    pub fn ensure_proposal(&mut self, proposal_id: ProposalId) -> &mut ProposalRuntimeState {
        self.proposals
            .entry(proposal_id.clone())
            .or_insert_with(|| ProposalRuntimeState::new(proposal_id))
    }

    pub fn set_proposal_status(&mut self, proposal_id: ProposalId, status: ProposalStatus) {
        self.ensure_proposal(proposal_id.clone()).status = status;
        self.sync_base_lane_owner();
    }

    pub fn proposal_status(&self, proposal_id: &ProposalId) -> Option<&ProposalStatus> {
        self.proposals
            .get(proposal_id)
            .map(|proposal| &proposal.status)
    }

    pub fn queued_proposals(&self) -> Vec<ProposalId> {
        self.proposals_with(|status| matches!(status, ProposalStatus::Queued { .. }))
    }

    pub fn stalled_proposals(&self) -> Vec<ProposalId> {
        self.proposals_with(|status| matches!(status, ProposalStatus::Stalled { .. }))
    }

    pub fn dependency_blocked_proposals(&self) -> Vec<ProposalId> {
        self.proposals_with(|status| matches!(status, ProposalStatus::DependencyBlocked { .. }))
    }

    pub fn merge_wait_proposals(&self) -> Vec<ProposalId> {
        self.proposals_with(|status| matches!(status, ProposalStatus::MergeWait { .. }))
    }

    pub fn resolve_wait_proposals(&self) -> Vec<ProposalId> {
        self.proposals_with(|status| matches!(status, ProposalStatus::Resolving { .. }))
    }

    pub fn rejected_proposals(&self) -> Vec<ProposalId> {
        self.proposals_with(|status| matches!(status, ProposalStatus::Rejected { .. }))
    }

    pub fn merged_proposals(&self) -> Vec<ProposalId> {
        self.proposals_with(|status| matches!(status, ProposalStatus::Merged { .. }))
    }

    pub fn dispatch_candidates(&self) -> Vec<ProposalId> {
        if self.base_lane_owner.is_some() {
            return Vec::new();
        }

        self.queued_proposals()
    }

    pub fn try_claim_base_lane(
        &mut self,
        proposal_id: ProposalId,
        kind: BaseLaneOwnerKind,
    ) -> Result<(), BaseLaneOwner> {
        if let Some(owner) = &self.base_lane_owner {
            return Err(owner.clone());
        }

        self.base_lane_owner = Some(BaseLaneOwner { proposal_id, kind });
        Ok(())
    }

    pub fn release_base_lane_for(&mut self, proposal_id: &ProposalId) {
        if self
            .base_lane_owner
            .as_ref()
            .is_some_and(|owner| &owner.proposal_id == proposal_id)
        {
            self.base_lane_owner = None;
        }
    }

    pub fn sync_base_lane_owner(&mut self) {
        self.base_lane_owner = self.proposals.iter().find_map(|(proposal_id, proposal)| {
            base_lane_kind_for_status(&proposal.status).map(|kind| BaseLaneOwner {
                proposal_id: proposal_id.clone(),
                kind,
            })
        });
    }

    fn proposals_with(&self, predicate: impl Fn(&ProposalStatus) -> bool) -> Vec<ProposalId> {
        self.proposals
            .iter()
            .filter_map(|(id, proposal)| predicate(&proposal.status).then_some(id.clone()))
            .collect()
    }
}

impl Default for ProjectRuntimeState {
    fn default() -> Self {
        Self::new(ProjectId::default())
    }
}

fn base_lane_kind_for_status(status: &ProposalStatus) -> Option<BaseLaneOwnerKind> {
    match status {
        ProposalStatus::Rejecting { .. } => Some(BaseLaneOwnerKind::Rejecting),
        ProposalStatus::Archiving { .. } | ProposalStatus::MergeWait { .. } => {
            Some(BaseLaneOwnerKind::Merge)
        }
        ProposalStatus::Resolving { .. } => Some(BaseLaneOwnerKind::Resolve),
        _ => None,
    }
}

#[cfg(test)]
pub mod dispatch_view {
    use super::*;
    use crate::runtime::proposal::{RuntimeRevision, WorkspaceRef};

    #[test]
    fn dispatch_candidates_are_derived_from_queued_status() {
        let mut project = ProjectRuntimeState::new("project-a");
        project.set_proposal_status(
            ProposalId::from_change_id("change-a"),
            ProposalStatus::Queued {
                revision: RuntimeRevision(1),
            },
        );
        project.set_proposal_status(
            ProposalId::from_change_id("change-b"),
            ProposalStatus::Stalled {
                blocker: crate::runtime::proposal::BlockerInfo::new("external", "waiting"),
                revision: RuntimeRevision(2),
            },
        );

        assert_eq!(
            project.dispatch_candidates(),
            vec![ProposalId::from_change_id("change-a")]
        );
        assert_eq!(
            project.stalled_proposals(),
            vec![ProposalId::from_change_id("change-b")]
        );
    }

    #[test]
    fn base_lane_owner_blocks_project_dispatch_candidates() {
        let mut project = ProjectRuntimeState::new("project-a");
        project.set_proposal_status(
            ProposalId::from_change_id("change-a"),
            ProposalStatus::Queued {
                revision: RuntimeRevision(1),
            },
        );
        project.set_proposal_status(
            ProposalId::from_change_id("change-b"),
            ProposalStatus::MergeWait {
                workspace: WorkspaceRef::new("/tmp/change-b"),
                revision: RuntimeRevision(2),
            },
        );

        assert_eq!(
            project
                .base_lane_owner
                .as_ref()
                .unwrap()
                .proposal_id
                .as_change_id(),
            "change-b"
        );
        assert!(project.dispatch_candidates().is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_simultaneous_base_lane_ownership_within_project() {
        let mut project = ProjectRuntimeState::new("project-a");
        project
            .try_claim_base_lane(
                ProposalId::from_change_id("change-a"),
                BaseLaneOwnerKind::Merge,
            )
            .unwrap();

        let existing = project
            .try_claim_base_lane(
                ProposalId::from_change_id("change-b"),
                BaseLaneOwnerKind::Resolve,
            )
            .unwrap_err();

        assert_eq!(existing.proposal_id.as_change_id(), "change-a");
        assert_eq!(existing.kind, BaseLaneOwnerKind::Merge);
    }
}
