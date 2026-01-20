//! Merge operations for parallel execution.
//!
//! This module handles:
//! - Merge attempt logic (checking base branch state)
//! - Merge execution and conflict resolution
//! - Merge verification

use crate::error::{OrchestratorError, Result};
use crate::vcs::git::commands as git_commands;
use std::path::Path;

/// Check if the base branch is dirty (has uncommitted changes or merge in progress).
///
/// Returns `Ok(None)` if the base branch is clean, or `Ok(Some(reason))` with a description
/// of why the base branch is dirty.
pub async fn base_dirty_reason(repo_root: &Path) -> Result<Option<String>> {
    let is_git_repo = git_commands::check_git_repo(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if !is_git_repo {
        return Ok(None);
    }

    let merge_in_progress = git_commands::is_merge_in_progress(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if merge_in_progress {
        return Ok(Some("Merge in progress (MERGE_HEAD exists)".to_string()));
    }

    let (has_changes, status) = git_commands::has_uncommitted_changes(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if has_changes {
        let trimmed = status.trim();
        let reason = if trimmed.is_empty() {
            "Working tree has uncommitted changes".to_string()
        } else {
            format!("Working tree has uncommitted changes:\n{}", trimmed)
        };
        return Ok(Some(reason));
    }

    Ok(None)
}

/// Result of a merge attempt
#[derive(Debug)]
pub enum MergeAttempt {
    /// Merge succeeded
    Merged,
    /// Merge deferred with reason (e.g., base dirty, archive not complete)
    Deferred(String),
}
