use crate::runtime::ids::{ProjectId, ProposalId};
use crate::runtime::orchestrator::{OrchestratorLifecycleStatus, OrchestratorRuntimeState};
use crate::runtime::project::{ProjectRuntimeState, ProjectStatus};
use crate::runtime::proposal::{
    BlockerInfo, ExternalBlockerInfo, ProposalStatus, RuntimeRevision, WorkspaceRef,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    Orchestrator(OrchestratorEvent),
    Project {
        project_id: ProjectId,
        event: ProjectEvent,
    },
    Proposal {
        project_id: ProjectId,
        proposal_id: ProposalId,
        event: ProposalEvent,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestratorEvent {
    Started,
    Stopping,
    Stopped,
    Error,
    DeriveStatusFromProjects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectEvent {
    Started,
    Stopping,
    Stopped,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalEvent {
    Queue {
        revision: RuntimeRevision,
    },
    DependencyBlocked {
        blocker: BlockerInfo,
        revision: RuntimeRevision,
    },
    /// The orchestrator validated an external prerequisite report and classified
    /// it as an external `blocked` wait.
    ExternalBlocked {
        blocker: ExternalBlockerInfo,
        revision: RuntimeRevision,
    },
    ApplyStarted {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    AcceptanceStarted {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    RejectingStarted {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    Stalled {
        blocker: BlockerInfo,
        revision: RuntimeRevision,
    },
    ArchiveStarted {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    MergeWait {
        workspace: WorkspaceRef,
        revision: RuntimeRevision,
    },
    ResolveStarted {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    Merged {
        revision: RuntimeRevision,
    },
    Rejected {
        reason: String,
        revision: RuntimeRevision,
    },
    Failed {
        error: String,
        revision: RuntimeRevision,
    },
    Stopped {
        reason: String,
        revision: RuntimeRevision,
    },
}

impl OrchestratorRuntimeState {
    pub fn apply_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Orchestrator(event) => self.apply_orchestrator_event(event),
            RuntimeEvent::Project { project_id, event } => {
                self.ensure_project(project_id).apply_project_event(event);
                self.derive_status_from_projects();
            }
            RuntimeEvent::Proposal {
                project_id,
                proposal_id,
                event,
            } => {
                self.ensure_project(project_id)
                    .apply_proposal_event(proposal_id, event);
                self.derive_status_from_projects();
            }
        }
    }

    fn apply_orchestrator_event(&mut self, event: OrchestratorEvent) {
        self.status = match event {
            OrchestratorEvent::Started => OrchestratorLifecycleStatus::Running,
            OrchestratorEvent::Stopping => OrchestratorLifecycleStatus::Stopping,
            OrchestratorEvent::Stopped => OrchestratorLifecycleStatus::Stopped,
            OrchestratorEvent::Error => OrchestratorLifecycleStatus::Error,
            OrchestratorEvent::DeriveStatusFromProjects => {
                self.derive_status_from_projects();
                return;
            }
        };
    }
}

impl ProjectRuntimeState {
    pub fn apply_project_event(&mut self, event: ProjectEvent) {
        self.status = match event {
            ProjectEvent::Started => ProjectStatus::Running,
            ProjectEvent::Stopping => ProjectStatus::Stopping,
            ProjectEvent::Stopped => ProjectStatus::Stopped,
            ProjectEvent::Error => ProjectStatus::Error,
        };
    }

    pub fn apply_proposal_event(&mut self, proposal_id: ProposalId, event: ProposalEvent) {
        let next_status = match event {
            ProposalEvent::Queue { revision } => ProposalStatus::Queued { revision },
            ProposalEvent::DependencyBlocked { blocker, revision } => {
                ProposalStatus::DependencyBlocked { blocker, revision }
            }
            ProposalEvent::ExternalBlocked { blocker, revision } => {
                ProposalStatus::ExternalBlocked { blocker, revision }
            }
            ProposalEvent::ApplyStarted {
                workspace,
                attempt,
                revision,
            } => ProposalStatus::Applying {
                workspace,
                attempt,
                revision,
            },
            ProposalEvent::AcceptanceStarted {
                workspace,
                attempt,
                revision,
            } => ProposalStatus::Accepting {
                workspace,
                attempt,
                revision,
            },
            ProposalEvent::RejectingStarted {
                workspace,
                attempt,
                revision,
            } => ProposalStatus::Rejecting {
                workspace,
                attempt,
                revision,
            },
            ProposalEvent::Stalled { blocker, revision } => {
                ProposalStatus::Stalled { blocker, revision }
            }
            ProposalEvent::ArchiveStarted {
                workspace,
                attempt,
                revision,
            } => ProposalStatus::Archiving {
                workspace,
                attempt,
                revision,
            },
            ProposalEvent::MergeWait {
                workspace,
                revision,
            } => ProposalStatus::MergeWait {
                workspace,
                revision,
            },
            ProposalEvent::ResolveStarted {
                workspace,
                attempt,
                revision,
            } => ProposalStatus::Resolving {
                workspace,
                attempt,
                revision,
            },
            ProposalEvent::Merged { revision } => ProposalStatus::Merged { revision },
            ProposalEvent::Rejected { reason, revision } => {
                ProposalStatus::Rejected { reason, revision }
            }
            ProposalEvent::Failed { error, revision } => ProposalStatus::Failed { error, revision },
            ProposalEvent::Stopped { reason, revision } => {
                ProposalStatus::Stopped { reason, revision }
            }
        };

        if self.should_ignore_transition(&proposal_id, &next_status) {
            return;
        }

        self.set_proposal_status(proposal_id, next_status);
    }

    fn should_ignore_transition(
        &self,
        proposal_id: &ProposalId,
        next_status: &ProposalStatus,
    ) -> bool {
        if let Some(owner) = &self.base_lane_owner {
            let same_owner = &owner.proposal_id == proposal_id;
            if next_status.is_base_lane_status() && !same_owner {
                return true;
            }
        }

        let Some(current_status) = self.proposal_status(proposal_id) else {
            return false;
        };

        if current_status.is_terminal() {
            return true;
        }

        if next_status.revision() < current_status.revision() {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ids::OrchestratorId;

    fn event(project_id: &str, proposal_id: &str, event: ProposalEvent) -> RuntimeEvent {
        RuntimeEvent::Proposal {
            project_id: ProjectId::from(project_id),
            proposal_id: ProposalId::from_change_id(proposal_id),
            event,
        }
    }

    #[test]
    fn reducer_transitions_through_happy_path_to_merged() {
        let mut state = OrchestratorRuntimeState::new(OrchestratorId::from("orch"));
        let workspace = WorkspaceRef::new("/tmp/change-a");

        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::Queue {
                revision: RuntimeRevision(1),
            },
        ));
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::ApplyStarted {
                workspace: workspace.clone(),
                attempt: 1,
                revision: RuntimeRevision(2),
            },
        ));
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::AcceptanceStarted {
                workspace: workspace.clone(),
                attempt: 1,
                revision: RuntimeRevision(3),
            },
        ));
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::ArchiveStarted {
                workspace: workspace.clone(),
                attempt: 1,
                revision: RuntimeRevision(4),
            },
        ));
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::MergeWait {
                workspace: workspace.clone(),
                revision: RuntimeRevision(5),
            },
        ));
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::ResolveStarted {
                workspace,
                attempt: 1,
                revision: RuntimeRevision(6),
            },
        ));
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::Merged {
                revision: RuntimeRevision(7),
            },
        ));

        let project = state.projects.get(&ProjectId::from("project")).unwrap();
        assert!(matches!(
            project.proposal_status(&ProposalId::from_change_id("change-a")),
            Some(ProposalStatus::Merged {
                revision: RuntimeRevision(7)
            })
        ));
        assert!(project.base_lane_owner.is_none());
    }

    #[test]
    fn reducer_covers_stalled_rejected_failed_and_stopped_states() {
        let mut state = OrchestratorRuntimeState::default();
        state.apply_event(event(
            "project",
            "stalled",
            ProposalEvent::Stalled {
                blocker: BlockerInfo::new("external", "needs operator"),
                revision: RuntimeRevision(1),
            },
        ));
        state.apply_event(event(
            "project",
            "rejected",
            ProposalEvent::Rejected {
                reason: "acceptance failed".to_string(),
                revision: RuntimeRevision(1),
            },
        ));
        state.apply_event(event(
            "project",
            "failed",
            ProposalEvent::Failed {
                error: "tool failed".to_string(),
                revision: RuntimeRevision(1),
            },
        ));
        state.apply_event(event(
            "project",
            "stopped",
            ProposalEvent::Stopped {
                reason: "operator".to_string(),
                revision: RuntimeRevision(1),
            },
        ));

        let project = state.projects.get(&ProjectId::from("project")).unwrap();
        assert_eq!(
            project.stalled_proposals(),
            vec![ProposalId::from_change_id("stalled")]
        );
        assert!(matches!(
            project.proposal_status(&ProposalId::from_change_id("rejected")),
            Some(ProposalStatus::Rejected { .. })
        ));
        assert!(matches!(
            project.proposal_status(&ProposalId::from_change_id("failed")),
            Some(ProposalStatus::Failed { .. })
        ));
        assert!(matches!(
            project.proposal_status(&ProposalId::from_change_id("stopped")),
            Some(ProposalStatus::Stopped { .. })
        ));
    }

    fn external_blocker() -> ExternalBlockerInfo {
        use crate::runtime::proposal::BlockerOrigin;
        ExternalBlockerInfo {
            origin: BlockerOrigin::Acceptance,
            category: "external_service".to_string(),
            evidence: vec!["staging deploy queue is offline".to_string()],
            prerequisite_owner: Some("platform".to_string()),
            unblock_condition: "the staging deploy queue accepts jobs again".to_string(),
            next_action: "retry acceptance once the queue is online".to_string(),
            resumable: true,
        }
    }

    /// Applying an external-blocked event replaces any stalled state and vice
    /// versa: one canonical status field means the two can never coexist.
    #[test]
    fn external_blocked_and_stalled_events_replace_rather_than_accumulate() {
        use crate::runtime::proposal::BlockerKind;
        let mut state = OrchestratorRuntimeState::default();

        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::ExternalBlocked {
                blocker: external_blocker(),
                revision: RuntimeRevision(1),
            },
        ));
        let project = state.projects.get(&ProjectId::from("project")).unwrap();
        let status = project
            .proposal_status(&ProposalId::from_change_id("change-a"))
            .unwrap();
        assert_eq!(status.display_status(), "blocked");
        assert_eq!(status.blocker_kind(), Some(BlockerKind::External));
        assert_eq!(
            project.external_blocked_proposals(),
            vec![ProposalId::from_change_id("change-a")]
        );
        assert!(project.stalled_proposals().is_empty());

        // A later stalled observation supersedes it rather than coexisting.
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::Stalled {
                blocker: BlockerInfo::new("repeated_acceptance_findings", "no semantic progress"),
                revision: RuntimeRevision(2),
            },
        ));
        let project = state.projects.get(&ProjectId::from("project")).unwrap();
        let status = project
            .proposal_status(&ProposalId::from_change_id("change-a"))
            .unwrap();
        assert_eq!(status.display_status(), "stalled");
        assert_eq!(status.blocker_kind(), None);
        assert!(project.external_blocked_proposals().is_empty());
    }

    /// An external-blocked proposal is never a dependency edge: its dependent
    /// keeps its own dependency blocker kind and unrelated work stays queued.
    #[test]
    fn external_blocked_proposal_does_not_become_a_dependency_edge() {
        use crate::runtime::proposal::BlockerKind;
        let mut state = OrchestratorRuntimeState::default();
        state.apply_event(event(
            "project",
            "alpha",
            ProposalEvent::ExternalBlocked {
                blocker: external_blocker(),
                revision: RuntimeRevision(1),
            },
        ));
        state.apply_event(event(
            "project",
            "beta",
            ProposalEvent::DependencyBlocked {
                blocker: BlockerInfo::new("dependency", "waiting for alpha"),
                revision: RuntimeRevision(1),
            },
        ));
        state.apply_event(event(
            "project",
            "gamma",
            ProposalEvent::Queue {
                revision: RuntimeRevision(1),
            },
        ));

        let project = state.projects.get(&ProjectId::from("project")).unwrap();
        assert_eq!(
            project
                .proposal_status(&ProposalId::from_change_id("beta"))
                .unwrap()
                .blocker_kind(),
            Some(BlockerKind::Dependency)
        );
        assert_eq!(
            project.dependency_blocked_proposals(),
            vec![ProposalId::from_change_id("beta")]
        );
        assert_eq!(
            project.external_blocked_proposals(),
            vec![ProposalId::from_change_id("alpha")]
        );
        // Ordinary dispatch suppresses both waits but not the unrelated change.
        assert_eq!(
            project.dispatch_candidates(),
            vec![ProposalId::from_change_id("gamma")]
        );
    }

    #[test]
    fn stale_non_terminal_events_do_not_regress_newer_state() {
        let mut state = OrchestratorRuntimeState::default();
        let workspace = WorkspaceRef::new("/tmp/change-a");
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::AcceptanceStarted {
                workspace: workspace.clone(),
                attempt: 1,
                revision: RuntimeRevision(4),
            },
        ));
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::ApplyStarted {
                workspace,
                attempt: 1,
                revision: RuntimeRevision(3),
            },
        ));

        let project = state.projects.get(&ProjectId::from("project")).unwrap();
        assert!(matches!(
            project.proposal_status(&ProposalId::from_change_id("change-a")),
            Some(ProposalStatus::Accepting {
                revision: RuntimeRevision(4),
                ..
            })
        ));
    }

    #[test]
    fn base_lane_events_are_deferred_when_lane_is_owned_by_another_proposal() {
        let mut state = OrchestratorRuntimeState::default();
        state.apply_event(event(
            "project",
            "change-a",
            ProposalEvent::MergeWait {
                workspace: WorkspaceRef::new("/tmp/change-a"),
                revision: RuntimeRevision(1),
            },
        ));
        state.apply_event(event(
            "project",
            "change-b",
            ProposalEvent::ResolveStarted {
                workspace: WorkspaceRef::new("/tmp/change-b"),
                attempt: 1,
                revision: RuntimeRevision(2),
            },
        ));

        let project = state.projects.get(&ProjectId::from("project")).unwrap();
        assert!(project
            .proposal_status(&ProposalId::from_change_id("change-b"))
            .is_none());
        assert_eq!(
            project
                .base_lane_owner
                .as_ref()
                .unwrap()
                .proposal_id
                .as_change_id(),
            "change-a"
        );
    }
}

#[cfg(test)]
pub mod terminal {
    use super::*;

    fn project_with_merged_proposal() -> ProjectRuntimeState {
        let mut project = ProjectRuntimeState::new("project");
        project.apply_proposal_event(
            ProposalId::from_change_id("change-a"),
            ProposalEvent::Merged {
                revision: RuntimeRevision(10),
            },
        );
        project
    }

    #[test]
    fn stale_apply_archive_resolve_merge_events_cannot_regress_merged_proposal() {
        let mut project = project_with_merged_proposal();
        let workspace = WorkspaceRef::new("/tmp/change-a");

        for event in [
            ProposalEvent::ApplyStarted {
                workspace: workspace.clone(),
                attempt: 2,
                revision: RuntimeRevision(11),
            },
            ProposalEvent::ArchiveStarted {
                workspace: workspace.clone(),
                attempt: 2,
                revision: RuntimeRevision(12),
            },
            ProposalEvent::ResolveStarted {
                workspace,
                attempt: 2,
                revision: RuntimeRevision(13),
            },
            ProposalEvent::Merged {
                revision: RuntimeRevision(14),
            },
        ] {
            project.apply_proposal_event(ProposalId::from_change_id("change-a"), event);
        }

        assert!(matches!(
            project.proposal_status(&ProposalId::from_change_id("change-a")),
            Some(ProposalStatus::Merged {
                revision: RuntimeRevision(10)
            })
        ));
    }

    #[test]
    fn stale_events_cannot_regress_rejected_proposal() {
        let mut project = ProjectRuntimeState::new("project");
        project.apply_proposal_event(
            ProposalId::from_change_id("change-a"),
            ProposalEvent::Rejected {
                reason: "not acceptable".to_string(),
                revision: RuntimeRevision(5),
            },
        );
        project.apply_proposal_event(
            ProposalId::from_change_id("change-a"),
            ProposalEvent::ApplyStarted {
                workspace: WorkspaceRef::new("/tmp/change-a"),
                attempt: 2,
                revision: RuntimeRevision(6),
            },
        );

        assert!(matches!(
            project.proposal_status(&ProposalId::from_change_id("change-a")),
            Some(ProposalStatus::Rejected {
                revision: RuntimeRevision(5),
                ..
            })
        ));
    }
}
