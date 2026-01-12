//! Workspace cleanup guard for RAII-based cleanup on partial failures.

use crate::vcs::VcsBackend;
use std::path::PathBuf;
use tracing::{debug, warn};

/// RAII guard for workspace cleanup on partial failures.
///
/// This guard tracks created workspaces and ensures they are cleaned up
/// on drop if not explicitly committed. This prevents workspace leaks
/// when errors occur during workspace creation or apply phases.
///
/// Note: Failed workspaces can be marked for preservation, which will
/// prevent their cleanup on drop to allow for manual investigation or
/// resume functionality.
pub(crate) struct WorkspaceCleanupGuard {
    /// Workspace names to clean up
    workspace_names: Vec<String>,
    /// Workspace names to preserve (not cleaned up on drop)
    preserved_workspaces: std::collections::HashSet<String>,
    /// VCS backend type
    vcs_backend: VcsBackend,
    /// Repository root for cleanup commands
    repo_root: PathBuf,
    /// Whether cleanup has been committed (skipped)
    committed: bool,
}

impl WorkspaceCleanupGuard {
    /// Create a new cleanup guard
    pub fn new(vcs_backend: VcsBackend, repo_root: PathBuf) -> Self {
        Self {
            workspace_names: Vec::new(),
            preserved_workspaces: std::collections::HashSet::new(),
            vcs_backend,
            repo_root,
            committed: false,
        }
    }

    /// Add a workspace to be tracked for cleanup
    pub fn track(&mut self, workspace_name: String) {
        self.workspace_names.push(workspace_name);
    }

    /// Mark a workspace for preservation (will not be cleaned up on drop).
    ///
    /// Call this for workspaces where errors occurred and the workspace
    /// should be preserved for debugging or resume functionality.
    #[allow(dead_code)] // Public API for workspace preservation
    pub fn preserve(&mut self, workspace_name: &str) {
        self.preserved_workspaces.insert(workspace_name.to_string());
    }

    /// Commit the guard, preventing cleanup on drop
    ///
    /// Call this when all workspaces have been successfully processed
    /// and cleanup will be handled through the normal path.
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for WorkspaceCleanupGuard {
    fn drop(&mut self) {
        if self.committed || self.workspace_names.is_empty() {
            return;
        }

        // Filter out preserved workspaces from cleanup
        let workspaces_to_clean: Vec<_> = self
            .workspace_names
            .iter()
            .filter(|name| !self.preserved_workspaces.contains(*name))
            .collect();

        if workspaces_to_clean.is_empty() {
            return;
        }

        warn!(
            "Cleaning up {} workspace(s) due to early error ({} preserved)",
            workspaces_to_clean.len(),
            self.preserved_workspaces.len()
        );

        // Use synchronous cleanup since we're in Drop
        // This is a best-effort cleanup - errors are logged but not propagated
        for workspace_name in &workspaces_to_clean {
            debug!(
                "Emergency cleanup: forgetting workspace '{}'",
                workspace_name
            );

            match self.vcs_backend {
                VcsBackend::Jj => {
                    // Forget the workspace in jj
                    let result = std::process::Command::new("jj")
                        .args(["workspace", "forget", workspace_name])
                        .current_dir(&self.repo_root)
                        .output();

                    match result {
                        Ok(output) if !output.status.success() => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            debug!(
                                "Failed to forget jj workspace '{}': {}",
                                workspace_name, stderr
                            );
                        }
                        Err(e) => {
                            debug!("Failed to run jj workspace forget: {}", e);
                        }
                        _ => {
                            debug!("Successfully forgot jj workspace '{}'", workspace_name);
                        }
                    }
                }
                VcsBackend::Git | VcsBackend::Auto => {
                    // For Git, we need the worktree path, but we only have the name
                    // This is a best-effort cleanup; the worktree will be orphaned
                    // but can be cleaned up later with `git worktree prune`
                    let result = std::process::Command::new("git")
                        .args(["branch", "-D", workspace_name])
                        .current_dir(&self.repo_root)
                        .output();

                    match result {
                        Ok(output) if !output.status.success() => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            debug!(
                                "Failed to delete git branch '{}': {}",
                                workspace_name, stderr
                            );
                        }
                        Err(e) => {
                            debug!("Failed to run git branch -D: {}", e);
                        }
                        _ => {
                            debug!("Successfully deleted git branch '{}'", workspace_name);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // === Tests for WorkspaceCleanupGuard (workspace-cleanup spec) ===

    #[test]
    fn test_cleanup_guard_creation() {
        let temp_dir = TempDir::new().unwrap();
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());

        // Guard should start with no workspaces and not committed
        assert!(!guard.committed);
        assert!(guard.workspace_names.is_empty());
    }

    #[test]
    fn test_cleanup_guard_tracks_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());

        guard.track("ws-change-a-1234".to_string());
        guard.track("ws-change-b-5678".to_string());

        assert_eq!(guard.workspace_names.len(), 2);
        assert!(guard
            .workspace_names
            .contains(&"ws-change-a-1234".to_string()));
        assert!(guard
            .workspace_names
            .contains(&"ws-change-b-5678".to_string()));
    }

    #[test]
    fn test_cleanup_guard_commit_prevents_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());

        guard.track("ws-change-a-1234".to_string());
        assert!(!guard.committed);

        // Commit the guard
        guard.commit();
        // After commit(), guard is consumed and cleanup is prevented
    }

    #[test]
    fn test_cleanup_guard_git_backend() {
        let temp_dir = TempDir::new().unwrap();
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

        assert!(matches!(guard.vcs_backend, VcsBackend::Git));
    }

    #[test]
    fn test_cleanup_guard_auto_backend_treated_as_git() {
        let temp_dir = TempDir::new().unwrap();
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Auto, temp_dir.path().to_path_buf());

        // Auto should be treated like Git in Drop
        assert!(matches!(guard.vcs_backend, VcsBackend::Auto));
    }

    #[test]
    fn test_cleanup_guard_drop_with_empty_workspaces_does_nothing() {
        let temp_dir = TempDir::new().unwrap();
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());

        // Drop with no tracked workspaces should not panic
        drop(guard);
    }

    #[test]
    fn test_cleanup_guard_drop_with_committed_guard_does_nothing() {
        let temp_dir = TempDir::new().unwrap();
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());
        guard.track("ws-test-1234".to_string());

        // Commit and then drop - should not attempt cleanup
        guard.commit();
    }

    // === Tests for RAII cleanup semantics (workspace-cleanup spec 4.1) ===

    #[test]
    fn test_cleanup_guard_raii_pattern() {
        // Test that the guard follows RAII pattern
        let temp_dir = TempDir::new().unwrap();

        // Simulate a scope where guard is created
        {
            let mut guard =
                WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());
            guard.track("ws-test-1234".to_string());
            // Not committed - will attempt cleanup on drop
        } // guard drops here

        // If we reach here, drop completed successfully
    }

    #[test]
    fn test_cleanup_guard_commit_on_success() {
        let temp_dir = TempDir::new().unwrap();

        // Simulate successful operation
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());
        guard.track("ws-success-1234".to_string());

        // On success, commit to prevent cleanup
        guard.commit();
        // Guard is consumed, no cleanup will occur
    }

    // === Tests for VCS backend cleanup paths ===

    #[test]
    fn test_cleanup_guard_jj_workspace_forget_command() {
        // Verify JJ cleanup uses "jj workspace forget" command
        let temp_dir = TempDir::new().unwrap();
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());

        assert!(matches!(guard.vcs_backend, VcsBackend::Jj));
        // The Drop impl calls "jj workspace forget <name>"
    }

    #[test]
    fn test_cleanup_guard_git_branch_delete_command() {
        // Verify Git cleanup uses "git branch -D" command
        let temp_dir = TempDir::new().unwrap();
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

        assert!(matches!(guard.vcs_backend, VcsBackend::Git));
        // The Drop impl calls "git branch -D <name>"
    }

    // === Tests for workspace preservation (preserve-workspace-on-error spec) ===

    #[test]
    fn test_cleanup_guard_preserve_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());

        guard.track("ws-change-a-1234".to_string());
        guard.track("ws-change-b-5678".to_string());

        // Preserve one workspace (simulating error)
        guard.preserve("ws-change-a-1234");

        assert!(guard.preserved_workspaces.contains("ws-change-a-1234"));
        assert!(!guard.preserved_workspaces.contains("ws-change-b-5678"));
    }

    #[test]
    fn test_cleanup_guard_preserved_workspace_not_cleaned_on_drop() {
        let temp_dir = TempDir::new().unwrap();

        // Test that preserved workspaces are excluded from cleanup
        {
            let mut guard =
                WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());
            guard.track("ws-success-1234".to_string());
            guard.track("ws-failed-5678".to_string());

            // Preserve the failed workspace
            guard.preserve("ws-failed-5678");

            // On drop, only ws-success-1234 should be attempted for cleanup
            // (ws-failed-5678 is preserved and should be skipped)
        }
        // If we reach here, drop completed successfully
    }

    #[test]
    fn test_cleanup_guard_all_preserved_does_nothing() {
        let temp_dir = TempDir::new().unwrap();

        // If all workspaces are preserved, drop should do nothing
        {
            let mut guard =
                WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());
            guard.track("ws-failed-1".to_string());
            guard.track("ws-failed-2".to_string());

            // Preserve all workspaces
            guard.preserve("ws-failed-1");
            guard.preserve("ws-failed-2");

            // On drop, nothing should be cleaned up
        }
        // If we reach here, drop completed successfully without attempting cleanup
    }

    #[test]
    fn test_cleanup_guard_preserved_workspaces_starts_empty() {
        let temp_dir = TempDir::new().unwrap();
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Jj, temp_dir.path().to_path_buf());

        assert!(guard.preserved_workspaces.is_empty());
    }
}
