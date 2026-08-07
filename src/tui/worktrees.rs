//! Worktree helper functions for TUI
//!
//! This module contains helper functions for worktree operations, extracted from runner.rs
//! to eliminate circular dependencies.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::worktree_ops::ObservationRequest;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Check if worktree command should be triggered based on config and git repo status
pub fn should_trigger_worktree_command(config: &OrchestratorConfig, is_git_repo: bool) -> bool {
    config.get_worktree_command().is_some() && is_git_repo
}

/// Build a worktree path with timestamp-based unique name
pub fn build_worktree_path(base_dir: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();
    base_dir.join(format!("proposal-{}", timestamp))
}

/// Load worktrees and check for merge conflicts in parallel.
///
/// This is the TUI's periodic refresh entry point. It owns no policy of its
/// own: eligibility, the revision-keyed observation cache, and the bounded
/// diagnostics all live in [`crate::worktree_ops`], which the Web/UDS periodic
/// refresh calls with the same [`ObservationRequest::Periodic`]. That is what
/// makes "one merge simulation per revision tuple" true across both frontends
/// instead of once per frontend.
pub async fn load_worktrees_with_conflict_check(
    repo_root: &Path,
) -> Result<Vec<super::types::WorktreeInfo>> {
    crate::worktree_ops::get_worktrees(repo_root, ObservationRequest::Periodic).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrchestratorConfig;

    #[test]
    fn test_should_trigger_worktree_command_missing_config() {
        let config = OrchestratorConfig::default();
        assert!(!should_trigger_worktree_command(&config, true));
    }

    #[test]
    fn test_should_trigger_worktree_command_not_git_repo() {
        let config = OrchestratorConfig {
            worktree_command: Some("cmd {workspace_dir}".to_string()),
            ..Default::default()
        };
        assert!(!should_trigger_worktree_command(&config, false));
    }

    #[test]
    fn test_should_trigger_worktree_command_enabled() {
        let config = OrchestratorConfig {
            worktree_command: Some("cmd {repo_root}".to_string()),
            ..Default::default()
        };
        assert!(should_trigger_worktree_command(&config, true));
    }
}
