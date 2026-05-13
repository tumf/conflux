//! Repository-visible dependency target classification helpers.
//!
//! The classifier is intentionally small and deterministic: callers provide the
//! queued and in-flight sets from scheduler/analyzer state, plus archive evidence
//! collected from the workspace or base tree. No durable state outside the
//! repository participates in the decision.

use std::collections::HashSet;
use std::path::Path;
use tracing::warn;

/// Classification for a dependency target referenced by an active change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DependencyTargetClass {
    Queued,
    InFlight,
    ActiveButNotQueued,
    Archived,
    Rejected,
    Error,
    Missing,
}

impl DependencyTargetClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::InFlight => "in-flight",
            Self::ActiveButNotQueued => "active-but-not-queued",
            Self::Archived => "archived",
            Self::Rejected => "rejected",
            Self::Error => "errored",
            Self::Missing => "missing",
        }
    }
}

/// Strip Conflux archive date prefixes such as `2026-04-29-change-id`.
pub(crate) fn strip_archive_date_prefix(name: &str) -> &str {
    if name.len() > 11 {
        let bytes = name.as_bytes();
        let has_date_prefix = bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[10] == b'-'
            && bytes[..4].iter().all(u8::is_ascii_digit)
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[8..10].iter().all(u8::is_ascii_digit);
        if has_date_prefix {
            return &name[11..];
        }
    }
    name
}

/// Classify a dependency using already-collected repository-visible evidence.
pub(crate) fn classify_dependency_target<'a>(
    dep_id: &str,
    queued_ids: impl IntoIterator<Item = &'a str>,
    in_flight_ids: impl IntoIterator<Item = &'a str>,
    active_ids: impl IntoIterator<Item = &'a str>,
    archived_ids: &HashSet<String>,
    rejected_ids: &HashSet<String>,
) -> DependencyTargetClass {
    if archived_ids.contains(dep_id) {
        DependencyTargetClass::Archived
    } else if rejected_ids.contains(dep_id) {
        DependencyTargetClass::Rejected
    } else if in_flight_ids.into_iter().any(|id| id == dep_id) {
        DependencyTargetClass::InFlight
    } else if queued_ids.into_iter().any(|id| id == dep_id) {
        DependencyTargetClass::Queued
    } else if active_ids.into_iter().any(|id| id == dep_id) {
        DependencyTargetClass::ActiveButNotQueued
    } else {
        DependencyTargetClass::Missing
    }
}

/// Collect active, non-archived change IDs under `openspec/changes`.
pub(crate) fn collect_active_change_ids(repo_root: &Path) -> HashSet<String> {
    let changes_dir = repo_root.join("openspec/changes");
    let Ok(entries) = std::fs::read_dir(changes_dir) else {
        return HashSet::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "archive"
                || name.starts_with('.')
                || !path.is_dir()
                || !path.join("proposal.md").exists()
            {
                return None;
            }
            Some(name)
        })
        .collect()
}

/// Collect archive IDs from `openspec/changes/archive` under a repository root.
pub(crate) fn collect_archived_change_ids(repo_root: &Path) -> HashSet<String> {
    let archive_dir = repo_root.join("openspec/changes/archive");
    let Ok(entries) = std::fs::read_dir(&archive_dir) else {
        return HashSet::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() || !path.join("proposal.md").exists() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            Some(strip_archive_date_prefix(&name).to_string())
        })
        .collect()
}

/// Collect rejected active change IDs from terminal `REJECTED.md` markers.
pub(crate) fn collect_rejected_change_ids(repo_root: &Path) -> HashSet<String> {
    let changes_dir = repo_root.join("openspec/changes");
    let Ok(entries) = std::fs::read_dir(changes_dir) else {
        return HashSet::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir()
                || !path.join("proposal.md").exists()
                || !path.join("REJECTED.md").exists()
            {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "archive" || name.starts_with('.') {
                return None;
            }
            Some(name)
        })
        .collect()
}

/// Add authoritative proposal metadata/body dependencies to an analysis result.
pub(crate) fn union_metadata_dependencies(
    dependencies: &mut std::collections::HashMap<String, Vec<String>>,
    change_id: &str,
    metadata_dependencies: &[String],
) {
    if metadata_dependencies.is_empty() {
        return;
    }

    let entry = dependencies.entry(change_id.to_string()).or_default();
    for dep_id in metadata_dependencies {
        if dep_id == change_id {
            warn!(
                change_id,
                dependency = dep_id,
                "Ignoring self-referential proposal metadata dependency during dependency union"
            );
            continue;
        }
        if !entry.contains(dep_id) {
            entry.push(dep_id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_dependency_targets_from_supplied_evidence() {
        let archived = HashSet::from(["archived-a".to_string()]);
        let rejected = HashSet::from(["rejected-a".to_string()]);

        assert_eq!(
            classify_dependency_target(
                "queued-a",
                ["queued-a"].into_iter(),
                [].into_iter(),
                [].into_iter(),
                &archived,
                &rejected,
            ),
            DependencyTargetClass::Queued
        );
        assert_eq!(
            classify_dependency_target(
                "flight-a",
                [].into_iter(),
                ["flight-a"].into_iter(),
                [].into_iter(),
                &archived,
                &rejected,
            ),
            DependencyTargetClass::InFlight
        );
        assert_eq!(
            classify_dependency_target(
                "active-a",
                [].into_iter(),
                [].into_iter(),
                ["active-a"].into_iter(),
                &archived,
                &rejected,
            ),
            DependencyTargetClass::ActiveButNotQueued
        );
        assert_eq!(
            classify_dependency_target(
                "archived-a",
                [].into_iter(),
                [].into_iter(),
                [].into_iter(),
                &archived,
                &rejected,
            ),
            DependencyTargetClass::Archived
        );
        assert_eq!(
            classify_dependency_target(
                "rejected-a",
                ["rejected-a"].into_iter(),
                [].into_iter(),
                [].into_iter(),
                &archived,
                &rejected,
            ),
            DependencyTargetClass::Rejected
        );
        assert_eq!(
            classify_dependency_target(
                "missing-a",
                [].into_iter(),
                [].into_iter(),
                [].into_iter(),
                &archived,
                &rejected,
            ),
            DependencyTargetClass::Missing
        );
        assert_eq!(DependencyTargetClass::Error.as_str(), "errored");
    }

    #[test]
    fn collects_rejected_change_ids_from_repository_markers() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes");
        std::fs::create_dir_all(changes_dir.join("rejected-a")).unwrap();
        std::fs::write(changes_dir.join("rejected-a/proposal.md"), "# Rejected\n").unwrap();
        std::fs::write(changes_dir.join("rejected-a/REJECTED.md"), "# REJECTED\n").unwrap();
        std::fs::create_dir_all(changes_dir.join("active-a")).unwrap();
        std::fs::write(changes_dir.join("active-a/proposal.md"), "# Active\n").unwrap();

        let rejected = collect_rejected_change_ids(temp_dir.path());

        assert!(rejected.contains("rejected-a"));
        assert!(!rejected.contains("active-a"));
    }

    #[test]
    fn collects_active_change_ids_from_repository_markers() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes");
        std::fs::create_dir_all(changes_dir.join("active-a")).unwrap();
        std::fs::write(changes_dir.join("active-a/proposal.md"), "# Active\n").unwrap();
        std::fs::create_dir_all(changes_dir.join("archive/2026-05-13-done-a")).unwrap();
        std::fs::write(
            changes_dir.join("archive/2026-05-13-done-a/proposal.md"),
            "# Done\n",
        )
        .unwrap();
        std::fs::create_dir_all(changes_dir.join("no-proposal")).unwrap();

        let active = collect_active_change_ids(temp_dir.path());

        assert!(active.contains("active-a"));
        assert!(!active.contains("archive"));
        assert!(!active.contains("no-proposal"));
        assert!(!active.contains("done-a"));
    }

    #[test]
    fn strips_date_prefixed_archive_names() {
        assert_eq!(
            strip_archive_date_prefix("2026-04-29-sample-change"),
            "sample-change"
        );
        assert_eq!(strip_archive_date_prefix("sample-change"), "sample-change");
        assert_eq!(
            strip_archive_date_prefix("2026-4-29-sample-change"),
            "2026-4-29-sample-change"
        );
    }

    #[test]
    fn unions_metadata_dependencies_without_removing_existing_dependencies() {
        let mut deps = std::collections::HashMap::from([(
            "route".to_string(),
            vec!["llm-discovered".to_string()],
        )]);

        union_metadata_dependencies(
            &mut deps,
            "route",
            &["policy".to_string(), "llm-discovered".to_string()],
        );

        assert_eq!(
            deps.get("route").cloned().unwrap(),
            vec!["llm-discovered".to_string(), "policy".to_string()]
        );
    }
}
