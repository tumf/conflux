//! Git command execution helpers.
//!
//! This module provides utilities for running Git commands,
//! organized into submodules by responsibility:
//! - `basic`: Basic git operations (run_git, status, branches, etc.)
//! - `commit`: Commit-related operations
//! - `worktree`: Worktree management
//! - `merge`: Merge operations and conflict detection

pub mod basic;
pub mod commit;
pub mod merge;
pub mod worktree;

// Re-export all public functions from submodules for backward compatibility
pub use basic::{
    branch_delete, branch_exists, check_git_repo, checkout, generate_unique_branch_name,
    get_conflict_files, get_current_branch, get_current_commit, get_status,
    has_uncommitted_changes, is_head_empty_commit, run_git,
};
pub use commit::{
    add_and_commit, create_archive_wip_commit, has_changes_to_commit, list_changes_in_head,
    squash_archive_wip_commits,
};
pub use merge::{
    check_merge_conflicts, first_parent_of, is_ancestor, is_merge_in_progress, merge, merge_branch,
    merge_commit_hash_by_subject_since, missing_merge_commits_since,
    presync_merge_subject_mismatches_since,
};
pub use worktree::{
    count_commits_ahead, is_worktree, list_worktrees, run_worktree_setup, worktree_add,
    worktree_remove,
};
