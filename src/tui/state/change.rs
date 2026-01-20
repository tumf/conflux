//! ChangeState implementation
//!
//! Contains the state representation for individual changes in the TUI.

use crate::openspec::Change;
use std::time::{Duration, Instant};

use super::super::types::QueueStatus;

/// State of a single change in the TUI
#[derive(Debug, Clone)]
pub struct ChangeState {
    /// Change ID
    pub id: String,
    /// Number of completed tasks
    pub completed_tasks: u32,
    /// Total number of tasks
    pub total_tasks: u32,
    /// Queue status
    pub queue_status: QueueStatus,
    /// Whether this change is selected
    pub selected: bool,
    /// Whether this is a newly detected change
    pub is_new: bool,
    /// Last modified timestamp
    #[allow(dead_code)]
    pub last_modified: String,
    /// Whether this change is approved for execution
    pub is_approved: bool,
    /// Whether this change is eligible for parallel execution
    pub is_parallel_eligible: bool,
    /// Whether a worktree exists for this change
    pub has_worktree: bool,
    /// When processing started for this change
    pub started_at: Option<Instant>,
    /// Elapsed time when processing finished (for display after completion)
    pub elapsed_time: Option<Duration>,
    /// Current iteration number (for apply/archive/acceptance operations)
    pub iteration_number: Option<u32>,
}

impl ChangeState {
    /// Create a new ChangeState from a Change
    pub fn from_change(change: &Change, selected: bool) -> Self {
        Self {
            id: change.id.clone(),
            completed_tasks: change.completed_tasks,
            total_tasks: change.total_tasks,
            selected,
            is_new: false,
            queue_status: QueueStatus::NotQueued,
            last_modified: change.last_modified.clone(),
            is_approved: change.is_approved,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        }
    }

    /// Calculate progress percentage
    pub fn progress_percent(&self) -> f32 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        (self.completed_tasks as f32 / self.total_tasks as f32) * 100.0
    }

    /// Calculate progress ratio (0.0 to 1.0)
    #[allow(dead_code)]
    pub fn progress_ratio(&self) -> f64 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.completed_tasks as f64 / self.total_tasks as f64
    }

    /// Check if all tasks are completed
    #[allow(dead_code)]
    pub fn is_complete(&self) -> bool {
        self.completed_tasks == self.total_tasks && self.total_tasks > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_state_progress() {
        let change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 3,
            total_tasks: 6,
            queue_status: QueueStatus::NotQueued,
            selected: false,
            is_new: false,
            last_modified: "now".to_string(),
            is_approved: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        assert_eq!(change.progress_percent(), 50.0);
        assert_eq!(change.progress_ratio(), 0.5);
        assert!(!change.is_complete());
    }

    #[test]
    fn test_change_state_from_change() {
        let change = Change {
            id: "test-id".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            last_modified: "2024-01-01".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let state = ChangeState::from_change(&change, true);

        assert_eq!(state.id, "test-id");
        assert_eq!(state.completed_tasks, 2);
        assert_eq!(state.total_tasks, 5);
        assert!(state.selected);
        assert!(!state.is_new);
        assert_eq!(state.queue_status, QueueStatus::NotQueued);
        assert!(state.is_approved);
        assert!(state.is_parallel_eligible);
    }

    #[test]
    fn test_progress_percent_zero_total() {
        let change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 0,
            queue_status: QueueStatus::NotQueued,
            selected: false,
            is_new: false,
            last_modified: "now".to_string(),
            is_approved: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        assert_eq!(change.progress_percent(), 0.0);
        assert_eq!(change.progress_ratio(), 0.0);
        assert!(!change.is_complete());
    }

    #[test]
    fn test_is_complete() {
        let mut change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 5,
            total_tasks: 5,
            queue_status: QueueStatus::NotQueued,
            selected: false,
            is_new: false,
            last_modified: "now".to_string(),
            is_approved: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        assert!(change.is_complete());

        change.completed_tasks = 4;
        assert!(!change.is_complete());
    }
}
