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
            format!("(detached: {})", &self.head)
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
    /// Returns "merging" if merge in progress, "merged" if not ahead of base, empty otherwise
    pub fn merge_status_label(&self) -> &str {
        if self.is_merging {
            "merging"
        } else if !self.has_commits_ahead && !self.is_main && !self.is_detached {
            "merged"
        } else {
            ""
        }
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
        };
        assert!(wt_with_conflict.has_merge_conflict());
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
        };
        assert_eq!(wt.conflict_file_count(), 2);
    }
}
