use crate::error::{OrchestratorError, Result};
use regex::Regex;
use std::process::Command;
use tracing::{debug, info};

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

/// Execute openspec list and parse the output
pub async fn list_changes(openspec_path: &str) -> Result<Vec<Change>> {
    info!("Executing openspec list");

    let output = Command::new(openspec_path)
        .arg("list")
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

    parse_openspec_list(&stdout)
}

/// Parse openspec list output
/// Expected format (actual openspec CLI output):
///   add-openspec-orchestrator     31/35 tasks   16m ago
/// or older format:
///   - change-id (n/m tasks) [last modified time]
fn parse_openspec_list(output: &str) -> Result<Vec<Change>> {
    // Try new format first: "  change-id     n/m tasks   time ago"
    let re_new = Regex::new(r"(?m)^\s{2}(\S+)\s+(\d+)/(\d+)\s+tasks(?:\s+(.+))?$")
        .map_err(|e| OrchestratorError::Parse(format!("Invalid regex: {}", e)))?;

    // Fallback to old format: "  - change-id (n/m tasks) [time]"
    let re_old = Regex::new(r"(?m)^\s*-\s+([^\s]+)\s+\((\d+)/(\d+)\s+tasks\)\s+\[([^\]]+)\]")
        .map_err(|e| OrchestratorError::Parse(format!("Invalid regex: {}", e)))?;

    let mut changes = Vec::new();

    // Try new format
    for cap in re_new.captures_iter(output) {
        let id = cap.get(1).unwrap().as_str().to_string();
        let completed_tasks: u32 = cap
            .get(2)
            .unwrap()
            .as_str()
            .parse()
            .map_err(|e| OrchestratorError::Parse(format!("Invalid task count: {}", e)))?;
        let total_tasks: u32 = cap
            .get(3)
            .unwrap()
            .as_str()
            .parse()
            .map_err(|e| OrchestratorError::Parse(format!("Invalid task count: {}", e)))?;
        let last_modified = cap.get(4).map(|m| m.as_str().trim().to_string()).unwrap_or_default();

        changes.push(Change {
            id,
            completed_tasks,
            total_tasks,
            last_modified,
        });
    }

    // If no matches with new format, try old format
    if changes.is_empty() {
        for cap in re_old.captures_iter(output) {
            let id = cap.get(1).unwrap().as_str().to_string();
            let completed_tasks: u32 = cap
                .get(2)
                .unwrap()
                .as_str()
                .parse()
                .map_err(|e| OrchestratorError::Parse(format!("Invalid task count: {}", e)))?;
            let total_tasks: u32 = cap
                .get(3)
                .unwrap()
                .as_str()
                .parse()
                .map_err(|e| OrchestratorError::Parse(format!("Invalid task count: {}", e)))?;
            let last_modified = cap.get(4).unwrap().as_str().to_string();

            changes.push(Change {
                id,
                completed_tasks,
                total_tasks,
                last_modified,
            });
        }
    }

    Ok(changes)
}

/// Execute openspec archive for the given change
pub async fn archive_change(openspec_path: &str, change_id: &str) -> Result<()> {
    info!("Archiving change: {}", change_id);

    let output = Command::new(openspec_path)
        .arg("archive")
        .arg(change_id)
        .arg("--yes")
        .output()
        .map_err(|e| OrchestratorError::OpenSpecCommand(format!("Failed to execute: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OrchestratorError::OpenSpecCommand(format!(
            "Archive failed: {}",
            stderr
        )));
    }

    info!("Successfully archived: {}", change_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_openspec_list_new_format() {
        // Actual openspec list output format
        let output = r#"Changes:
  add-openspec-orchestrator     31/35 tasks   16m ago
  add-feature-x     2/5 tasks   2h ago
  fix-bug-y     5/5 tasks   1d ago
"#;

        let changes = parse_openspec_list(output).unwrap();
        assert_eq!(changes.len(), 3);

        assert_eq!(changes[0].id, "add-openspec-orchestrator");
        assert_eq!(changes[0].completed_tasks, 31);
        assert_eq!(changes[0].total_tasks, 35);
        assert!(!changes[0].is_complete());

        assert_eq!(changes[1].id, "add-feature-x");
        assert_eq!(changes[1].completed_tasks, 2);
        assert_eq!(changes[1].total_tasks, 5);

        assert_eq!(changes[2].id, "fix-bug-y");
        assert_eq!(changes[2].completed_tasks, 5);
        assert_eq!(changes[2].total_tasks, 5);
        assert!(changes[2].is_complete());
    }

    #[test]
    fn test_parse_openspec_list_old_format() {
        // Old format (fallback)
        let output = r#"
Changes:
  - add-feature-x (2/5 tasks) [2 hours ago]
  - fix-bug-y (5/5 tasks) [1 day ago]
  - refactor-z (0/3 tasks) [3 hours ago]
"#;

        let changes = parse_openspec_list(output).unwrap();
        assert_eq!(changes.len(), 3);

        assert_eq!(changes[0].id, "add-feature-x");
        assert_eq!(changes[0].completed_tasks, 2);
        assert_eq!(changes[0].total_tasks, 5);
        assert_eq!(changes[0].progress_percent(), 40.0);
        assert!(!changes[0].is_complete());

        assert_eq!(changes[1].id, "fix-bug-y");
        assert_eq!(changes[1].completed_tasks, 5);
        assert_eq!(changes[1].total_tasks, 5);
        assert_eq!(changes[1].progress_percent(), 100.0);
        assert!(changes[1].is_complete());

        assert_eq!(changes[2].id, "refactor-z");
        assert_eq!(changes[2].completed_tasks, 0);
        assert_eq!(changes[2].total_tasks, 3);
        assert_eq!(changes[2].progress_percent(), 0.0);
        assert!(!changes[2].is_complete());
    }

    #[test]
    fn test_parse_openspec_list_no_time() {
        // Format without time
        let output = r#"Changes:
  add-feature     0/10 tasks
"#;

        let changes = parse_openspec_list(output).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].id, "add-feature");
        assert_eq!(changes[0].completed_tasks, 0);
        assert_eq!(changes[0].total_tasks, 10);
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
}
