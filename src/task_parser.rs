//! Native task progress parsing for tasks.md files.
//!
//! This module provides native parsing of task checkboxes in markdown files,
//! supporting both bullet lists (`- [ ]`) and numbered lists (`1. [ ]`).

use crate::error::{OrchestratorError, Result};
use crate::tui::log_deduplicator;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;
use tracing::debug;

/// Task progress information.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskProgress {
    /// Number of completed tasks.
    pub completed: u32,
    /// Total number of tasks.
    pub total: u32,
}

impl TaskProgress {
    /// Create a new TaskProgress with zero counts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a TaskProgress with specific counts.
    #[cfg(test)]
    pub fn with_counts(completed: u32, total: u32) -> Self {
        Self { completed, total }
    }
}

/// Get the task checkbox regex pattern.
///
/// Pattern matches both bullet and numbered lists with checkboxes:
/// - `- [ ] Task` (bullet unchecked)
/// - `- [x] Task` (bullet checked)
/// - `* [X] Task` (asterisk checked)
/// - `1. [ ] Task` (numbered unchecked)
/// - `10. [x] Task` (numbered checked)
///
/// Does NOT match:
/// - `  - [ ] Sub-item` (indented sub-bullets)
/// - `Some text [ ]` (inline checkboxes)
/// - `## [x] Header` (markdown headers)
fn task_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        // ^: start of line
        // (?:[-*]|\d+\.): bullet (-/*) or numbered (digits followed by .)
        // \s+: one or more whitespace
        // \[([ xX])\]: checkbox with capture group for status
        Regex::new(r"^(?:[-*]|\d+\.)\s+\[([ xX])\]").expect("Invalid regex pattern")
    })
}

/// Parse task progress from markdown content.
///
/// Parses each line looking for task checkboxes at the start of lines.
/// Returns the count of completed and total tasks.
/// When change_id is provided, emits deduplicated debug logs.
pub fn parse_content(content: &str, change_id: Option<&str>) -> TaskProgress {
    let regex = task_regex();
    let mut progress = TaskProgress::new();

    for line in content.lines() {
        if let Some(captures) = regex.captures(line) {
            progress.total += 1;
            // Capture group 1 contains the checkbox status: ' ', 'x', or 'X'
            if let Some(status) = captures.get(1) {
                let status_char = status.as_str();
                if status_char == "x" || status_char == "X" {
                    progress.completed += 1;
                }
            }
        }
    }

    if let Some(change_id) = change_id {
        if log_deduplicator::should_log_task_progress(change_id, progress.completed, progress.total)
        {
            debug!(
                "Parsed task progress: {}/{} tasks completed",
                progress.completed, progress.total
            );
        }
    }

    progress
}

/// Parse task progress from a file.
///
/// Reads the file content and parses it for task checkboxes.
/// When change_id is provided, emits deduplicated debug logs.
pub fn parse_file(path: &Path, change_id: Option<&str>) -> Result<TaskProgress> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to read tasks file {:?}: {}", path, e))
    })?;

    Ok(parse_content(&content, change_id))
}

/// Parse task progress for a change by its ID.
///
/// Looks for tasks.md at `openspec/changes/{change_id}/tasks.md`.
pub fn parse_change(change_id: &str) -> Result<TaskProgress> {
    let tasks_path = Path::new("openspec/changes")
        .join(change_id)
        .join("tasks.md");

    if !tasks_path.exists() {
        return Err(OrchestratorError::ConfigLoad(format!(
            "Tasks file not found: {:?}",
            tasks_path
        )));
    }

    parse_file(&tasks_path, Some(change_id))
}

/// Parse task progress with worktree priority and base tree fallback.
///
/// Resolution order:
/// 1. Try worktree_path/openspec/changes/{change_id}/tasks.md (uncommitted)
/// 2. Fallback to openspec/changes/{change_id}/tasks.md (base tree)
///
/// This function is designed for TUI auto-refresh to read the latest progress
/// from worktrees where AI agents update tasks.md before committing.
pub fn parse_change_with_worktree_fallback(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Result<TaskProgress> {
    // Try worktree first (uncommitted changes)
    if let Some(wt_path) = worktree_path {
        let wt_tasks = wt_path
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md");

        if wt_tasks.exists() {
            debug!("Reading tasks from worktree: {:?}", wt_tasks);
            return parse_file(&wt_tasks, Some(change_id));
        }
    }

    // Fallback to base tree
    parse_change(change_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ====================
    // Bullet list format tests
    // ====================

    #[test]
    fn test_bullet_unchecked() {
        let content = "- [ ] Task 1\n- [ ] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_bullet_checked_lowercase() {
        let content = "- [x] Task 1\n- [x] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 2);
    }

    #[test]
    fn test_bullet_checked_uppercase() {
        let content = "- [X] Task 1\n- [X] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 2);
    }

    #[test]
    fn test_asterisk_bullets() {
        let content = "* [ ] Task 1\n* [x] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 1);
    }

    #[test]
    fn test_bullet_mixed_status() {
        let content = "- [x] Completed\n- [ ] Pending\n- [X] Also done";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 3);
        assert_eq!(progress.completed, 2);
    }

    // ====================
    // Numbered list format tests
    // ====================

    #[test]
    fn test_numbered_unchecked() {
        let content = "1. [ ] Task 1\n2. [ ] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_numbered_checked() {
        let content = "1. [x] Task 1\n2. [x] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 2);
    }

    #[test]
    fn test_numbered_multi_digit() {
        let content = "1. [x] Task 1\n10. [ ] Task 10\n100. [X] Task 100";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 3);
        assert_eq!(progress.completed, 2);
    }

    #[test]
    fn test_numbered_mixed_status() {
        let content = "1. [x] Done\n2. [ ] Not done\n3. [X] Also done";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 3);
        assert_eq!(progress.completed, 2);
    }

    // ====================
    // Mixed format tests
    // ====================

    #[test]
    fn test_mixed_bullets_and_numbers() {
        let content =
            "- [x] Bullet done\n1. [ ] Number pending\n* [X] Asterisk done\n2. [x] Number done";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 4);
        assert_eq!(progress.completed, 3);
    }

    #[test]
    fn test_mixed_with_sections() {
        let content = r#"# Tasks

## Implementation
- [x] Task 1
- [ ] Task 2

## Testing
1. [x] Test 1
2. [ ] Test 2
"#;
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 4);
        assert_eq!(progress.completed, 2);
    }

    // ====================
    // Edge case tests
    // ====================

    #[test]
    fn test_empty_content() {
        let progress = parse_content("", None);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_no_tasks() {
        let content = "# Just a header\nSome text without tasks.\n\n- Regular list item";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_indented_not_counted() {
        let content =
            "- [x] Parent task\n  - [ ] Sub-task (should not count)\n  - [x] Another sub-task";
        let progress = parse_content(content, None);
        // Only the parent task at the start of line should count
        assert_eq!(progress.total, 1);
        assert_eq!(progress.completed, 1);
    }

    #[test]
    fn test_inline_checkbox_not_counted() {
        let content = "Some text with [ ] inline checkbox\nAnother line [x] here";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_header_checkbox_not_counted() {
        let content = "## [x] Header with checkbox\n### [ ] Another header";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_real_world_example() {
        let content = r#"# Tasks

## Implementation Tasks

- [x] Create `src/task_parser.rs` module with regex-based task parsing
- [x] Implement `TaskProgress` struct with `completed` and `total` fields
- [ ] Implement `parse_content()` function to parse task markdown content
- [ ] Implement `parse_file()` function to read and parse tasks.md files

## Testing Tasks

1. [ ] Add unit tests for bullet list format
2. [ ] Add unit tests for numbered list format
3. [x] Add unit tests for mixed format

## Validation

- [ ] Run `cargo test` to verify all tests pass
- [ ] Run `cargo clippy` to check for warnings
"#;
        let progress = parse_content(content, None);
        // 4 bullets + 3 numbered + 2 bullets = 9 total
        // 2 checked bullets + 1 checked numbered = 3 completed
        assert_eq!(progress.total, 9);
        assert_eq!(progress.completed, 3);
    }

    // ====================
    // TaskProgress struct tests
    // ====================

    #[test]
    fn test_task_progress_new() {
        let progress = TaskProgress::new();
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.total, 0);
    }

    #[test]
    fn test_task_progress_with_counts() {
        let progress = TaskProgress::with_counts(5, 10);
        assert_eq!(progress.completed, 5);
        assert_eq!(progress.total, 10);
    }

    #[test]
    fn test_task_progress_default() {
        let progress = TaskProgress::default();
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.total, 0);
    }

    // ====================
    // File parsing tests
    // ====================

    #[test]
    fn test_parse_file_not_found() {
        let result = parse_file(Path::new("/nonexistent/path/tasks.md"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_change_not_found() {
        let result = parse_change("nonexistent-change-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_change_with_worktree_fallback_from_worktree() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let worktree_path = temp_dir.path();

        // Create worktree structure
        let change_dir = worktree_path.join("openspec/changes/test-change");
        std::fs::create_dir_all(&change_dir).unwrap();

        // Write tasks.md in worktree
        let tasks_content = "- [x] Task 1\n- [x] Task 2\n- [ ] Task 3";
        std::fs::write(change_dir.join("tasks.md"), tasks_content).unwrap();

        // Parse with worktree
        let result = parse_change_with_worktree_fallback("test-change", Some(worktree_path));
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 3);
    }

    #[test]
    fn test_parse_change_with_worktree_fallback_to_base() {
        use std::env;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create base tree structure
        let change_dir = base_path.join("openspec/changes/test-change");
        std::fs::create_dir_all(&change_dir).unwrap();

        // Write tasks.md in base tree
        let tasks_content = "- [x] Task 1\n- [ ] Task 2";
        std::fs::write(change_dir.join("tasks.md"), tasks_content).unwrap();

        // Parse with non-existent worktree (should fallback to base)
        let result = parse_change_with_worktree_fallback("test-change", None);
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 2);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }
}
