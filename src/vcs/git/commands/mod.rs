//! Git command execution helpers.
//!
//! This module provides utilities for running Git commands,
//! organized into submodules by responsibility:
//! - `basic`: Basic git operations (run_git, status, branches, etc.)
//! - `commit`: Commit-related operations
//! - `worktree`: Worktree management
//! - `merge`: Merge operations and conflict detection
//! - `status_policy`: the read-only `git status` optional-lock policy every
//!   Conflux-owned status observation is built from

pub mod basic;
pub mod commit;
pub mod merge;
pub mod status_policy;
pub mod worktree;

// Re-export all public functions from submodules for backward compatibility
pub use basic::{
    branch_delete, branch_delete_at_oid, branch_delete_if_merged, branch_exists, branch_ref_oid,
    check_git_repo, checkout, generate_unique_branch_name, get_changed_files,
    get_changed_files_since, get_conflict_files, get_current_branch, get_current_commit,
    get_ref_revision, get_status, has_uncommitted_changes, is_head_empty_commit, porcelain_status,
    push_same_named_branch, run_git,
};
pub use commit::{
    create_archive_wip_commit, create_verified_commit, list_changes_in_head,
    list_changes_with_uncommitted_files, squash_archive_wip_commits, validate_staged_snapshot,
};
// The bounded-diagnostic surface is re-exported for callers and tests even
// where this crate's own binary target has no use for every item.
#[allow(unused_imports)]
pub use merge::{
    bounded_prefix, check_merge_conflicts, commit_diff_entries, commits_with_exact_subject,
    committed_file_text, committed_tree_paths, diff_paths_between, first_parent_lineage,
    index_conflict_entries, index_stage0_paths, is_ancestor, is_clean_including_untracked,
    is_merge_in_progress, merge, merge_base, merge_branch, merge_branch_preserving_conflict,
    merge_head, parents_of, rev_parse_commit, summarize_merge_tree, CommitDiffEntry,
    MergeSimulation, PreservedMergeOutcome, MAX_CONFLICT_SAMPLE, MAX_OUTPUT_PREFIX_BYTES,
};
#[allow(unused_imports)]
pub use worktree::{
    classify_worktree_identity, count_commits_ahead, is_worktree, list_worktrees,
    run_worktree_setup, run_worktree_teardown, validate_worktree_command_cwd,
    validate_worktree_command_cwd_facts, validate_worktree_identity, worktree_add,
    worktree_records, worktree_remove, worktree_remove_forced, worktree_remove_with_options,
    worktree_setup_completed_diagnostic, worktree_setup_start_diagnostic, RegisteredWorktreePath,
    WorktreeCommandCwdFacts, WorktreeCommandCwdValidationError, WorktreeIdentity, WorktreeRecord,
    WorktreeRemoveOptions, WorktreeSetupReport,
};
