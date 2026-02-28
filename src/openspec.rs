use crate::error::{OrchestratorError, Result};
use crate::task_parser;
use crate::tui::log_deduplicator;
use std::fs;
use std::path::Path;
use tracing::{debug, info};

/// Represents a change from openspec list
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub id: String,
    pub completed_tasks: u32,
    pub total_tasks: u32,
    #[allow(dead_code)]
    pub last_modified: String,
    /// Dependencies on other changes (parsed from proposal.md)
    pub dependencies: Vec<String>,
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

/// Parse dependencies from a proposal.md file.
///
/// Looks for a `## Dependencies` section and extracts change IDs from bullet points.
/// Supports formats like:
/// - `- feature-base`
/// - `- [feature-base](../feature-base/proposal.md)`
/// - `- feature-base: description`
fn parse_dependencies(change_id: &str) -> Vec<String> {
    let proposal_path = Path::new("openspec/changes")
        .join(change_id)
        .join("proposal.md");

    let content = match fs::read_to_string(&proposal_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut dependencies = Vec::new();
    let mut in_deps_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for Dependencies section header
        if trimmed.starts_with("## Dependencies") {
            in_deps_section = true;
            continue;
        }

        // Exit section on next header
        if in_deps_section && trimmed.starts_with("## ") {
            break;
        }

        // Parse bullet points in Dependencies section
        if in_deps_section && trimmed.starts_with("- ") {
            let item = trimmed.trim_start_matches("- ").trim();

            // Skip "None" or empty items
            if item.is_empty() || item.eq_ignore_ascii_case("none") {
                continue;
            }

            // Extract change ID from various formats
            let dep_id = extract_dependency_id(item);
            if !dep_id.is_empty() && dep_id != change_id {
                dependencies.push(dep_id);
            }
        }
    }

    if !dependencies.is_empty() {
        info!(
            "Parsed dependencies for '{}': [{}]",
            change_id,
            dependencies.join(", ")
        );
    }
    dependencies
}

/// Extract a dependency ID from a bullet point item.
///
/// Handles formats like:
/// - `feature-base` -> `feature-base`
/// - `[feature-base](../feature-base/proposal.md)` -> `feature-base`
/// - `feature-base: some description` -> `feature-base`
/// - `feature-base (optional)` -> `feature-base`
fn extract_dependency_id(item: &str) -> String {
    // Handle markdown link format: [id](path)
    if item.starts_with('[') {
        if let Some(end) = item.find(']') {
            return item[1..end].trim().to_string();
        }
    }

    // Handle inline code format: `id`
    if let Some(stripped) = item.strip_prefix('`') {
        if let Some(end) = stripped.find('`') {
            return stripped[..end].trim().to_string();
        }
    }

    // Handle plain text with optional suffix (colon, parenthesis)
    // Note: Don't split on '-' as it's common in change IDs like "feature-base"
    item.split(&[':', '('][..])
        .next()
        .unwrap_or(item)
        .trim()
        .to_string()
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

        // Skip changes without proposal.md
        let proposal_path = path.join("proposal.md");
        if !proposal_path.exists() {
            debug!("Skipping change '{}' - no proposal.md found", dir_name);
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

        // Parse dependencies from proposal.md
        let dependencies = parse_dependencies(dir_name);

        changes.push(Change {
            id: dir_name.to_string(),
            completed_tasks,
            total_tasks,
            last_modified: String::new(), // Not used in native implementation
            dependencies,
        });
    }

    // Sort by id for consistent ordering
    changes.sort_by(|a, b| a.id.cmp(&b.id));

    if log_deduplicator::should_log_change_count(changes.len()) {
        debug!("Found {} changes via native parsing", changes.len());
    }
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoggingConfig;
    use crate::tui::log_deduplicator;
    use std::env;
    use std::fs;
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    static LOG_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn test_change_progress() {
        let change = Change {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 0,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
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
            dependencies: Vec::new(),
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

    #[test]
    fn test_list_changes_native_excludes_without_proposal() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        // Test that changes without proposal.md are excluded
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec").join("changes");
        fs::create_dir_all(&changes_dir).unwrap();

        // Create change-a WITH proposal.md
        let change_a_dir = changes_dir.join("change-a");
        fs::create_dir_all(&change_a_dir).unwrap();
        fs::File::create(change_a_dir.join("proposal.md")).unwrap();
        let mut tasks_a = fs::File::create(change_a_dir.join("tasks.md")).unwrap();
        writeln!(tasks_a, "- [x] Task 1\n- [ ] Task 2").unwrap();

        // Create change-b WITHOUT proposal.md
        let change_b_dir = changes_dir.join("change-b");
        fs::create_dir_all(&change_b_dir).unwrap();
        let mut tasks_b = fs::File::create(change_b_dir.join("tasks.md")).unwrap();
        writeln!(tasks_b, "- [ ] Task 1").unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let result = list_changes_native().unwrap();

        env::set_current_dir(original_dir).unwrap();

        // Should only include change-a (has proposal.md), not change-b (no proposal.md)
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "change-a");
        assert_eq!(result[0].completed_tasks, 1);
        assert_eq!(result[0].total_tasks, 2);
    }

    #[test]
    fn test_list_changes_native_suppresses_repetitive_logs() {
        let _cwd_lock = crate::test_support::cwd_lock().lock().unwrap();
        let log_lock = LOG_TEST_MUTEX.get_or_init(|| Mutex::new(()));
        let _guard = log_lock.lock().expect("log mutex poisoned");

        let temp_dir = TempDir::new().unwrap();
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("sample-change");
        fs::create_dir_all(&change_dir).unwrap();
        // Create proposal.md (required for change to be included)
        fs::File::create(change_dir.join("proposal.md")).unwrap();
        let mut tasks_file = fs::File::create(change_dir.join("tasks.md")).unwrap();
        writeln!(tasks_file, "- [ ] Task 1").unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        log_deduplicator::configure_logging(LoggingConfig {
            suppress_repetitive_debug: true,
            summary_interval_secs: 0,
        });

        let _ = list_changes_native();
        let _ = list_changes_native();

        let should_log_progress = log_deduplicator::should_log_task_progress("sample-change", 0, 1);
        let should_log_count = log_deduplicator::should_log_change_count(1);

        env::set_current_dir(original_dir).unwrap();

        assert!(!should_log_progress);
        assert!(!should_log_count);
    }
}
