//! Common types for parallel execution.

use std::collections::{HashMap, HashSet};

/// Result of a workspace execution (VCS-agnostic)
#[derive(Debug, Clone)]
pub struct WorkspaceResult {
    /// OpenSpec change ID
    pub change_id: String,
    /// Workspace name
    pub workspace_name: String,
    /// Final revision if successful
    pub final_revision: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Tracks failed changes and their dependencies to enable automatic skipping.
///
/// When a change fails, any changes that depend on it should be skipped
/// since they are unlikely to succeed without the dependency.
#[derive(Debug, Default)]
pub struct FailedChangeTracker {
    /// Set of failed change IDs
    failed_changes: HashSet<String>,
    /// Dependencies between changes (change_id -> list of dependencies)
    dependencies: HashMap<String, Vec<String>>,
}

impl FailedChangeTracker {
    /// Create a new empty tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the dependencies for all changes.
    ///
    /// The dependencies map should contain change_id -> [dependency_ids].
    pub fn set_dependencies(&mut self, dependencies: HashMap<String, Vec<String>>) {
        self.dependencies = dependencies;
    }

    /// Mark a change as failed
    pub fn mark_failed(&mut self, change_id: &str) {
        self.failed_changes.insert(change_id.to_string());
    }

    /// Check if a change should be skipped due to a failed dependency.
    ///
    /// Returns `Some(failed_dep_id)` if the change depends on a failed change,
    /// otherwise returns `None`.
    pub fn should_skip(&self, change_id: &str) -> Option<String> {
        if let Some(deps) = self.dependencies.get(change_id) {
            for dep in deps {
                if self.failed_changes.contains(dep) {
                    return Some(dep.clone());
                }
            }
        }
        None
    }

    /// Get all failed changes
    #[allow(dead_code)] // Public API for external callers
    pub fn failed_changes(&self) -> &HashSet<String> {
        &self.failed_changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failed_tracker_new() {
        let tracker = FailedChangeTracker::new();
        assert!(tracker.failed_changes.is_empty());
        assert!(tracker.dependencies.is_empty());
    }

    #[test]
    fn test_mark_failed() {
        let mut tracker = FailedChangeTracker::new();
        tracker.mark_failed("change-a");
        assert!(tracker.failed_changes.contains("change-a"));
    }

    #[test]
    fn test_should_skip_no_dependencies() {
        let tracker = FailedChangeTracker::new();
        assert!(tracker.should_skip("change-a").is_none());
    }

    #[test]
    fn test_should_skip_with_failed_dependency() {
        let mut tracker = FailedChangeTracker::new();

        // Set up: change-b depends on change-a
        let mut deps = HashMap::new();
        deps.insert("change-b".to_string(), vec!["change-a".to_string()]);
        tracker.set_dependencies(deps);

        // change-a fails
        tracker.mark_failed("change-a");

        // change-b should be skipped
        let result = tracker.should_skip("change-b");
        assert_eq!(result, Some("change-a".to_string()));
    }

    #[test]
    fn test_should_skip_no_failed_dependency() {
        let mut tracker = FailedChangeTracker::new();

        // Set up: change-b depends on change-a
        let mut deps = HashMap::new();
        deps.insert("change-b".to_string(), vec!["change-a".to_string()]);
        tracker.set_dependencies(deps);

        // change-a did NOT fail
        // change-b should NOT be skipped
        assert!(tracker.should_skip("change-b").is_none());
    }

    #[test]
    fn test_should_skip_with_multiple_dependencies() {
        let mut tracker = FailedChangeTracker::new();

        // Set up: change-c depends on change-a and change-b
        let mut deps = HashMap::new();
        deps.insert(
            "change-c".to_string(),
            vec!["change-a".to_string(), "change-b".to_string()],
        );
        tracker.set_dependencies(deps);

        // Only change-b fails
        tracker.mark_failed("change-b");

        // change-c should be skipped (returns first failed dep found)
        let result = tracker.should_skip("change-c");
        assert_eq!(result, Some("change-b".to_string()));
    }
}
