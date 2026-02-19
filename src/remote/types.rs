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
    /// A log entry from server execution
    Log { entry: RemoteLogEntry },
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

    /// Verify that a RemoteLogEntry can be serialized and deserialized correctly (round-trip).
    #[test]
    fn test_remote_log_entry_round_trip() {
        let entry = RemoteLogEntry {
            message: "Running apply for change add-feature-x".to_string(),
            level: "info".to_string(),
            change_id: Some("add-feature-x".to_string()),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let decoded: RemoteLogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.message, entry.message);
        assert_eq!(decoded.level, entry.level);
        assert_eq!(decoded.change_id, entry.change_id);
        assert_eq!(decoded.timestamp, entry.timestamp);
    }

    /// Verify that RemoteStateUpdate::Log is serialized with type="log" and can be deserialized.
    #[test]
    fn test_remote_state_update_log_round_trip() {
        let entry = RemoteLogEntry {
            message: "stderr: Build failed".to_string(),
            level: "warn".to_string(),
            change_id: None,
            timestamp: "2024-06-01T12:00:00Z".to_string(),
        };

        let update = RemoteStateUpdate::Log {
            entry: entry.clone(),
        };
        let json = serde_json::to_string(&update).unwrap();

        // Verify the type tag is correct
        assert!(json.contains(r#""type":"log""#));

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
