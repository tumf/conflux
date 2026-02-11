//! Hook context helper functions for CLI and TUI modes.
//!
//! Provides builder functions for creating HookContext instances
//! with appropriate values for different orchestration events.
//!
//! Note: These functions are infrastructure for future CLI/TUI integration.
//! They will be used as the refactoring continues.

#![allow(dead_code)]

use crate::hooks::HookContext;
use crate::openspec::Change;

/// Build a hook context for run start.
pub fn build_start_context(total_changes: usize) -> HookContext {
    HookContext::new(0, total_changes, total_changes, false)
}

/// Build a hook context for run finish.
pub fn build_finish_context(
    changes_processed: usize,
    total_changes: usize,
    status: &str,
) -> HookContext {
    HookContext::new(changes_processed, total_changes, 0, false).with_status(status)
}

/// Build a hook context for change start.
pub fn build_change_start_context(
    change: &Change,
    changes_processed: usize,
    total_changes: usize,
    remaining_changes: usize,
) -> HookContext {
    HookContext::new(changes_processed, total_changes, remaining_changes, false)
        .with_change(&change.id, change.completed_tasks, change.total_tasks)
        .with_apply_count(0)
}

/// Build a hook context for change end.
pub fn build_change_end_context(
    change: &Change,
    changes_processed: usize,
    total_changes: usize,
    remaining_changes: usize,
    apply_count: u32,
) -> HookContext {
    HookContext::new(changes_processed, total_changes, remaining_changes, false)
        .with_change(&change.id, change.completed_tasks, change.total_tasks)
        .with_apply_count(apply_count)
}

/// Build a hook context for pre/post apply.
pub fn build_apply_context(
    change: &Change,
    changes_processed: usize,
    total_changes: usize,
    remaining_changes: usize,
    apply_count: u32,
) -> HookContext {
    HookContext::new(changes_processed, total_changes, remaining_changes, false)
        .with_change(&change.id, change.completed_tasks, change.total_tasks)
        .with_apply_count(apply_count)
}

/// Build a hook context for on_change_complete.
pub fn build_change_complete_context(
    change: &Change,
    changes_processed: usize,
    total_changes: usize,
    remaining_changes: usize,
    apply_count: u32,
) -> HookContext {
    HookContext::new(changes_processed, total_changes, remaining_changes, false)
        .with_change(&change.id, change.completed_tasks, change.total_tasks)
        .with_apply_count(apply_count)
}

/// Build a hook context for pre/post archive.
pub fn build_archive_context(
    change: &Change,
    changes_processed: usize,
    total_changes: usize,
    remaining_changes: usize,
    apply_count: u32,
) -> HookContext {
    HookContext::new(changes_processed, total_changes, remaining_changes, false)
        .with_change(&change.id, change.completed_tasks, change.total_tasks)
        .with_apply_count(apply_count)
}

/// Build a hook context for errors.
pub fn build_error_context(
    change: &Change,
    changes_processed: usize,
    total_changes: usize,
    remaining_changes: usize,
    apply_count: u32,
    error: &str,
) -> HookContext {
    HookContext::new(changes_processed, total_changes, remaining_changes, false)
        .with_change(&change.id, change.completed_tasks, change.total_tasks)
        .with_apply_count(apply_count)
        .with_error(error)
}

/// Build a hook context for queue add/remove (TUI only).
pub fn build_queue_context(
    change: &Change,
    changes_processed: usize,
    total_changes: usize,
    remaining_changes: usize,
) -> HookContext {
    HookContext::new(changes_processed, total_changes, remaining_changes, false).with_change(
        &change.id,
        change.completed_tasks,
        change.total_tasks,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn test_build_start_context() {
        let ctx = build_start_context(5);
        assert_eq!(ctx.changes_processed, 0);
        assert_eq!(ctx.total_changes, 5);
        assert_eq!(ctx.remaining_changes, 5);
        assert!(ctx.change_id.is_none());
    }

    #[test]
    fn test_build_finish_context() {
        let ctx = build_finish_context(3, 5, "completed");
        assert_eq!(ctx.changes_processed, 3);
        assert_eq!(ctx.total_changes, 5);
        assert_eq!(ctx.remaining_changes, 0);
        assert_eq!(ctx.status, Some("completed".to_string()));
    }

    #[test]
    fn test_build_change_start_context() {
        let change = test_change("my-change", 2, 5);
        let ctx = build_change_start_context(&change, 1, 3, 2);

        assert_eq!(ctx.change_id, Some("my-change".to_string()));
        assert_eq!(ctx.completed_tasks, Some(2));
        assert_eq!(ctx.total_tasks, Some(5));
        assert_eq!(ctx.apply_count, 0);
        assert_eq!(ctx.changes_processed, 1);
        assert_eq!(ctx.total_changes, 3);
        assert_eq!(ctx.remaining_changes, 2);
    }

    #[test]
    fn test_build_apply_context() {
        let change = test_change("my-change", 3, 5);
        let ctx = build_apply_context(&change, 1, 3, 2, 2);

        assert_eq!(ctx.change_id, Some("my-change".to_string()));
        assert_eq!(ctx.apply_count, 2);
    }

    #[test]
    fn test_build_error_context() {
        let change = test_change("my-change", 2, 5);
        let ctx = build_error_context(&change, 1, 3, 2, 1, "Apply failed");

        assert_eq!(ctx.change_id, Some("my-change".to_string()));
        assert_eq!(ctx.error, Some("Apply failed".to_string()));
    }

    #[test]
    fn test_build_archive_context() {
        let change = test_change("my-change", 5, 5);
        let ctx = build_archive_context(&change, 1, 3, 2, 3);

        assert_eq!(ctx.change_id, Some("my-change".to_string()));
        assert_eq!(ctx.completed_tasks, Some(5));
        assert_eq!(ctx.total_tasks, Some(5));
        assert_eq!(ctx.apply_count, 3);
    }

    #[test]
    fn test_build_queue_context() {
        let change = test_change("my-change", 1, 5);
        let ctx = build_queue_context(&change, 0, 5, 5);

        assert_eq!(ctx.change_id, Some("my-change".to_string()));
        assert_eq!(ctx.apply_count, 0); // Queue operations don't have apply count
    }
}
