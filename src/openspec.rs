use crate::error::{OrchestratorError, Result};
use crate::task_parser;
use serde::Deserialize;
use std::process::Command;
use tracing::{debug, info};

/// JSON response from openspec list --json
#[derive(Debug, Deserialize)]
struct OpenSpecListResponse {
    changes: Vec<OpenSpecChange>,
}

/// JSON change entry from openspec list --json
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenSpecChange {
    name: String,
    completed_tasks: u32,
    total_tasks: u32,
    #[allow(dead_code)]
    last_modified: String,
    #[allow(dead_code)]
    status: String,
}

/// Represents a change from openspec list
#[derive(Debug, Clone)]
pub struct Change {
    pub id: String,
    pub completed_tasks: u32,
    pub total_tasks: u32,
    #[allow(dead_code)]
    pub last_modified: String,
}

impl Change {
    /// Calculate progress percentage
    pub fn progress_percent(&self) -> f32 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        (self.completed_tasks as f32 / self.total_tasks as f32) * 100.0
    }

    /// Check if all tasks are completed
    pub fn is_complete(&self) -> bool {
        self.completed_tasks == self.total_tasks && self.total_tasks > 0
    }
}

/// Execute openspec list --json and parse the output
pub async fn list_changes(openspec_cmd: &str) -> Result<Vec<Change>> {
    let full_cmd = format!("{} list --json", openspec_cmd);
    info!("Executing: {}", full_cmd);

    let output = Command::new("sh")
        .arg("-c")
        .arg(&full_cmd)
        .output()
        .map_err(|e| OrchestratorError::OpenSpecCommand(format!("Failed to execute: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OrchestratorError::OpenSpecCommand(format!(
            "Command failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8(output.stdout)?;
    debug!("openspec list output:\n{}", stdout);

    let response: OpenSpecListResponse = serde_json::from_str(&stdout)
        .map_err(|e| OrchestratorError::Parse(format!("Failed to parse JSON: {}", e)))?;

    let mut changes: Vec<Change> = response
        .changes
        .into_iter()
        .map(|c| Change {
            id: c.name,
            completed_tasks: c.completed_tasks,
            total_tasks: c.total_tasks,
            last_modified: c.last_modified,
        })
        .collect();

    // Use native parsing as fallback when CLI reports 0/0 task counts
    // This handles the OpenSpec CLI bug where numbered task lists aren't recognized
    for change in &mut changes {
        if change.total_tasks == 0 {
            if let Ok(progress) = task_parser::parse_change(&change.id) {
                debug!(
                    "Native parsing for '{}': {}/{} tasks",
                    change.id, progress.completed, progress.total
                );
                change.completed_tasks = progress.completed;
                change.total_tasks = progress.total;
            }
        }
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_json(json: &str) -> Result<Vec<Change>> {
        let response: OpenSpecListResponse = serde_json::from_str(json)
            .map_err(|e| OrchestratorError::Parse(format!("Failed to parse JSON: {}", e)))?;
        Ok(response
            .changes
            .into_iter()
            .map(|c| Change {
                id: c.name,
                completed_tasks: c.completed_tasks,
                total_tasks: c.total_tasks,
                last_modified: c.last_modified,
            })
            .collect())
    }

    #[test]
    fn test_parse_json_in_progress() {
        let json = r#"{
  "changes": [
    {
      "name": "add-feature",
      "completedTasks": 2,
      "totalTasks": 5,
      "lastModified": "2026-01-08T10:00:00.000Z",
      "status": "in_progress"
    }
  ]
}"#;

        let changes = parse_json(json).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].id, "add-feature");
        assert_eq!(changes[0].completed_tasks, 2);
        assert_eq!(changes[0].total_tasks, 5);
        assert!(!changes[0].is_complete());
    }

    #[test]
    fn test_parse_json_complete() {
        let json = r#"{
  "changes": [
    {
      "name": "add-project-isolation",
      "completedTasks": 21,
      "totalTasks": 21,
      "lastModified": "2026-01-08T11:29:52.427Z",
      "status": "complete"
    },
    {
      "name": "add-sensitive-redaction",
      "completedTasks": 15,
      "totalTasks": 15,
      "lastModified": "2026-01-08T11:14:26.951Z",
      "status": "complete"
    }
  ]
}"#;

        let changes = parse_json(json).unwrap();
        assert_eq!(changes.len(), 2);

        assert_eq!(changes[0].id, "add-project-isolation");
        assert_eq!(changes[0].completed_tasks, 21);
        assert_eq!(changes[0].total_tasks, 21);
        assert!(changes[0].is_complete());

        assert_eq!(changes[1].id, "add-sensitive-redaction");
        assert!(changes[1].is_complete());
    }

    #[test]
    fn test_parse_json_empty() {
        let json = r#"{"changes": []}"#;
        let changes = parse_json(json).unwrap();
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_change_progress() {
        let change = Change {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 0,
            last_modified: "now".to_string(),
        };
        assert_eq!(change.progress_percent(), 0.0);
        assert!(!change.is_complete());
    }

    #[test]
    fn test_change_is_complete() {
        let change = Change {
            id: "test".to_string(),
            completed_tasks: 5,
            total_tasks: 5,
            last_modified: "now".to_string(),
        };
        assert_eq!(change.progress_percent(), 100.0);
        assert!(change.is_complete());
    }
}
