use std::collections::BTreeMap;

use crate::runtime::ids::{OrchestratorId, ProjectId};
use crate::runtime::project::{ProjectRuntimeState, ProjectStatus};

/// Global orchestrator lifecycle. Proposal details live inside project state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrchestratorLifecycleStatus {
    #[default]
    Idle,
    Running,
    Stopping,
    Stopped,
    Error,
}

/// Top-level runtime state that owns project aggregation only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestratorRuntimeState {
    pub id: OrchestratorId,
    pub status: OrchestratorLifecycleStatus,
    pub projects: BTreeMap<ProjectId, ProjectRuntimeState>,
}

impl OrchestratorRuntimeState {
    pub fn new(id: impl Into<OrchestratorId>) -> Self {
        Self {
            id: id.into(),
            status: OrchestratorLifecycleStatus::Idle,
            projects: BTreeMap::new(),
        }
    }

    pub fn ensure_project(&mut self, project_id: ProjectId) -> &mut ProjectRuntimeState {
        self.projects
            .entry(project_id.clone())
            .or_insert_with(|| ProjectRuntimeState::new(project_id))
    }

    pub fn derive_status_from_projects(&mut self) {
        self.status = if self
            .projects
            .values()
            .any(|project| project.status == ProjectStatus::Error)
        {
            OrchestratorLifecycleStatus::Error
        } else if self
            .projects
            .values()
            .any(|project| project.status == ProjectStatus::Stopping)
        {
            OrchestratorLifecycleStatus::Stopping
        } else if self
            .projects
            .values()
            .any(|project| project.status == ProjectStatus::Running)
        {
            OrchestratorLifecycleStatus::Running
        } else if self
            .projects
            .values()
            .all(|project| project.status == ProjectStatus::Stopped)
            && !self.projects.is_empty()
        {
            OrchestratorLifecycleStatus::Stopped
        } else {
            OrchestratorLifecycleStatus::Idle
        };
    }
}

impl Default for OrchestratorRuntimeState {
    fn default() -> Self {
        Self::new(OrchestratorId::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestrator_snapshot_status_is_derived_from_project_events() {
        let mut orchestrator = OrchestratorRuntimeState::default();
        orchestrator
            .ensure_project(ProjectId::from("project-a"))
            .status = ProjectStatus::Running;
        orchestrator.derive_status_from_projects();
        assert_eq!(orchestrator.status, OrchestratorLifecycleStatus::Running);

        orchestrator
            .ensure_project(ProjectId::from("project-a"))
            .status = ProjectStatus::Error;
        orchestrator.derive_status_from_projects();
        assert_eq!(orchestrator.status, OrchestratorLifecycleStatus::Error);
    }
}
