//! Worktree helper functions for TUI
//!
//! This module contains helper functions for worktree operations, extracted from runner.rs
//! to eliminate circular dependencies.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

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

/// Load worktrees and check for merge conflicts in parallel
pub async fn load_worktrees_with_conflict_check(
    repo_root: &Path,
) -> Result<Vec<super::types::WorktreeInfo>> {
    use super::types::{MergeConflictInfo, WorktreeInfo};

    // First, get the list of worktrees
    let worktrees_data = crate::vcs::git::commands::list_worktrees(repo_root).await?;

    // Convert to WorktreeInfo structs
    let mut worktrees: Vec<WorktreeInfo> = worktrees_data
        .into_iter()
        .map(|(path, head, branch, is_detached, is_main)| WorktreeInfo {
            path: PathBuf::from(path),
            head,
            branch: branch.clone(),
            is_detached,
            is_main,
            merge_conflict: None,
            has_commits_ahead: false, // Will be populated later in parallel check
            is_merging: false,
        })
        .collect();

    // Get the base branch name from the main worktree
    let _base_branch = if let Some(main_wt) = worktrees.iter().find(|wt| wt.is_main) {
        main_wt.branch.clone()
    } else {
        // Fallback: get current branch from repo root
        match crate::vcs::git::commands::get_current_branch(repo_root).await {
            Ok(Some(branch)) => branch,
            Ok(None) | Err(_) => {
                // If we can't get the base branch (detached HEAD or error), skip conflict checking
                return Ok(worktrees);
            }
        }
    };

    // Check conflicts and commits ahead in parallel for non-main, non-detached worktrees
    let mut tasks = tokio::task::JoinSet::new();

    for (idx, worktree) in worktrees.iter().enumerate() {
        // Skip main worktree and detached HEADs
        if worktree.is_main || worktree.is_detached || worktree.branch.is_empty() {
            continue;
        }

        let wt_path = worktree.path.clone();
        let branch_name = worktree.branch.clone();
        let base_branch = _base_branch.clone();

        tasks.spawn(async move {
            // Check merge conflicts
            let conflict_result =
                crate::vcs::git::commands::check_merge_conflicts(&wt_path, &base_branch).await;

            // Check commits ahead
            let ahead_result = crate::vcs::git::commands::count_commits_ahead(
                &wt_path,
                &base_branch,
                &branch_name,
            )
            .await;

            (idx, conflict_result, ahead_result)
        });
    }

    // Collect results
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((idx, conflict_result, ahead_result)) => {
                // Process conflict check result
                match conflict_result {
                    Ok(conflict_files_opt) => {
                        if let Some(conflict_files) = conflict_files_opt {
                            // Conflicts detected
                            worktrees[idx].merge_conflict =
                                Some(MergeConflictInfo { conflict_files });
                        } else {
                            // No conflicts
                            worktrees[idx].merge_conflict = None;
                        }
                    }
                    Err(e) => {
                        // Check failed - treat as unknown (no conflict info)
                        debug!(
                            "Conflict check failed for worktree {}: {}",
                            worktrees[idx].path.display(),
                            e
                        );
                        worktrees[idx].merge_conflict = None;
                    }
                }

                // Process commits ahead check result
                match ahead_result {
                    Ok(count) => {
                        worktrees[idx].has_commits_ahead = count > 0;
                        debug!(
                            "Worktree {} has {} commits ahead of base",
                            worktrees[idx].path.display(),
                            count
                        );
                    }
                    Err(e) => {
                        // Check failed - treat as no commits ahead (safe default)
                        debug!(
                            "Commits ahead check failed for worktree {}: {}",
                            worktrees[idx].path.display(),
                            e
                        );
                        worktrees[idx].has_commits_ahead = false;
                    }
                }
            }
            Err(e) => {
                // Join error
                warn!("Worktree check task panicked: {}", e);
            }
        }
    }

    Ok(worktrees)
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
