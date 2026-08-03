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
    get_changed_files, get_changed_files_since, get_conflict_files, get_current_branch,
    get_current_commit, get_ref_revision, get_status, has_uncommitted_changes,
    is_head_empty_commit, push_same_named_branch, run_git,
};
pub use commit::{
    create_archive_wip_commit, create_verified_commit, list_changes_in_head,
    list_changes_with_uncommitted_files, squash_archive_wip_commits, validate_staged_snapshot,
};
pub use merge::{
    check_merge_conflicts, commit_diff_entries, commits_with_exact_subject, committed_tree_paths,
    first_parent_lineage, index_conflict_entries, index_stage0_paths, is_ancestor,
    is_clean_including_untracked, is_merge_in_progress, merge, merge_base, merge_branch,
    merge_branch_preserving_conflict, merge_head, parents_of, rev_parse_commit, CommitDiffEntry,
    PreservedMergeOutcome,
};
#[allow(unused_imports)]
pub use worktree::{
    classify_worktree_identity, count_commits_ahead, is_worktree, list_worktrees,
    run_worktree_setup, run_worktree_teardown, validate_worktree_command_cwd,
    validate_worktree_command_cwd_facts, validate_worktree_identity, worktree_add,
    worktree_records, worktree_remove, worktree_remove_with_options, RegisteredWorktreePath,
    WorktreeCommandCwdFacts, WorktreeCommandCwdValidationError, WorktreeIdentity, WorktreeRecord,
    WorktreeRemoveOptions,
};
