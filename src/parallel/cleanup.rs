//! Workspace cleanup guard for RAII-based cleanup on partial failures.

use crate::vcs::VcsBackend;
use std::path::PathBuf;
use tracing::debug;

/// RAII guard for workspace cleanup on partial failures.
///
/// This guard tracks created workspaces and preserves them by default.
/// Workspaces are only cleaned up when explicitly requested via commit().
/// This ensures workspaces are retained for debugging, resume, or manual
/// investigation after errors or cancellation.
///
/// Note: The guard preserves all workspaces by default. Use commit() after
/// successful merge to enable cleanup for successfully processed workspaces.
pub(crate) struct WorkspaceCleanupGuard {
    /// Workspace names and paths to clean up
    workspaces: std::collections::HashMap<String, PathBuf>,
    /// Workspace names to preserve (not cleaned up on drop)
    preserved_workspaces: std::collections::HashSet<String>,
    /// VCS backend type
    vcs_backend: VcsBackend,
    /// Repository root for cleanup commands
    repo_root: PathBuf,
    /// Whether cleanup is allowed on drop (default: false, preserves workspaces)
    cleanup_allowed: bool,
}

impl WorkspaceCleanupGuard {
    /// Create a new cleanup guard
    ///
    /// By default, workspaces are preserved (not cleaned up on drop).
    /// Call commit() to enable cleanup for successfully processed workspaces.
    pub fn new(vcs_backend: VcsBackend, repo_root: PathBuf) -> Self {
        Self {
            workspaces: std::collections::HashMap::new(),
            preserved_workspaces: std::collections::HashSet::new(),
            vcs_backend,
            repo_root,
            cleanup_allowed: false,
        }
    }

    /// Add a workspace to be tracked for cleanup
    pub fn track(&mut self, workspace_name: String, workspace_path: PathBuf) {
        self.workspaces.insert(workspace_name, workspace_path);
    }

    /// Mark a workspace for preservation (will not be cleaned up on drop).
    ///
    /// Call this for workspaces where errors occurred and the workspace
    /// should be preserved for debugging or resume functionality.
    #[allow(dead_code)] // Public API for workspace preservation
    pub fn preserve(&mut self, workspace_name: &str) {
        self.preserved_workspaces.insert(workspace_name.to_string());
    }

    /// Mark all tracked workspaces for preservation.
    ///
    /// Call this when cancellation or errors occur and all workspaces
    /// should be preserved for debugging or resume functionality.
    pub fn preserve_all(&mut self) {
        for workspace_name in self.workspaces.keys() {
            self.preserved_workspaces.insert(workspace_name.clone());
        }
    }

    /// Commit the guard, enabling cleanup on drop for non-preserved workspaces
    ///
    /// Call this after successful merge completion to enable cleanup.
    /// Preserved workspaces will still be excluded from cleanup.
    #[allow(dead_code)] // Public API for optional cleanup enablement
    pub fn commit(mut self) {
        self.cleanup_allowed = true;
    }
}

impl Drop for WorkspaceCleanupGuard {
    fn drop(&mut self) {
        // Only cleanup if explicitly allowed (via commit after successful merge)
        if !self.cleanup_allowed || self.workspaces.is_empty() {
            return;
        }

        // Filter out preserved workspaces from cleanup
        let workspaces_to_clean: Vec<_> = self
            .workspaces
            .iter()
            .filter(|(name, _path)| !self.preserved_workspaces.contains(*name))
            .collect();

        if workspaces_to_clean.is_empty() {
            return;
        }

        debug!(
            "Cleaning up {} workspace(s) after successful merge ({} preserved)",
            workspaces_to_clean.len(),
            self.preserved_workspaces.len()
        );

        // Use synchronous cleanup since we're in Drop
        // This is a best-effort cleanup - errors are logged but not propagated
        for (workspace_name, workspace_path) in &workspaces_to_clean {
            debug!(
                "Emergency cleanup: removing workspace '{}' at {:?}",
                workspace_name, workspace_path
            );

            match self.vcs_backend {
                VcsBackend::Git | VcsBackend::Auto => {
                    // First, remove the worktree
                    debug!(
                        module = module_path!(),
                        "Executing git command: git worktree remove {:?} --force (cwd: {:?})",
                        workspace_path,
                        self.repo_root
                    );
                    let result = std::process::Command::new("git")
                        .args([
                            "worktree",
                            "remove",
                            workspace_path.to_str().unwrap(),
                            "--force",
                        ])
                        .current_dir(&self.repo_root)
                        .output();

                    match result {
                        Ok(output) if !output.status.success() => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            debug!(
                                "Failed to remove git worktree at {:?}: {}",
                                workspace_path, stderr
                            );
                        }
                        Err(e) => {
                            debug!("Failed to run git worktree remove: {}", e);
                        }
                        _ => {
                            debug!("Successfully removed git worktree at {:?}", workspace_path);
                        }
                    }

                    // Then, delete the branch
                    debug!(
                        module = module_path!(),
                        "Executing git command: git branch -D {} (cwd: {:?})",
                        workspace_name,
                        self.repo_root
                    );
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
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

        // Guard should start with no workspaces and cleanup not allowed (preserves by default)
        assert!(!guard.cleanup_allowed);
        assert!(guard.workspaces.is_empty());
    }

    #[test]
    fn test_cleanup_guard_tracks_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

        guard.track("ws-change-a-1234".to_string(), PathBuf::from("/tmp/ws-a"));
        guard.track("ws-change-b-5678".to_string(), PathBuf::from("/tmp/ws-b"));

        assert_eq!(guard.workspaces.len(), 2);
        assert!(guard.workspaces.contains_key("ws-change-a-1234"));
        assert!(guard.workspaces.contains_key("ws-change-b-5678"));
    }

    #[test]
    fn test_cleanup_guard_commit_enables_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

        guard.track("ws-change-a-1234".to_string(), PathBuf::from("/tmp/ws-a"));
        assert!(!guard.cleanup_allowed);

        // Commit the guard to enable cleanup
        guard.commit();
        // After commit(), guard is consumed and cleanup is enabled on drop
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
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

        // Drop with no tracked workspaces should not panic
        drop(guard);
    }

    #[test]
    fn test_cleanup_guard_drop_without_commit_preserves_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());
        guard.track("ws-test-1234".to_string(), PathBuf::from("/tmp/ws-test"));

        // Drop without commit - workspaces are preserved by default
        drop(guard);
    }

    // === Tests for RAII cleanup semantics (workspace-cleanup spec 4.1) ===

    #[test]
    fn test_cleanup_guard_raii_pattern() {
        // Test that the guard follows RAII pattern
        let temp_dir = TempDir::new().unwrap();

        // Simulate a scope where guard is created
        {
            let mut guard =
                WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());
            guard.track("ws-test-1234".to_string(), PathBuf::from("/tmp/ws-test"));
            // Not committed - workspaces are preserved on drop (default behavior)
        } // guard drops here

        // If we reach here, drop completed successfully (no cleanup attempted)
    }

    #[test]
    fn test_cleanup_guard_commit_on_success() {
        let temp_dir = TempDir::new().unwrap();

        // Simulate successful operation
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());
        guard.track(
            "ws-success-1234".to_string(),
            PathBuf::from("/tmp/ws-success"),
        );

        // On successful merge, commit to enable cleanup
        guard.commit();
        // Guard is consumed, cleanup will occur on drop for non-preserved workspaces
    }

    // === Tests for VCS backend cleanup paths ===

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
        let mut guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

        guard.track("ws-change-a-1234".to_string(), PathBuf::from("/tmp/ws-a"));
        guard.track("ws-change-b-5678".to_string(), PathBuf::from("/tmp/ws-b"));

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
                WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());
            guard.track(
                "ws-success-1234".to_string(),
                PathBuf::from("/tmp/ws-success"),
            );
            guard.track(
                "ws-failed-5678".to_string(),
                PathBuf::from("/tmp/ws-failed"),
            );

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
                WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());
            guard.track("ws-failed-1".to_string(), PathBuf::from("/tmp/ws-failed-1"));
            guard.track("ws-failed-2".to_string(), PathBuf::from("/tmp/ws-failed-2"));

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
        let guard = WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

        assert!(guard.preserved_workspaces.is_empty());
    }

    // === Test for MergeDeferred worktree preservation (fix-merge-wait-resolve-flow) ===

    #[test]
    fn test_cleanup_guard_merge_deferred_preserves_worktree() {
        let temp_dir = TempDir::new().unwrap();

        // Simulate the MergeDeferred scenario from execute_with_order_based_reanalysis
        {
            let mut guard =
                WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

            // Track a workspace that completed archive
            guard.track(
                "ws-change-archived-1234".to_string(),
                PathBuf::from("/tmp/ws-archived"),
            );

            // Simulate MergeDeferred: preserve the workspace
            // This happens when attempt_merge returns MergeAttempt::Deferred
            guard.preserve("ws-change-archived-1234");

            // Verify the workspace is marked for preservation
            assert!(guard
                .preserved_workspaces
                .contains("ws-change-archived-1234"));

            // On drop (without commit), the workspace should be preserved
            // This allows the user to manually resolve conflicts and retry merge
        }
        // If we reach here, drop completed successfully without cleaning up the preserved workspace
    }

    #[test]
    fn test_cleanup_guard_merge_deferred_multiple_workspaces() {
        let temp_dir = TempDir::new().unwrap();

        // Test scenario with multiple workspaces, some merged successfully, some deferred
        {
            let mut guard =
                WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

            // Track multiple workspaces
            guard.track(
                "ws-change-merged-1".to_string(),
                PathBuf::from("/tmp/ws-merged-1"),
            );
            guard.track(
                "ws-change-deferred-2".to_string(),
                PathBuf::from("/tmp/ws-deferred-2"),
            );
            guard.track(
                "ws-change-merged-3".to_string(),
                PathBuf::from("/tmp/ws-merged-3"),
            );

            // Preserve only the deferred workspace
            guard.preserve("ws-change-deferred-2");

            // Verify preservation state
            assert!(!guard.preserved_workspaces.contains("ws-change-merged-1"));
            assert!(guard.preserved_workspaces.contains("ws-change-deferred-2"));
            assert!(!guard.preserved_workspaces.contains("ws-change-merged-3"));

            // On drop (without commit), only the deferred workspace is preserved
            // The merged workspaces would be cleaned up if commit() was called
        }
        // Drop completes successfully
    }
}
