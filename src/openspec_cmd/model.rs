use std::collections::{HashMap, HashSet};

pub(super) struct ChangeInfo {
    pub(super) id: String,
    pub(super) path: String,
    pub(super) title: Option<String>,
    pub(super) tasks_completed: u32,
    pub(super) tasks_total: u32,
    pub(super) dependencies: Vec<String>,
    pub(super) dependency_statuses: Vec<DependencyStatusInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DependencyStatusInfo {
    pub(super) id: String,
    pub(super) status: DependencyListStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DependencyListStatus {
    Done,
    Running,
    Pending,
    Rejected,
    Missing,
}

impl DependencyListStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Running => "running",
            Self::Pending => "pending",
            Self::Rejected => "rejected",
            Self::Missing => "missing",
        }
    }
}

pub(super) struct DependencyStatusContext {
    pub(super) active_ids: HashSet<String>,
    pub(super) in_flight_ids: HashSet<String>,
    pub(super) archived_ids: HashSet<String>,
    pub(super) rejected_ids: HashSet<String>,
}

pub(super) struct SpecInfo {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) requirement_count: usize,
}

pub(super) struct ShowInfo {
    pub(super) id: String,
    pub(super) path: String,
    pub(super) archived: bool,
    pub(super) proposal: Option<String>,
    pub(super) tasks: Option<String>,
    pub(super) tasks_completed: u32,
    pub(super) tasks_total: u32,
    pub(super) dependencies: Vec<String>,
    pub(super) dependency_statuses: Vec<DependencyStatusInfo>,
    pub(super) design: Option<String>,
    pub(super) specs: HashMap<String, String>,
}
