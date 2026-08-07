//! Implementation blocks and tests for TUI types
//!
//! This module contains the method implementations for types defined in `types.rs`.
//! Separated from type definitions to maintain a clear distinction between
//! type declarations and their behavior, as required by the TUI architecture spec.

#[cfg(test)]
use super::types::MergeConflictInfo;
use super::types::WorktreeInfo;

impl WorktreeInfo {
    /// Get display label for the worktree (basename of path)
    pub fn display_label(&self) -> String {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)")
            .to_string()
    }

    /// Get display branch name (or "(detached)" if detached HEAD)
    pub fn display_branch(&self) -> String {
        if self.is_detached {
            format!("(detached: {})", self.head)
        } else if self.branch.is_empty() {
            "(no branch)".to_string()
        } else {
            self.branch.clone()
        }
    }

    /// Check if worktree has merge conflicts
    pub fn has_merge_conflict(&self) -> bool {
        self.merge_conflict
            .as_ref()
            .map(|c| !c.conflict_files.is_empty())
            .unwrap_or(false)
    }

    /// Get count of conflicting files
    pub fn conflict_file_count(&self) -> usize {
        self.merge_conflict
            .as_ref()
            .map(|c| c.conflict_files.len())
            .unwrap_or(0)
    }

    /// Get merge status label for display
    ///
    /// Returns "merging" if merge in progress, "merged" if the branch was
    /// inspected and carries nothing base does not already have, empty
    /// otherwise. An uninspected row never reports "merged": nothing measured
    /// it, and saying so would be the false no-commits-ahead claim a skipped
    /// inspection must not produce.
    pub fn merge_status_label(&self) -> &str {
        if self.is_merging {
            "merging"
        } else if !self.has_commits_ahead
            && !self.is_main
            && !self.is_detached
            && self.inspection.is_inspected()
        {
            "merged"
        } else {
            ""
        }
    }

    /// Badge naming how this row's ahead/conflict facts were obtained.
    ///
    /// Empty for the main worktree, which is the base and has nothing to be
    /// inspected against, and for an ordinary freshly checked row.
    pub fn inspection_label(&self) -> &'static str {
        if self.is_main {
            return "";
        }
        self.inspection.label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_worktree_info_display_label() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "main".to_string(),
            is_detached: false,
            is_main: true,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
            inspection: crate::worktree_ops::InspectionState::Checked,
        };
        assert_eq!(wt.display_label(), "worktree");
    }

    #[test]
    fn test_worktree_info_display_branch_normal() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "feature-branch".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
            inspection: crate::worktree_ops::InspectionState::Checked,
        };
        assert_eq!(wt.display_branch(), "feature-branch");
    }

    #[test]
    fn test_worktree_info_display_branch_detached() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "".to_string(),
            is_detached: true,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
            inspection: crate::worktree_ops::InspectionState::Checked,
        };
        assert_eq!(wt.display_branch(), "(detached: abc123)");
    }

    #[test]
    fn test_worktree_info_has_merge_conflict() {
        let wt_no_conflict = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "main".to_string(),
            is_detached: false,
            is_main: true,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
            inspection: crate::worktree_ops::InspectionState::Checked,
        };
        assert!(!wt_no_conflict.has_merge_conflict());

        let wt_with_conflict = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: Some(MergeConflictInfo {
                conflict_files: vec!["file.rs".to_string()],
            }),
            has_commits_ahead: true,
            is_merging: false,
            inspection: crate::worktree_ops::InspectionState::Checked,
        };
        assert!(wt_with_conflict.has_merge_conflict());
    }

    #[test]
    fn an_uninspected_row_is_never_labelled_merged() {
        use crate::worktree_ops::InspectionState;

        let mut wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "stale-change".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
            inspection: InspectionState::Checked,
        };
        assert_eq!(
            wt.merge_status_label(),
            "merged",
            "an inspected branch with nothing ahead of base really is merged"
        );

        // Same row, same fields, one difference: nobody measured it.
        wt.inspection = InspectionState::NotInspected;
        assert_eq!(
            wt.merge_status_label(),
            "",
            "a skipped inspection must not be reported as a completed merge"
        );
        assert_eq!(wt.inspection_label(), "not inspected");

        wt.inspection = InspectionState::Reused;
        assert_eq!(wt.merge_status_label(), "merged");
        assert_eq!(
            wt.inspection_label(),
            "cached",
            "a reused observation is distinguishable from a freshly checked one"
        );

        wt.inspection = InspectionState::Checked;
        assert_eq!(
            wt.inspection_label(),
            "",
            "the ordinary checked row carries no extra badge"
        );

        let main = WorktreeInfo {
            is_main: true,
            inspection: InspectionState::NotInspected,
            ..wt.clone()
        };
        assert_eq!(
            main.inspection_label(),
            "",
            "the main worktree is the base and is never inspected against itself"
        );
    }

    #[test]
    fn test_worktree_info_conflict_file_count() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: Some(MergeConflictInfo {
                conflict_files: vec!["file1.rs".to_string(), "file2.rs".to_string()],
            }),
            has_commits_ahead: true,
            is_merging: false,
            inspection: crate::worktree_ops::InspectionState::Checked,
        };
        assert_eq!(wt.conflict_file_count(), 2);
    }
}
