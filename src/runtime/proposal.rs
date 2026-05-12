use crate::runtime::ids::ProposalId;

/// Monotonic reducer revision used to ignore stale runtime observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct RuntimeRevision(pub u64);

/// Worktree or workspace reference carried only while a proposal is being processed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceRef {
    pub path: String,
    pub branch: Option<String>,
}

impl WorkspaceRef {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            branch: None,
        }
    }

    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }
}

/// Operator-facing blocker details for stalled or dependency-blocked proposals.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockerInfo {
    pub category: String,
    pub summary: String,
    pub retry_count: u32,
}

impl BlockerInfo {
    pub fn new(category: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            summary: summary.into(),
            retry_count: 0,
        }
    }

    pub fn with_retry_count(mut self, retry_count: u32) -> Self {
        self.retry_count = retry_count;
        self
    }
}

/// Single canonical lifecycle status for one proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalStatus {
    NotQueued,
    Queued { revision: RuntimeRevision },
    DependencyBlocked { blocker: BlockerInfo, revision: RuntimeRevision },
    Applying { workspace: WorkspaceRef, attempt: u32, revision: RuntimeRevision },
    Accepting { workspace: WorkspaceRef, attempt: u32, revision: RuntimeRevision },
    Rejecting { workspace: WorkspaceRef, attempt: u32, revision: RuntimeRevision },
    Stalled { blocker: BlockerInfo, revision: RuntimeRevision },
    Archiving { workspace: WorkspaceRef, attempt: u32, revision: RuntimeRevision },
    MergeWait { workspace: WorkspaceRef, revision: RuntimeRevision },
    Resolving { workspace: WorkspaceRef, attempt: u32, revision: RuntimeRevision },
    Merged { revision: RuntimeRevision },
    Rejected { reason: String, revision: RuntimeRevision },
    Failed { error: String, revision: RuntimeRevision },
    Stopped { reason: String, revision: RuntimeRevision },
}

impl Default for ProposalStatus {
    fn default() -> Self {
        Self::NotQueued
    }
}

impl ProposalStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotQueued => "not_queued",
            Self::Queued { .. } => "queued",
            Self::DependencyBlocked { .. } => "dependency_blocked",
            Self::Applying { .. } => "applying",
            Self::Accepting { .. } => "accepting",
            Self::Rejecting { .. } => "rejecting",
            Self::Stalled { .. } => "stalled",
            Self::Archiving { .. } => "archiving",
            Self::MergeWait { .. } => "merge_wait",
            Self::Resolving { .. } => "resolving",
            Self::Merged { .. } => "merged",
            Self::Rejected { .. } => "rejected",
            Self::Failed { .. } => "failed",
            Self::Stopped { .. } => "stopped",
        }
    }

    pub fn revision(&self) -> RuntimeRevision {
        match self {
            Self::NotQueued => RuntimeRevision::default(),
            Self::Queued { revision }
            | Self::DependencyBlocked { revision, .. }
            | Self::Applying { revision, .. }
            | Self::Accepting { revision, .. }
            | Self::Rejecting { revision, .. }
            | Self::Stalled { revision, .. }
            | Self::Archiving { revision, .. }
            | Self::MergeWait { revision, .. }
            | Self::Resolving { revision, .. }
            | Self::Merged { revision }
            | Self::Rejected { revision, .. }
            | Self::Failed { revision, .. }
            | Self::Stopped { revision, .. } => *revision,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Merged { .. } | Self::Rejected { .. } | Self::Failed { .. } | Self::Stopped { .. })
    }

    pub fn is_base_lane_status(&self) -> bool {
        matches!(self, Self::Rejecting { .. } | Self::Archiving { .. } | Self::MergeWait { .. } | Self::Resolving { .. })
    }
}

/// Runtime state for a single OpenSpec proposal/change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalRuntimeState {
    pub id: ProposalId,
    pub status: ProposalStatus,
}

impl ProposalRuntimeState {
    pub fn new(id: impl Into<ProposalId>) -> Self {
        Self {
            id: id.into(),
            status: ProposalStatus::NotQueued,
        }
    }

    pub fn with_status(id: impl Into<ProposalId>, status: ProposalStatus) -> Self {
        Self { id: id.into(), status }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_status_labels_cover_all_lifecycle_states() {
        let workspace = WorkspaceRef::new("/tmp/worktree").with_branch("feature/x");
        let blocker = BlockerInfo::new("dependency", "waiting for base");
        let statuses = vec![
            ProposalStatus::NotQueued,
            ProposalStatus::Queued { revision: RuntimeRevision(1) },
            ProposalStatus::DependencyBlocked { blocker: blocker.clone(), revision: RuntimeRevision(2) },
            ProposalStatus::Applying { workspace: workspace.clone(), attempt: 1, revision: RuntimeRevision(3) },
            ProposalStatus::Accepting { workspace: workspace.clone(), attempt: 1, revision: RuntimeRevision(4) },
            ProposalStatus::Rejecting { workspace: workspace.clone(), attempt: 1, revision: RuntimeRevision(5) },
            ProposalStatus::Stalled { blocker: blocker.clone(), revision: RuntimeRevision(6) },
            ProposalStatus::Archiving { workspace: workspace.clone(), attempt: 1, revision: RuntimeRevision(7) },
            ProposalStatus::MergeWait { workspace: workspace.clone(), revision: RuntimeRevision(8) },
            ProposalStatus::Resolving { workspace, attempt: 1, revision: RuntimeRevision(9) },
            ProposalStatus::Merged { revision: RuntimeRevision(10) },
            ProposalStatus::Rejected { reason: "not acceptable".to_string(), revision: RuntimeRevision(11) },
            ProposalStatus::Failed { error: "boom".to_string(), revision: RuntimeRevision(12) },
            ProposalStatus::Stopped { reason: "operator".to_string(), revision: RuntimeRevision(13) },
        ];

        let labels: Vec<_> = statuses.iter().map(ProposalStatus::label).collect();
        assert_eq!(labels, vec![
            "not_queued",
            "queued",
            "dependency_blocked",
            "applying",
            "accepting",
            "rejecting",
            "stalled",
            "archiving",
            "merge_wait",
            "resolving",
            "merged",
            "rejected",
            "failed",
            "stopped",
        ]);
        assert!(statuses[10].is_terminal());
        assert!(!statuses[3].is_terminal());
    }

    #[test]
    fn proposal_runtime_state_has_one_canonical_status_field() {
        let proposal = ProposalRuntimeState::with_status(
            "change-a",
            ProposalStatus::Queued { revision: RuntimeRevision(1) },
        );

        assert_eq!(proposal.id.as_change_id(), "change-a");
        assert_eq!(proposal.status.label(), "queued");
    }
}
