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

/// A state update message received over WebSocket
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteStateUpdate {
    /// Full state snapshot (sent on connect or full refresh)
    FullState { projects: Vec<RemoteProject> },
    /// Incremental update for a single change
    ChangeUpdate { change: RemoteChange },
    /// A change was removed
    ChangeRemoved { id: String, project: String },
    /// Heartbeat / keep-alive
    Ping,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_change_deserialization() {
        let json = r#"{
            "id": "add-feature-x",
            "project": "my-project",
            "completed_tasks": 3,
            "total_tasks": 5,
            "last_modified": "2024-01-01T00:00:00Z",
            "status": "applying",
            "iteration_number": 2
        }"#;

        let change: RemoteChange = serde_json::from_str(json).unwrap();
        assert_eq!(change.id, "add-feature-x");
        assert_eq!(change.project, "my-project");
        assert_eq!(change.completed_tasks, 3);
        assert_eq!(change.total_tasks, 5);
        assert_eq!(change.status, "applying");
        assert_eq!(change.iteration_number, Some(2));
    }

    #[test]
    fn test_remote_state_update_full_state_deserialization() {
        let json = r#"{
            "type": "full_state",
            "projects": [
                {
                    "id": "proj-1",
                    "name": "Project 1",
                    "changes": []
                }
            ]
        }"#;

        let update: RemoteStateUpdate = serde_json::from_str(json).unwrap();
        match update {
            RemoteStateUpdate::FullState { projects } => {
                assert_eq!(projects.len(), 1);
                assert_eq!(projects[0].id, "proj-1");
            }
            _ => panic!("Expected FullState"),
        }
    }

    #[test]
    fn test_remote_state_update_change_update_deserialization() {
        let json = r#"{
            "type": "change_update",
            "change": {
                "id": "my-change",
                "project": "proj-1",
                "completed_tasks": 1,
                "total_tasks": 3,
                "last_modified": "2024-01-01T00:00:00Z",
                "status": "queued",
                "iteration_number": null
            }
        }"#;

        let update: RemoteStateUpdate = serde_json::from_str(json).unwrap();
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
}
