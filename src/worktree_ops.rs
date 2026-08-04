//! Common worktree operations shared between TUI and Web API.
//!
//! This module provides the shared worktree retrieval logic used to keep the TUI
//! and the `/api/v2` operator console consistent. Delete/merge eligibility now
//! lives in [`service`]; see [`service::classify_delete_eligibility`] and
//! [`service::classify_merge_eligibility`].
//!
//! [`service`] adds the frontend-independent *operation* layer on top of these
//! observations: create, guarded delete, base merge, the repository mutation
//! guard, hooks, and events. [`git_backend`] is its only real-repository
//! implementation.

pub mod git_backend;
pub mod service;

use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// One worktree as observed, with the safety facts a mutation decision needs.
///
/// [`crate::tui::types::WorktreeInfo`] is a presentation projection and its
/// `has_commits_ahead` is a plain `bool`, so an observation that failed and one
/// that confidently answered "no" look identical there. This type keeps them
/// apart for the callers that must fail closed on the difference.
pub struct WorktreeObservation {
    /// The presentation projection every frontend already renders.
    pub info: crate::tui::types::WorktreeInfo,
    /// Whether this branch has commits base does not have.
    pub has_commits_ahead: crate::worktree_ops::service::SafetyFact,
}

/// Load all worktrees with parallel conflict checking and commits ahead detection.
///
/// This is the canonical worktree retrieval function used by both TUI and Web API
/// to ensure consistent worktree state across interfaces. An unobservable
/// commits-ahead state is flattened to `false` here; callers that must not treat
/// that as safe use [`observe_worktrees`] instead.
pub async fn get_worktrees(
    repo_root: &Path,
) -> crate::error::Result<Vec<crate::tui::types::WorktreeInfo>> {
    Ok(observe_worktrees(repo_root)
        .await?
        .into_iter()
        .map(|observation| observation.info)
        .collect())
}

/// Load all worktrees, keeping unobservable safety facts distinguishable.
pub async fn observe_worktrees(repo_root: &Path) -> crate::error::Result<Vec<WorktreeObservation>> {
    use crate::worktree_ops::service::SafetyFact;

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

    // A worktree with no branch of its own has no commits-ahead answer to give,
    // and neither does the main worktree, which is the base. Both start at
    // `Unknown` so a caller that would delete on the strength of "not ahead"
    // cannot get that answer from a worktree that was never measured.
    let mut ahead: Vec<SafetyFact> = worktrees
        .iter()
        .map(|worktree| {
            if worktree.is_main {
                SafetyFact::No
            } else {
                SafetyFact::Unknown
            }
        })
        .collect();

    // Get the base branch name from the main worktree
    let base_branch = if let Some(main_wt) = worktrees.iter().find(|wt| wt.is_main) {
        main_wt.branch.clone()
    } else {
        // Fallback: get current branch from repo root
        match crate::vcs::git::commands::get_current_branch(repo_root).await? {
            Some(branch) => branch,
            None => {
                // Detached HEAD or error: nothing can be compared against a base
                // that could not be resolved, so every answer stays unknown.
                return Ok(zip_observations(worktrees, ahead));
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
                        ahead[idx] = SafetyFact::from(count > 0);
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

    Ok(zip_observations(worktrees, ahead))
}

fn zip_observations(
    worktrees: Vec<crate::tui::types::WorktreeInfo>,
    ahead: Vec<crate::worktree_ops::service::SafetyFact>,
) -> Vec<WorktreeObservation> {
    worktrees
        .into_iter()
        .zip(ahead)
        .map(|(info, has_commits_ahead)| WorktreeObservation {
            info,
            has_commits_ahead,
        })
        .collect()
}
