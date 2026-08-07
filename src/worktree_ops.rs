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
//!
//! [`inspection`] decides how much each observation is allowed to cost. Every
//! frontend observes through [`observe_worktrees`], so the eligibility policy
//! and the revision-keyed observation cache are the same ones for all of them.

pub mod git_backend;
pub mod inspection;
pub mod service;

use std::path::{Path, PathBuf};
use tracing::{debug, warn};

pub use inspection::{InspectionState, ObservationRequest};

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

/// Load all worktrees with conflict checking and commits ahead detection.
///
/// This is the canonical worktree retrieval function used by both TUI and Web API
/// to ensure consistent worktree state across interfaces. An unobservable
/// commits-ahead state is flattened to `false` here; callers that must not treat
/// that as safe use [`observe_worktrees`] instead.
pub async fn get_worktrees(
    repo_root: &Path,
    request: ObservationRequest,
) -> crate::error::Result<Vec<crate::tui::types::WorktreeInfo>> {
    Ok(observe_worktrees(repo_root, request)
        .await?
        .into_iter()
        .map(|observation| observation.info)
        .collect())
}

/// Load all worktrees, keeping unobservable safety facts distinguishable.
///
/// Every registered worktree is returned. What `request` decides is only how
/// many Git commands the answer is worth: see [`inspection`] for the policy and
/// why a skipped row must never read as conflict-free.
pub async fn observe_worktrees(
    repo_root: &Path,
    request: ObservationRequest,
) -> crate::error::Result<Vec<WorktreeObservation>> {
    use crate::worktree_ops::service::SafetyFact;
    use inspection::{Admission, InspectionCandidate, InspectionScope};

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
                inspection: InspectionState::NotInspected,
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
    if base_branch.is_empty() {
        return Ok(zip_observations(worktrees, ahead));
    }

    // Eligibility is decided before a single ahead/conflict command exists, so
    // an ineligible worktree costs exactly nothing.
    let scope = InspectionScope::resolve(repo_root, &request);
    let admitted: Vec<(usize, Admission)> = worktrees
        .iter()
        .enumerate()
        .map(|(idx, worktree)| {
            let candidate = InspectionCandidate::new(
                &worktree.path,
                &worktree.branch,
                worktree.is_main,
                worktree.is_detached,
            );
            (idx, scope.admits(&candidate))
        })
        .filter(|(_, admission)| *admission != Admission::Skip)
        .collect();

    if admitted.is_empty() {
        return Ok(zip_observations(worktrees, ahead));
    }

    // One base-side resolution for the whole refresh, not one per worktree.
    let Some(base_head) = crate::vcs::git::commands::rev_parse_commit(repo_root, &base_branch)
        .await
        .ok()
        .flatten()
    else {
        debug!(
            base_branch = %base_branch,
            "base branch tip could not be resolved; no worktree can be inspected against it"
        );
        return Ok(zip_observations(worktrees, ahead));
    };
    let repository = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let cache = inspection::shared_cache();

    // Check conflicts and commits ahead in parallel for the admitted worktrees.
    let mut tasks = tokio::task::JoinSet::new();

    for (idx, admission) in admitted {
        let worktree = &worktrees[idx];
        let key = inspection::ObservationKey {
            repository: repository.clone(),
            branch: worktree.branch.clone(),
            base_head: base_head.clone(),
            worktree_head: worktree.head.clone(),
            merge_base: String::new(),
        };
        let wt_path = worktree.path.clone();
        let branch_name = worktree.branch.clone();
        let base_branch_clone = base_branch.clone();

        tasks.spawn(async move {
            // The merge base completes the identity a reused observation is
            // keyed by. It is derived first, and cheaply, so a cache hit never
            // reaches the simulation it exists to avoid.
            inspection::record(inspection::InspectionCommand::MergeBase, &wt_path);
            let merge_base =
                crate::vcs::git::commands::merge_base(&wt_path, &key.base_head, &key.worktree_head)
                    .await
                    .ok()
                    .flatten();
            let Some(merge_base) = merge_base else {
                debug!(
                    worktree = %wt_path.display(),
                    "merge base could not be resolved; leaving the observation uninspected"
                );
                return (idx, None, None);
            };
            let key = inspection::ObservationKey { merge_base, ..key };

            if admission == Admission::Cacheable {
                if let Some(observation) = inspection::shared_cache().get(&key) {
                    return (idx, Some(key), Some((observation, InspectionState::Reused)));
                }
            }

            inspection::record(inspection::InspectionCommand::Conflicts, &wt_path);
            let conflict_result =
                crate::vcs::git::commands::check_merge_conflicts(&wt_path, &base_branch_clone)
                    .await;

            inspection::record(inspection::InspectionCommand::CommitsAhead, &wt_path);
            let ahead_result = crate::vcs::git::commands::count_commits_ahead(
                &wt_path,
                &base_branch_clone,
                &branch_name,
            )
            .await;

            let conflict_files = match conflict_result {
                Ok(simulation) => Some(simulation.conflict_files().unwrap_or_default()),
                Err(error) => {
                    debug!(
                        worktree = %wt_path.display(),
                        "conflict check failed: {error}"
                    );
                    None
                }
            };
            let has_commits_ahead = match ahead_result {
                Ok(count) => SafetyFact::from(count > 0),
                Err(error) => {
                    debug!(
                        worktree = %wt_path.display(),
                        "commits ahead check failed: {error}"
                    );
                    SafetyFact::Unknown
                }
            };

            match (conflict_files, has_commits_ahead) {
                // Only a complete observation is worth keying: caching a failed
                // command would keep answering "unknown" until a revision moves.
                (Some(conflict_files), SafetyFact::Yes | SafetyFact::No) => (
                    idx,
                    Some(key),
                    Some((
                        inspection::Observation {
                            conflict_files,
                            has_commits_ahead,
                        },
                        InspectionState::Checked,
                    )),
                ),
                (conflict_files, has_commits_ahead) => (
                    idx,
                    None,
                    Some((
                        inspection::Observation {
                            conflict_files: conflict_files.unwrap_or_default(),
                            has_commits_ahead,
                        },
                        InspectionState::Checked,
                    )),
                ),
            }
        });
    }

    // Collect results
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((idx, key, Some((observation, state)))) => {
                if !observation.conflict_files.is_empty() {
                    worktrees[idx].merge_conflict = Some(crate::tui::types::MergeConflictInfo {
                        conflict_files: observation.conflict_files.clone(),
                    });
                }
                worktrees[idx].has_commits_ahead = observation.has_commits_ahead.is_known_yes();
                worktrees[idx].inspection = state;
                ahead[idx] = observation.has_commits_ahead;
                if let Some(key) = key {
                    cache.insert(key, observation);
                }
            }
            Ok((_, _, None)) => {}
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

/// True when two paths name the same worktree.
///
/// Git reports canonical paths, while a server-derived path is whatever the
/// configured workspace root produced. On platforms with symlinked temp or home
/// directories those differ textually for the same directory, so a plain `==`
/// would report a freshly created worktree as unobservable.
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}
