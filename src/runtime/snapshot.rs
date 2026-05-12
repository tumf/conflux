use crate::runtime::ids::{OrchestratorId, ProjectId, ProposalId};
use crate::runtime::orchestrator::{OrchestratorLifecycleStatus, OrchestratorRuntimeState};
use crate::runtime::project::{BaseLaneOwner, ProjectRuntimeState, ProjectStatus};
use crate::runtime::proposal::{ProposalRuntimeState, ProposalStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalSnapshot {
    pub id: ProposalId,
    pub status: ProposalStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSnapshot {
    pub id: ProjectId,
    pub status: ProjectStatus,
    pub proposals: Vec<ProposalSnapshot>,
    pub base_lane_owner: Option<BaseLaneOwner>,
    pub compatibility: RuntimeCompatibilityView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestratorSnapshot {
    pub id: OrchestratorId,
    pub status: OrchestratorLifecycleStatus,
    pub projects: Vec<ProjectSnapshot>,
}

/// Derived compatibility terms for legacy change-oriented views.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeCompatibilityView {
    pub queued: Vec<ProposalId>,
    pub stalled: Vec<ProposalId>,
    pub dependency_blocked: Vec<ProposalId>,
    pub merge_wait: Vec<ProposalId>,
    pub resolve_wait: Vec<ProposalId>,
    pub rejected: Vec<ProposalId>,
    pub merged: Vec<ProposalId>,
    pub dispatch_candidates: Vec<ProposalId>,
}

impl From<&ProposalRuntimeState> for ProposalSnapshot {
    fn from(proposal: &ProposalRuntimeState) -> Self {
        Self {
            id: proposal.id.clone(),
            status: proposal.status.clone(),
        }
    }
}

impl From<&ProjectRuntimeState> for RuntimeCompatibilityView {
    fn from(project: &ProjectRuntimeState) -> Self {
        Self {
            queued: project.queued_proposals(),
            stalled: project.stalled_proposals(),
            dependency_blocked: project.dependency_blocked_proposals(),
            merge_wait: project.merge_wait_proposals(),
            resolve_wait: project.resolve_wait_proposals(),
            rejected: project.rejected_proposals(),
            merged: project.merged_proposals(),
            dispatch_candidates: project.dispatch_candidates(),
        }
    }
}

impl From<&ProjectRuntimeState> for ProjectSnapshot {
    fn from(project: &ProjectRuntimeState) -> Self {
        Self {
            id: project.id.clone(),
            status: project.status,
            proposals: project
                .proposals
                .values()
                .map(ProposalSnapshot::from)
                .collect(),
            base_lane_owner: project.base_lane_owner.clone(),
            compatibility: RuntimeCompatibilityView::from(project),
        }
    }
}

impl From<&OrchestratorRuntimeState> for OrchestratorSnapshot {
    fn from(orchestrator: &OrchestratorRuntimeState) -> Self {
        Self {
            id: orchestrator.id.clone(),
            status: orchestrator.status,
            projects: orchestrator
                .projects
                .values()
                .map(ProjectSnapshot::from)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::proposal::{RuntimeRevision, WorkspaceRef};

    #[test]
    fn snapshot_preserves_project_proposal_nesting_and_derived_views() {
        let mut orchestrator = OrchestratorRuntimeState::default();
        let project = orchestrator.ensure_project(ProjectId::from("project-a"));
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

        let snapshot = OrchestratorSnapshot::from(&orchestrator);
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.projects[0].proposals.len(), 2);
        assert_eq!(
            snapshot.projects[0].compatibility.queued,
            vec![ProposalId::from_change_id("change-a")]
        );
        assert_eq!(
            snapshot.projects[0].compatibility.merge_wait,
            vec![ProposalId::from_change_id("change-b")]
        );
        assert!(snapshot.projects[0]
            .compatibility
            .dispatch_candidates
            .is_empty());
    }
}
