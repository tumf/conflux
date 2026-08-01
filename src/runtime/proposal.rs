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

/// Why a proposal is `blocked`.
///
/// Lifecycle status and blocker kind are deliberately independent concepts: a
/// dependency wait and a validated external prerequisite wait share the
/// `blocked` lifecycle but never lose their distinct explanation. `Stalled` has
/// no blocker kind at all — it is an execution outcome, not a wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockerKind {
    /// Waiting on an unarchived proposal dependency, derived from the proposal
    /// graph.
    Dependency,
    /// Waiting on a validated non-repository prerequisite.
    External,
}

impl BlockerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dependency => "dependency",
            Self::External => "external",
        }
    }
}

/// Phase that observed and reported an external prerequisite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockerOrigin {
    Apply,
    Acceptance,
}

impl BlockerOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Acceptance => "acceptance",
        }
    }
}

/// Validated operator-facing detail for an external prerequisite wait.
///
/// Every field is authored by the reporting agent and validated by the
/// orchestrator before this value exists; nothing here is inferred from prose or
/// from a compatibility verdict token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalBlockerInfo {
    /// Phase that observed the prerequisite.
    pub origin: BlockerOrigin,
    /// Supported blocker category.
    pub category: String,
    /// Concrete non-empty evidence.
    pub evidence: Vec<String>,
    /// Owning team/role or named prerequisite, when supplied.
    pub prerequisite_owner: Option<String>,
    /// Verifiable condition that clears the wait.
    pub unblock_condition: String,
    /// Operator-facing action that can satisfy the condition.
    pub next_action: String,
    /// Whether execution can resume once the prerequisite is satisfied.
    pub resumable: bool,
}

impl ExternalBlockerInfo {
    /// One-line operator summary preserving category, condition, and action.
    pub fn summary(&self) -> String {
        format!(
            "external blocker ({}) reported by {}: {}; unblock when {}; next action {}",
            self.category,
            self.origin.as_str(),
            self.evidence.join(" | "),
            self.unblock_condition,
            self.next_action
        )
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProposalStatus {
    #[default]
    NotQueued,
    Queued {
        revision: RuntimeRevision,
    },
    DependencyBlocked {
        blocker: BlockerInfo,
        revision: RuntimeRevision,
    },
    /// Waiting on a validated non-repository prerequisite.
    ///
    /// A separate variant (rather than a flag on `Stalled` or
    /// `DependencyBlocked`) is what makes simultaneous or ambiguous
    /// blocked/stalled state unrepresentable.
    ExternalBlocked {
        blocker: ExternalBlockerInfo,
        revision: RuntimeRevision,
    },
    Applying {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    Accepting {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    Rejecting {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    Stalled {
        blocker: BlockerInfo,
        revision: RuntimeRevision,
    },
    Archiving {
        workspace: WorkspaceRef,
        attempt: u32,
        revision: RuntimeRevision,
    },
    MergeWait {
        workspace: WorkspaceRef,
        revision: RuntimeRevision,
    },
    Resolving {
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

impl ProposalStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotQueued => "not_queued",
            Self::Queued { .. } => "queued",
            Self::DependencyBlocked { .. } => "dependency_blocked",
            Self::ExternalBlocked { .. } => "external_blocked",
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
            | Self::ExternalBlocked { revision, .. }
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

    /// Operator-facing lifecycle status shared by TUI, WebSocket/API, and the
    /// dashboard.
    ///
    /// Dependency waits and external prerequisite waits collapse to the same
    /// `blocked` word here on purpose; [`Self::blocker_kind`] is what keeps them
    /// distinguishable.
    pub fn display_status(&self) -> &'static str {
        match self {
            Self::NotQueued => "not queued",
            Self::Queued { .. } => "queued",
            Self::DependencyBlocked { .. } | Self::ExternalBlocked { .. } => "blocked",
            Self::Applying { .. } => "applying",
            Self::Accepting { .. } => "accepting",
            Self::Rejecting { .. } => "rejecting",
            Self::Stalled { .. } => "stalled",
            Self::Archiving { .. } => "archiving",
            Self::MergeWait { .. } => "merge wait",
            Self::Resolving { .. } => "resolving",
            Self::Merged { .. } => "merged",
            Self::Rejected { .. } => "rejected",
            Self::Failed { .. } => "error",
            Self::Stopped { .. } => "stopped",
        }
    }

    /// Machine-readable blocker kind for a `blocked` proposal.
    ///
    /// `None` for every other status, including `Stalled`: an execution hold is
    /// never a wait on a named prerequisite.
    pub fn blocker_kind(&self) -> Option<BlockerKind> {
        match self {
            Self::DependencyBlocked { .. } => Some(BlockerKind::Dependency),
            Self::ExternalBlocked { .. } => Some(BlockerKind::External),
            _ => None,
        }
    }

    /// Validated external blocker detail, when this proposal waits on one.
    pub fn external_blocker(&self) -> Option<&ExternalBlockerInfo> {
        match self {
            Self::ExternalBlocked { blocker, .. } => Some(blocker),
            _ => None,
        }
    }

    /// Whether ordinary dispatch must skip this proposal because it is holding
    /// on a wait or an execution stop rather than making progress.
    pub fn suppresses_ordinary_dispatch(&self) -> bool {
        matches!(
            self,
            Self::DependencyBlocked { .. } | Self::ExternalBlocked { .. } | Self::Stalled { .. }
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Merged { .. }
                | Self::Rejected { .. }
                | Self::Failed { .. }
                | Self::Stopped { .. }
        )
    }

    pub fn is_base_lane_status(&self) -> bool {
        matches!(
            self,
            Self::Rejecting { .. }
                | Self::Archiving { .. }
                | Self::MergeWait { .. }
                | Self::Resolving { .. }
        )
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
        Self {
            id: id.into(),
            status,
        }
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
            ProposalStatus::Queued {
                revision: RuntimeRevision(1),
            },
            ProposalStatus::DependencyBlocked {
                blocker: blocker.clone(),
                revision: RuntimeRevision(2),
            },
            ProposalStatus::ExternalBlocked {
                blocker: external_blocker(),
                revision: RuntimeRevision(2),
            },
            ProposalStatus::Applying {
                workspace: workspace.clone(),
                attempt: 1,
                revision: RuntimeRevision(3),
            },
            ProposalStatus::Accepting {
                workspace: workspace.clone(),
                attempt: 1,
                revision: RuntimeRevision(4),
            },
            ProposalStatus::Rejecting {
                workspace: workspace.clone(),
                attempt: 1,
                revision: RuntimeRevision(5),
            },
            ProposalStatus::Stalled {
                blocker: blocker.clone(),
                revision: RuntimeRevision(6),
            },
            ProposalStatus::Archiving {
                workspace: workspace.clone(),
                attempt: 1,
                revision: RuntimeRevision(7),
            },
            ProposalStatus::MergeWait {
                workspace: workspace.clone(),
                revision: RuntimeRevision(8),
            },
            ProposalStatus::Resolving {
                workspace,
                attempt: 1,
                revision: RuntimeRevision(9),
            },
            ProposalStatus::Merged {
                revision: RuntimeRevision(10),
            },
            ProposalStatus::Rejected {
                reason: "not acceptable".to_string(),
                revision: RuntimeRevision(11),
            },
            ProposalStatus::Failed {
                error: "boom".to_string(),
                revision: RuntimeRevision(12),
            },
            ProposalStatus::Stopped {
                reason: "operator".to_string(),
                revision: RuntimeRevision(13),
            },
        ];

        let labels: Vec<_> = statuses.iter().map(ProposalStatus::label).collect();
        assert_eq!(
            labels,
            vec![
                "not_queued",
                "queued",
                "dependency_blocked",
                "external_blocked",
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
            ]
        );
        assert!(statuses
            .iter()
            .find(|status| status.label() == "merged")
            .unwrap()
            .is_terminal());
        assert!(!statuses
            .iter()
            .find(|status| status.label() == "applying")
            .unwrap()
            .is_terminal());
    }

    fn external_blocker() -> ExternalBlockerInfo {
        ExternalBlockerInfo {
            origin: BlockerOrigin::Acceptance,
            category: "credential".to_string(),
            evidence: vec!["STAGING_API_KEY is unset".to_string()],
            prerequisite_owner: Some("platform".to_string()),
            unblock_condition: "STAGING_API_KEY is present in the verification environment"
                .to_string(),
            next_action: "provision STAGING_API_KEY then retry acceptance".to_string(),
            resumable: true,
        }
    }

    /// `blocked` and `stalled` are distinct enum variants, so a proposal can
    /// never be simultaneously or ambiguously both, and a `blocked` proposal
    /// always names its blocker kind.
    #[test]
    fn blocked_and_stalled_are_mutually_exclusive_and_keep_blocker_kind() {
        let dependency = ProposalStatus::DependencyBlocked {
            blocker: BlockerInfo::new("dependency", "waiting for alpha"),
            revision: RuntimeRevision(1),
        };
        let external = ProposalStatus::ExternalBlocked {
            blocker: external_blocker(),
            revision: RuntimeRevision(1),
        };
        let stalled = ProposalStatus::Stalled {
            blocker: BlockerInfo::new("repeated_acceptance_findings", "no semantic progress"),
            revision: RuntimeRevision(1),
        };

        assert_eq!(dependency.display_status(), "blocked");
        assert_eq!(external.display_status(), "blocked");
        assert_eq!(stalled.display_status(), "stalled");

        assert_eq!(dependency.blocker_kind(), Some(BlockerKind::Dependency));
        assert_eq!(external.blocker_kind(), Some(BlockerKind::External));
        assert_eq!(stalled.blocker_kind(), None);

        // Only the external wait exposes external prerequisite detail.
        assert!(dependency.external_blocker().is_none());
        assert!(stalled.external_blocker().is_none());
        let detail = external.external_blocker().unwrap();
        assert_eq!(detail.origin.as_str(), "acceptance");
        assert_eq!(detail.category, "credential");
        assert!(detail.resumable);
        assert!(detail.summary().contains("unblock when"));

        for status in [&dependency, &external, &stalled] {
            assert!(status.suppresses_ordinary_dispatch());
            assert!(!status.is_terminal());
        }
    }

    /// A dependent proposal's own dependency wait is never overwritten by the
    /// external blocker kind of the proposal it waits on.
    #[test]
    fn dependent_proposal_keeps_dependency_blocker_kind() {
        let alpha = ProposalRuntimeState::with_status(
            "alpha",
            ProposalStatus::ExternalBlocked {
                blocker: external_blocker(),
                revision: RuntimeRevision(1),
            },
        );
        let beta = ProposalRuntimeState::with_status(
            "beta",
            ProposalStatus::DependencyBlocked {
                blocker: BlockerInfo::new("dependency", "waiting for alpha"),
                revision: RuntimeRevision(1),
            },
        );

        assert_eq!(alpha.status.display_status(), beta.status.display_status());
        assert_eq!(alpha.status.blocker_kind(), Some(BlockerKind::External));
        assert_eq!(beta.status.blocker_kind(), Some(BlockerKind::Dependency));
        assert!(beta.status.external_blocker().is_none());
    }

    #[test]
    fn proposal_runtime_state_has_one_canonical_status_field() {
        let proposal = ProposalRuntimeState::with_status(
            "change-a",
            ProposalStatus::Queued {
                revision: RuntimeRevision(1),
            },
        );

        assert_eq!(proposal.id.as_change_id(), "change-a");
        assert_eq!(proposal.status.label(), "queued");
    }
}
