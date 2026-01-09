use crate::error::{OrchestratorError, Result};
use crate::task_parser;
use serde::Deserialize;
use std::fs;
use std::path::Path;
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
#[derive(Debug, Clone, PartialEq)]
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

/// List changes by directly reading the openspec/changes directory.
///
/// This is the native implementation that avoids calling the external
/// `openspec list --json` command. It reads the directory structure and
/// parses tasks.md files to get accurate task progress.
pub fn list_changes_native() -> Result<Vec<Change>> {
    let changes_dir = Path::new("openspec/changes");

    if !changes_dir.exists() {
        debug!("Changes directory does not exist: {:?}", changes_dir);
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(changes_dir).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to read changes directory: {}", e))
    })?;

    let mut changes = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| {
            OrchestratorError::ConfigLoad(format!("Failed to read directory entry: {}", e))
        })?;

        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        // Skip special directories like 'archive'
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if dir_name == "archive" || dir_name.starts_with('.') {
            continue;
        }

        // Parse tasks.md for this change
        let (completed_tasks, total_tasks) = match task_parser::parse_change(dir_name) {
            Ok(progress) => (progress.completed, progress.total),
            Err(_) => {
                // If tasks.md doesn't exist or can't be parsed, use 0/0
                debug!("Could not parse tasks for change '{}', using 0/0", dir_name);
                (0, 0)
            }
        };

        changes.push(Change {
            id: dir_name.to_string(),
            completed_tasks,
            total_tasks,
            last_modified: String::new(), // Not used in native implementation
        });
    }

    // Sort by id for consistent ordering
    changes.sort_by(|a, b| a.id.cmp(&b.id));

    debug!("Found {} changes via native parsing", changes.len());
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

    // ====================
    // list_changes_native tests
    // ====================

    // Note: These tests run in the actual project directory which has openspec/changes
    // The function relies on relative paths, so these tests validate real behavior

    #[test]
    fn test_list_changes_native_returns_ok() {
        // Test that the function returns Ok when run in a valid openspec project
        // This test verifies the basic functionality works in the real project structure
        let result = list_changes_native();
        // The function should always return Ok (empty vec if dir doesn't exist)
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_changes_native_excludes_archive() {
        // The result should not include "archive" as a change ID
        let result = list_changes_native().unwrap();
        assert!(
            !result.iter().any(|c| c.id == "archive"),
            "archive directory should be excluded"
        );
    }

    #[test]
    fn test_list_changes_native_excludes_hidden() {
        // The result should not include any hidden directories (starting with .)
        let result = list_changes_native().unwrap();
        assert!(
            !result.iter().any(|c| c.id.starts_with('.')),
            "hidden directories should be excluded"
        );
    }

    #[test]
    fn test_list_changes_native_sorted_by_id() {
        // The result should be sorted by change ID
        let result = list_changes_native().unwrap();
        if result.len() > 1 {
            let mut sorted = result.clone();
            sorted.sort_by(|a, b| a.id.cmp(&b.id));
            assert_eq!(result, sorted, "changes should be sorted by ID");
        }
    }

    #[test]
    fn test_list_changes_native_parses_task_counts() {
        // If there are any changes, verify they have valid task counts
        let result = list_changes_native().unwrap();
        for change in &result {
            // completed_tasks should never exceed total_tasks
            assert!(
                change.completed_tasks <= change.total_tasks,
                "completed_tasks ({}) should not exceed total_tasks ({}) for change '{}'",
                change.completed_tasks,
                change.total_tasks,
                change.id
            );
        }
    }

    #[test]
    fn test_list_changes_native_integration() {
        // Integration test: verify the function works with the actual project structure
        // This test runs in the project root where openspec/changes exists
        let result = list_changes_native();
        assert!(result.is_ok(), "list_changes_native should succeed");

        let changes = result.unwrap();
        // Verify each change has a non-empty ID
        for change in &changes {
            assert!(!change.id.is_empty(), "change ID should not be empty");
        }
    }
}
