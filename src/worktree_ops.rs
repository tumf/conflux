//! Common worktree operations shared between TUI and Web API.
//!
//! This module provides shared logic for worktree retrieval, validation, and guard checks
//! to ensure consistency between TUI and Web interfaces.

use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Load all worktrees with parallel conflict checking and commits ahead detection.
///
/// This is the canonical worktree retrieval function used by both TUI and Web API
/// to ensure consistent worktree state across interfaces.
pub async fn get_worktrees(
    repo_root: &Path,
) -> crate::error::Result<Vec<crate::tui::types::WorktreeInfo>> {
    // Get the list of worktrees
    let worktrees_data = crate::vcs::git::commands::list_worktrees(repo_root).await?;

    // Convert to WorktreeInfo structs
    let mut worktrees: Vec<crate::tui::types::WorktreeInfo> = worktrees_data
        .into_iter()
        .map(
            |(path, head, branch, is_detached, is_main)| crate::tui::types::WorktreeInfo {
                path: PathBuf::from(path),
                head,
                branch: branch.clone(),
                is_detached,
                is_main,
                merge_conflict: None,
                has_commits_ahead: false,
                is_merging: false,
            },
        )
        .collect();

    // Get the base branch name from the main worktree
    let base_branch = if let Some(main_wt) = worktrees.iter().find(|wt| wt.is_main) {
        main_wt.branch.clone()
    } else {
        // Fallback: get current branch from repo root
        match crate::vcs::git::commands::get_current_branch(repo_root).await? {
            Some(branch) => branch,
            None => {
                // Detached HEAD or error - skip enrichment
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
        let base_branch_clone = base_branch.clone();

        tasks.spawn(async move {
            // Check merge conflicts
            let conflict_result =
                crate::vcs::git::commands::check_merge_conflicts(&wt_path, &base_branch_clone)
                    .await;

            // Check commits ahead
            let ahead_result = crate::vcs::git::commands::count_commits_ahead(
                &wt_path,
                &base_branch_clone,
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
                            worktrees[idx].merge_conflict =
                                Some(crate::tui::types::MergeConflictInfo { conflict_files });
                        }
                    }
                    Err(e) => {
                        debug!(
                            "Conflict check failed for worktree {}: {}",
                            worktrees[idx].path.display(),
                            e
                        );
                    }
                }

                // Process commits ahead check result
                match ahead_result {
                    Ok(count) => {
                        worktrees[idx].has_commits_ahead = count > 0;
                    }
                    Err(e) => {
                        debug!(
                            "Commits ahead check failed for worktree {}: {}",
                            worktrees[idx].path.display(),
                            e
                        );
                    }
                }
            }
            Err(e) => {
                warn!("Worktree check task panicked: {}", e);
            }
        }
    }

    Ok(worktrees)
}

/// Guard: Check if a worktree can be safely deleted.
///
/// A worktree can be deleted if:
/// - It is not the main worktree
/// - It has no uncommitted changes
/// - It has no unmerged commits (not ahead of base branch)
///
/// Returns (can_delete, reason_if_not)
pub async fn can_delete_worktree(
    worktree: &crate::tui::types::WorktreeInfo,
) -> (bool, Option<String>) {
    if worktree.is_main {
        return (false, Some("Cannot delete main worktree".to_string()));
    }

    // Check if worktree has uncommitted changes
    match crate::vcs::git::commands::has_uncommitted_changes(&worktree.path).await {
        Ok((has_changes, _)) if has_changes => {
            return (false, Some("Worktree has uncommitted changes".to_string()));
        }
        Err(e) => {
            warn!("Failed to check uncommitted changes: {}", e);
            // Allow deletion on error (fail-open for cleanup)
        }
        _ => {}
    }

    // Check if worktree has unmerged commits ahead of base
    if worktree.has_commits_ahead {
        return (
            false,
            Some("Worktree has unmerged commits ahead of base branch".to_string()),
        );
    }

    (true, None)
}

/// Guard: Check if a worktree can be safely merged.
///
/// A worktree can be merged if:
/// - It is not the main worktree
/// - It has no merge conflicts
/// - It has commits ahead of the base branch
///
/// Returns (can_merge, reason_if_not)
pub fn can_merge_worktree(worktree: &crate::tui::types::WorktreeInfo) -> (bool, Option<String>) {
    if worktree.is_main {
        return (false, Some("Cannot merge main worktree".to_string()));
    }

    if worktree.merge_conflict.is_some() {
        return (false, Some("Worktree has merge conflicts".to_string()));
    }

    if !worktree.has_commits_ahead {
        return (
            false,
            Some("Worktree has no commits ahead of base".to_string()),
        );
    }

    (true, None)
}

/// Check if a worktree exists by branch name.
///
/// Returns true if a worktree with the given branch exists.
pub async fn worktree_exists(repo_root: &Path, branch_name: &str) -> crate::error::Result<bool> {
    let worktrees = get_worktrees(repo_root).await?;
    Ok(worktrees.iter().any(|wt| wt.branch == branch_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_can_delete_main_worktree() {
        let worktree = crate::tui::types::WorktreeInfo {
            path: PathBuf::from("/repo"),
            head: "abc123".to_string(),
            branch: "main".to_string(),
            is_detached: false,
            is_main: true,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        };

        let (can_delete, reason) = can_delete_worktree(&worktree).await;
        assert!(!can_delete);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("main worktree"));
    }

    #[test]
    fn test_can_merge_main_worktree() {
        let worktree = crate::tui::types::WorktreeInfo {
            path: PathBuf::from("/repo"),
            head: "abc123".to_string(),
            branch: "main".to_string(),
            is_detached: false,
            is_main: true,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        };

        let (can_merge, reason) = can_merge_worktree(&worktree);
        assert!(!can_merge);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("main worktree"));
    }

    #[test]
    fn test_can_merge_with_conflicts() {
        let worktree = crate::tui::types::WorktreeInfo {
            path: PathBuf::from("/repo/wt1"),
            head: "abc123".to_string(),
            branch: "feature-1".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: Some(crate::tui::types::MergeConflictInfo {
                conflict_files: vec!["file.txt".to_string()],
            }),
            has_commits_ahead: true,
            is_merging: false,
        };

        let (can_merge, reason) = can_merge_worktree(&worktree);
        assert!(!can_merge);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("merge conflicts"));
    }

    #[test]
    fn test_can_merge_no_commits_ahead() {
        let worktree = crate::tui::types::WorktreeInfo {
            path: PathBuf::from("/repo/wt1"),
            head: "abc123".to_string(),
            branch: "feature-1".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        };

        let (can_merge, reason) = can_merge_worktree(&worktree);
        assert!(!can_merge);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("no commits ahead"));
    }

    #[test]
    fn test_can_merge_valid() {
        let worktree = crate::tui::types::WorktreeInfo {
            path: PathBuf::from("/repo/wt1"),
            head: "abc123".to_string(),
            branch: "feature-1".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        };

        let (can_merge, reason) = can_merge_worktree(&worktree);
        assert!(can_merge);
        assert!(reason.is_none());
    }
}
