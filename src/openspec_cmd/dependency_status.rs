use crate::dependency_targets::{self, DependencyTargetClass};
use crate::openspec_cmd::model::{
    DependencyListStatus, DependencyStatusContext, DependencyStatusInfo,
};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

impl DependencyStatusContext {
    pub(super) fn from_workspace(root_dir: &Path) -> Self {
        Self {
            active_ids: collect_active_change_ids_from_root(root_dir),
            in_flight_ids: collect_in_flight_change_ids_from_root(root_dir),
            archived_ids: dependency_targets::collect_archived_change_ids(root_dir),
            rejected_ids: dependency_targets::collect_rejected_change_ids(root_dir),
        }
    }

    pub(super) fn statuses_for(&self, dependencies: &[String]) -> Vec<DependencyStatusInfo> {
        dependencies
            .iter()
            .map(|dependency| DependencyStatusInfo {
                id: dependency.clone(),
                status: self.status_for(dependency),
            })
            .collect()
    }

    pub(super) fn status_for(&self, dependency: &str) -> DependencyListStatus {
        match dependency_targets::classify_dependency_target(
            dependency,
            self.active_ids.iter().map(String::as_str),
            self.in_flight_ids.iter().map(String::as_str),
            self.active_ids.iter().map(String::as_str),
            &self.archived_ids,
            &self.rejected_ids,
        ) {
            DependencyTargetClass::Archived => DependencyListStatus::Done,
            DependencyTargetClass::InFlight | DependencyTargetClass::Resolving => {
                DependencyListStatus::Running
            }
            DependencyTargetClass::Queued | DependencyTargetClass::ActiveButNotQueued => {
                DependencyListStatus::Pending
            }
            DependencyTargetClass::Rejected => DependencyListStatus::Rejected,
            DependencyTargetClass::Error => {
                unreachable!("dependency list classification cannot produce terminal-error state")
            }
            DependencyTargetClass::Missing => DependencyListStatus::Missing,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct DependencyTargetDiagnostic {
    pub(super) classification: DependencyTargetClass,
    pub(super) message: String,
}

pub(super) fn classify_proposal_dependency_targets(
    change_id: &str,
    proposal_file: &Path,
) -> Vec<DependencyTargetDiagnostic> {
    let proposal_metadata = crate::openspec::parse_proposal_metadata_from_file(proposal_file);
    if proposal_metadata.dependencies.is_empty() {
        return Vec::new();
    }

    let active_ids = collect_active_change_ids();
    let in_flight_ids = collect_in_flight_change_ids();
    let archived_ids = collect_archived_change_ids();
    let rejected_ids = dependency_targets::collect_rejected_change_ids(Path::new("."));

    proposal_metadata
        .dependencies
        .into_iter()
        .map(|dependency| {
            let classification = dependency_targets::classify_dependency_target(
                &dependency,
                active_ids.iter().map(String::as_str),
                in_flight_ids.iter().map(String::as_str),
                active_ids.iter().map(String::as_str),
                &archived_ids,
                &rejected_ids,
            );

            let message = match classification {
                DependencyTargetClass::Queued => format!(
                    "{}: proposal dependency '{}' classified as queued (active change)",
                    change_id, dependency
                ),
                DependencyTargetClass::InFlight => format!(
                    "{}: proposal dependency '{}' classified as in-flight (workspace execution marker)",
                    change_id, dependency
                ),
                DependencyTargetClass::Resolving => format!(
                    "{}: proposal dependency '{}' classified as resolving",
                    change_id, dependency
                ),
                DependencyTargetClass::ActiveButNotQueued => format!(
                    "{}: proposal dependency '{}' classified as active-but-not-queued (active change exists outside the queued set)",
                    change_id, dependency
                ),
                DependencyTargetClass::Archived => format!(
                    "{}: proposal dependency '{}' classified as archived dependency reference (warning: metadata should be reviewed after archive)",
                    change_id, dependency
                ),
                DependencyTargetClass::Rejected => format!(
                    "{}: proposal dependency '{}' is invalid: classified as rejected dependency target",
                    change_id, dependency
                ),
                DependencyTargetClass::Error => unreachable!(
                    "proposal dependency classification cannot produce terminal-error state"
                ),
                DependencyTargetClass::Missing => format!(
                    "{}: proposal dependency '{}' is invalid: not found in active, in-flight, archived, or rejected change targets",
                    change_id, dependency
                ),
            };

            DependencyTargetDiagnostic {
                classification,
                message,
            }
        })
        .collect()
}

pub(super) fn collect_active_change_ids() -> HashSet<String> {
    collect_active_change_ids_from_root(Path::new("."))
}

pub(super) fn collect_active_change_ids_from_root(root_dir: &Path) -> HashSet<String> {
    let changes_dir = root_dir.join("openspec/changes");
    let Ok(entries) = fs::read_dir(changes_dir) else {
        return HashSet::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "archive" || name.starts_with('.') {
                return None;
            }
            if !path.join("proposal.md").exists() || path.join("REJECTED.md").exists() {
                return None;
            }
            Some(name)
        })
        .collect()
}

pub(super) fn collect_in_flight_change_ids() -> HashSet<String> {
    collect_in_flight_change_ids_from_root(Path::new("."))
}

pub(super) fn collect_in_flight_change_ids_from_root(root_dir: &Path) -> HashSet<String> {
    let state_file = root_dir.join(".conflux-inflight");
    let Ok(content) = fs::read_to_string(state_file) else {
        return HashSet::new();
    };

    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(super) fn collect_archived_change_ids() -> HashSet<String> {
    dependency_targets::collect_archived_change_ids(Path::new("."))
}
