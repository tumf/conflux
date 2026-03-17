//! Mapping from remote server types to local TUI types
//!
//! Converts `RemoteChange` / `RemoteProject` (server response types) to the
//! local `Change` type used by the TUI, so the same display logic can render
//! both local and remote data without modification.

use crate::openspec::Change;

use super::types::{RemoteChange, RemoteProject};

/// Convert a [`RemoteChange`] to a local [`Change`] used by the TUI.
///
/// The `project` field from the remote change is discarded at this layer; it is
/// used for grouping before calling this function.
pub fn remote_change_to_local(remote: &RemoteChange) -> Change {
    Change {
        id: remote.id.clone(),
        completed_tasks: remote.completed_tasks,
        total_tasks: remote.total_tasks,
        last_modified: remote.last_modified.clone(),
        dependencies: Vec::new(), // Dependencies are not available from remote API
    }
}

/// Group remote changes by project and return a flat list of [`Change`]s.
///
/// The returned list is ordered project-by-project so the TUI naturally shows
/// changes grouped together. Within each project the order is stable (preserving
/// server order).
///
/// The project name is prepended to the change ID as `"<project>/<change_id>"` so
/// that the user can visually identify which project each change belongs to.
pub fn group_changes_by_project(projects: &[RemoteProject]) -> Vec<Change> {
    let mut result = Vec::new();
    for project in projects {
        for change in &project.changes {
            // Prefix the change ID with the project name so grouping is visible in the TUI
            let mut local = remote_change_to_local(change);
            // Encode project.id into the local change id so that client-side actions
            // (run/stop/retry) can target the correct project while keeping the display
            // portion human-friendly.
            // Format: "<project_id>::<project_name>/<change_id>"
            local.id = format!("{}::{}/{}", project.id, project.name, change.id);
            result.push(local);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::types::{RemoteChange, RemoteProject};

    fn make_remote_change(id: &str, project: &str, done: u32, total: u32) -> RemoteChange {
        RemoteChange {
            id: id.to_string(),
            project: project.to_string(),
            completed_tasks: done,
            total_tasks: total,
            last_modified: "2024-01-01T00:00:00Z".to_string(),
            status: "applying".to_string(),
            iteration_number: None,
        }
    }

    #[test]
    fn test_remote_change_to_local() {
        let remote = make_remote_change("my-change", "proj-1", 2, 5);
        let local = remote_change_to_local(&remote);
        assert_eq!(local.id, "my-change");
        assert_eq!(local.completed_tasks, 2);
        assert_eq!(local.total_tasks, 5);
        assert!(local.dependencies.is_empty());
    }

    #[test]
    fn test_group_changes_by_project_prefixes_ids() {
        let projects = vec![
            RemoteProject {
                id: "proj-1".to_string(),
                name: "Project One".to_string(),
                changes: vec![
                    make_remote_change("change-a", "proj-1", 1, 3),
                    make_remote_change("change-b", "proj-1", 0, 2),
                ],
            },
            RemoteProject {
                id: "proj-2".to_string(),
                name: "Project Two".to_string(),
                changes: vec![make_remote_change("change-c", "proj-2", 3, 3)],
            },
        ];

        let result = group_changes_by_project(&projects);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, "proj-1::Project One/change-a");
        assert_eq!(result[1].id, "proj-1::Project One/change-b");
        assert_eq!(result[2].id, "proj-2::Project Two/change-c");
    }
}
