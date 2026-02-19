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
            RemoteStateUpdate::FullState { projects } => {
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
