//! Type definitions for remote server communication

use serde::{Deserialize, Serialize};

/// A change returned by the remote server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteChange {
    /// Change ID (e.g., "add-feature-x")
    pub id: String,
    /// Project identifier (e.g., remote URL + branch)
    pub project: String,
    /// Number of completed tasks
    pub completed_tasks: u32,
    /// Total number of tasks
    pub total_tasks: u32,
    /// Last modification timestamp (ISO 8601)
    pub last_modified: String,
    /// Current status of the change
    pub status: String,
    /// Iteration number (for apply/archive/acceptance operations)
    pub iteration_number: Option<u32>,
}

/// A project group from the remote server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteProject {
    /// Project identifier (e.g., repo URL + branch)
    pub id: String,
    /// Human-readable project name
    pub name: String,
    /// Changes belonging to this project
    pub changes: Vec<RemoteChange>,
}

/// A log entry streamed from the remote server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteLogEntry {
    /// Log message content
    pub message: String,
    /// Log level: "info", "warn", "error", "success"
    pub level: String,
    /// Optional change ID this log belongs to
    pub change_id: Option<String>,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Optional project ID this log belongs to (for project-level log association)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Optional operation type (apply, archive, resolve, analyze)
    /// Always serialized (even as null) so clients can rely on the key being present.
    pub operation: Option<String>,
    /// Optional iteration number for apply/archive operations
    /// Always serialized (even as null) so clients can rely on the key being present.
    pub iteration: Option<u32>,
}

/// A project entry from the `GET /api/v1/projects` endpoint.
///
/// Returned by the server's project list endpoint (not the state endpoint).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectEntry {
    /// Project identifier (e.g., "abc123def")
    pub id: String,
    /// Remote URL of the project repository
    pub remote_url: String,
    /// Branch being tracked
    pub branch: String,
    /// Current status: "idle", "running", etc.
    pub status: String,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
}

/// Worktree information for server-mode API responses.
///
/// This type is used by the Server Mode `/api/v1/projects/{id}/worktrees` endpoints
/// and WebSocket `full_state` messages. It mirrors `tui::types::WorktreeInfo` but is
/// tailored for remote (server-mode) communication with `path` serialized as a String
/// and an additional `label` field for display purposes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteWorktreeInfo {
    /// Worktree filesystem path (as string for serialization)
    pub path: String,
    /// Human-readable label (typically the branch name or change id)
    pub label: String,
    /// Current HEAD commit (short hash)
    pub head: String,
    /// Branch name (empty if detached)
    pub branch: String,
    /// Whether HEAD is detached
    pub is_detached: bool,
    /// Whether this is the main worktree
    pub is_main: bool,
    /// Whether a merge operation is in progress
    pub is_merging: bool,
    /// Whether this worktree has commits ahead of the base branch
    pub has_commits_ahead: bool,
    /// Merge conflict information (None if no conflicts)
    pub merge_conflict: Option<RemoteWorktreeMergeConflict>,
}

/// Merge conflict details for a remote worktree
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteWorktreeMergeConflict {
    /// List of files with merge conflicts
    pub conflict_files: Vec<String>,
}

impl From<crate::tui::types::WorktreeInfo> for RemoteWorktreeInfo {
    fn from(wt: crate::tui::types::WorktreeInfo) -> Self {
        let label = if wt.branch.is_empty() {
            format!("detached@{}", &wt.head[..7.min(wt.head.len())])
        } else {
            wt.branch.clone()
        };
        Self {
            path: wt.path.to_string_lossy().to_string(),
            label,
            head: wt.head,
            branch: wt.branch,
            is_detached: wt.is_detached,
            is_main: wt.is_main,
            is_merging: wt.is_merging,
            has_commits_ahead: wt.has_commits_ahead,
            merge_conflict: wt.merge_conflict.map(|mc| RemoteWorktreeMergeConflict {
                conflict_files: mc.conflict_files,
            }),
        }
    }
}

/// A state update message received over WebSocket
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteStateUpdate {
    /// Full state snapshot (sent on connect or full refresh)
    FullState {
        projects: Vec<RemoteProject>,
        /// Per-project worktree information (project_id -> worktrees)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        worktrees: Option<std::collections::HashMap<String, Vec<RemoteWorktreeInfo>>>,
    },
    /// Incremental update for a single change
    ChangeUpdate { change: RemoteChange },
    /// A change was removed
    ChangeRemoved { id: String, project: String },
    /// A log entry from server execution
    Log { entry: RemoteLogEntry },
    /// Heartbeat / keep-alive
    Ping,
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{
        change_update_json, full_state_json, make_remote_change, make_remote_log_entry,
        make_remote_project, remote_change_json,
    };
    use super::*;

    #[test]
    fn test_remote_change_deserialization() {
        let json = remote_change_json("add-feature-x", "my-project", 3, 5, "applying", Some(2));

        let change: RemoteChange = serde_json::from_str(&json).unwrap();
        assert_eq!(change.id, "add-feature-x");
        assert_eq!(change.project, "my-project");
        assert_eq!(change.completed_tasks, 3);
        assert_eq!(change.total_tasks, 5);
        assert_eq!(change.status, "applying");
        assert_eq!(change.iteration_number, Some(2));
    }

    #[test]
    fn test_remote_state_update_full_state_deserialization() {
        let json = full_state_json("proj-1", "Project 1", &[]);

        let update: RemoteStateUpdate = serde_json::from_str(&json).unwrap();
        match update {
            RemoteStateUpdate::FullState { projects, .. } => {
                assert_eq!(projects.len(), 1);
                assert_eq!(projects[0].id, "proj-1");
            }
            _ => panic!("Expected FullState"),
        }
    }

    #[test]
    fn test_remote_state_update_change_update_deserialization() {
        let change = remote_change_json("my-change", "proj-1", 1, 3, "queued", None);
        let json = change_update_json(&change);

        let update: RemoteStateUpdate = serde_json::from_str(&json).unwrap();
        match update {
            RemoteStateUpdate::ChangeUpdate { change } => {
                assert_eq!(change.id, "my-change");
                assert_eq!(change.iteration_number, None);
            }
            _ => panic!("Expected ChangeUpdate"),
        }
    }

    #[test]
    fn test_remote_state_update_ping_deserialization() {
        let json = r#"{"type": "ping"}"#;
        let update: RemoteStateUpdate = serde_json::from_str(json).unwrap();
        assert!(matches!(update, RemoteStateUpdate::Ping));
    }

    /// Verify that the struct builders produce correct default values.
    #[test]
    fn test_make_remote_change_defaults() {
        let change = make_remote_change("test-id", "test-project");
        assert_eq!(change.id, "test-id");
        assert_eq!(change.project, "test-project");
        assert_eq!(change.status, "queued");
        assert_eq!(change.iteration_number, None);
    }

    /// Verify that make_remote_project correctly assembles a project with changes.
    #[test]
    fn test_make_remote_project() {
        let changes = vec![
            make_remote_change("change-1", "proj-a"),
            make_remote_change("change-2", "proj-a"),
        ];
        let project = make_remote_project("proj-a", "Project A", changes);
        assert_eq!(project.id, "proj-a");
        assert_eq!(project.name, "Project A");
        assert_eq!(project.changes.len(), 2);
    }

    /// Verify that make_remote_log_entry produces a valid log entry.
    #[test]
    fn test_make_remote_log_entry_defaults() {
        let entry = make_remote_log_entry("test message", "info");
        assert_eq!(entry.message, "test message");
        assert_eq!(entry.level, "info");
        assert_eq!(entry.change_id, None);
        assert_eq!(entry.operation, None);
    }

    /// Verify that a RemoteLogEntry can be serialized and deserialized correctly (round-trip).
    #[test]
    fn test_remote_log_entry_round_trip() {
        let entry = RemoteLogEntry {
            message: "Running apply for change add-feature-x".to_string(),
            level: "info".to_string(),
            change_id: Some("add-feature-x".to_string()),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            project_id: Some("proj-abc123".to_string()),
            operation: Some("apply".to_string()),
            iteration: Some(1),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let decoded: RemoteLogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.message, entry.message);
        assert_eq!(decoded.level, entry.level);
        assert_eq!(decoded.change_id, entry.change_id);
        assert_eq!(decoded.timestamp, entry.timestamp);
        assert_eq!(decoded.project_id, entry.project_id);
        assert_eq!(decoded.operation, entry.operation);
        assert_eq!(decoded.iteration, entry.iteration);
    }

    /// Verify that RemoteStateUpdate::Log is serialized with type="log" and can be deserialized.
    #[test]
    fn test_remote_state_update_log_round_trip() {
        let entry = RemoteLogEntry {
            message: "stderr: Build failed".to_string(),
            level: "warn".to_string(),
            change_id: None,
            timestamp: "2024-06-01T12:00:00Z".to_string(),
            project_id: Some("proj-xyz789".to_string()),
            operation: None,
            iteration: None,
        };

        let update = RemoteStateUpdate::Log {
            entry: entry.clone(),
        };
        let json = serde_json::to_string(&update).unwrap();

        // Verify the type tag is correct
        assert!(json.contains(r#""type":"log""#));

        // Verify that operation and iteration keys are always present in JSON output
        // even when their values are None (null), so clients can rely on key presence.
        assert!(
            json.contains(r#""operation":null"#),
            "operation key must be present as null, got: {json}"
        );
        assert!(
            json.contains(r#""iteration":null"#),
            "iteration key must be present as null, got: {json}"
        );

        let decoded: RemoteStateUpdate = serde_json::from_str(&json).unwrap();
        match decoded {
            RemoteStateUpdate::Log {
                entry: decoded_entry,
            } => {
                assert_eq!(decoded_entry.message, entry.message);
                assert_eq!(decoded_entry.level, entry.level);
                assert_eq!(decoded_entry.change_id, entry.change_id);
                assert_eq!(decoded_entry.timestamp, entry.timestamp);
            }
            _ => panic!("Expected Log variant"),
        }
    }

    /// Verify RemoteWorktreeInfo conversion from WorktreeInfo.
    #[test]
    fn test_remote_worktree_info_from_worktree_info() {
        let wt = crate::tui::types::WorktreeInfo {
            path: std::path::PathBuf::from("/repo/wt1"),
            head: "abc123def456".to_string(),
            branch: "feature-1".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        };

        let remote: RemoteWorktreeInfo = wt.into();
        assert_eq!(remote.path, "/repo/wt1");
        assert_eq!(remote.label, "feature-1");
        assert_eq!(remote.head, "abc123def456");
        assert_eq!(remote.branch, "feature-1");
        assert!(!remote.is_detached);
        assert!(!remote.is_main);
        assert!(remote.has_commits_ahead);
        assert!(remote.merge_conflict.is_none());
    }

    /// Verify RemoteWorktreeInfo conversion for detached HEAD.
    #[test]
    fn test_remote_worktree_info_detached_head() {
        let wt = crate::tui::types::WorktreeInfo {
            path: std::path::PathBuf::from("/repo"),
            head: "abc1234".to_string(),
            branch: "".to_string(),
            is_detached: true,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        };

        let remote: RemoteWorktreeInfo = wt.into();
        assert_eq!(remote.label, "detached@abc1234");
        assert!(remote.is_detached);
    }

    /// Verify RemoteWorktreeInfo conversion with merge conflicts.
    #[test]
    fn test_remote_worktree_info_with_conflicts() {
        let wt = crate::tui::types::WorktreeInfo {
            path: std::path::PathBuf::from("/repo/wt1"),
            head: "abc123".to_string(),
            branch: "feature-1".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: Some(crate::tui::types::MergeConflictInfo {
                conflict_files: vec!["file1.rs".to_string(), "file2.rs".to_string()],
            }),
            has_commits_ahead: true,
            is_merging: false,
        };

        let remote: RemoteWorktreeInfo = wt.into();
        assert!(remote.merge_conflict.is_some());
        let conflict = remote.merge_conflict.unwrap();
        assert_eq!(conflict.conflict_files.len(), 2);
        assert_eq!(conflict.conflict_files[0], "file1.rs");
    }

    /// Verify RemoteWorktreeInfo serialization round-trip.
    #[test]
    fn test_remote_worktree_info_serialization_round_trip() {
        let info = RemoteWorktreeInfo {
            path: "/repo/wt1".to_string(),
            label: "feature-1".to_string(),
            head: "abc123".to_string(),
            branch: "feature-1".to_string(),
            is_detached: false,
            is_main: false,
            is_merging: false,
            has_commits_ahead: true,
            merge_conflict: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let decoded: RemoteWorktreeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, info);
    }

    /// Verify FullState with worktrees field serialization.
    #[test]
    fn test_full_state_with_worktrees() {
        let mut worktrees_map = std::collections::HashMap::new();
        worktrees_map.insert(
            "proj-1".to_string(),
            vec![RemoteWorktreeInfo {
                path: "/repo/wt1".to_string(),
                label: "main".to_string(),
                head: "abc123".to_string(),
                branch: "main".to_string(),
                is_detached: false,
                is_main: true,
                is_merging: false,
                has_commits_ahead: false,
                merge_conflict: None,
            }],
        );

        let update = RemoteStateUpdate::FullState {
            projects: vec![],
            worktrees: Some(worktrees_map),
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("worktrees"));

        let decoded: RemoteStateUpdate = serde_json::from_str(&json).unwrap();
        match decoded {
            RemoteStateUpdate::FullState { worktrees, .. } => {
                assert!(worktrees.is_some());
                let wts = worktrees.unwrap();
                assert_eq!(wts.len(), 1);
                assert!(wts.contains_key("proj-1"));
            }
            _ => panic!("Expected FullState"),
        }
    }

    /// Verify FullState without worktrees field (backward compatibility).
    #[test]
    fn test_full_state_without_worktrees_backward_compatible() {
        // Old-style JSON without worktrees field should still deserialize
        let json = r#"{
            "type": "full_state",
            "projects": []
        }"#;

        let update: RemoteStateUpdate = serde_json::from_str(json).unwrap();
        match update {
            RemoteStateUpdate::FullState { worktrees, .. } => {
                assert!(worktrees.is_none());
            }
            _ => panic!("Expected FullState"),
        }
    }

    /// Verify deserialization from a raw JSON string with type="log".
    #[test]
    fn test_remote_state_update_log_deserialization_from_json() {
        let json = r#"{
            "type": "log",
            "entry": {
                "message": "Build succeeded",
                "level": "success",
                "change_id": "feat-x",
                "timestamp": "2024-01-02T09:00:00Z"
            }
        }"#;

        let update: RemoteStateUpdate = serde_json::from_str(json).unwrap();
        match update {
            RemoteStateUpdate::Log { entry } => {
                assert_eq!(entry.message, "Build succeeded");
                assert_eq!(entry.level, "success");
                assert_eq!(entry.change_id, Some("feat-x".to_string()));
                assert_eq!(entry.timestamp, "2024-01-02T09:00:00Z");
            }
            _ => panic!("Expected Log variant"),
        }
    }
}
