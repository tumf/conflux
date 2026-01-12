//! Common type definitions for execution operations.
//!
//! Provides shared types used across serial and parallel execution modes.

// Allow dead_code since this is a foundation module - types will be used
// by subsequent changes (refactor-archive-common, refactor-apply-common).
#![allow(dead_code)]

use crate::config::OrchestratorConfig;
use crate::hooks::HookRunner;
use crate::parallel::ParallelEvent;
use std::path::Path;
use tokio::sync::mpsc;

/// Execution context containing all information needed to execute operations.
///
/// This context is passed to common execution functions (archive, apply, etc.)
/// and provides a unified interface for both serial and parallel modes.
///
/// # Examples
///
/// ```ignore
/// // Serial mode: no workspace path, hooks available
/// let ctx = ExecutionContext::new("add-feature", &config)
///     .with_hooks(&hook_runner);
///
/// // Parallel mode: workspace path specified, event channel for streaming
/// let ctx = ExecutionContext::new("add-feature", &config)
///     .with_workspace(workspace_path)
///     .with_event_tx(tx.clone());
/// ```
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    /// The change ID being processed
    pub change_id: &'a str,

    /// Path to the workspace directory.
    /// - `None` for serial mode (operates in main workspace)
    /// - `Some(path)` for parallel mode (operates in isolated workspace)
    pub workspace_path: Option<&'a Path>,

    /// Reference to the orchestrator configuration
    pub config: &'a OrchestratorConfig,

    /// Hook runner for executing lifecycle hooks.
    /// - `Some` when hooks are configured (typically serial mode)
    /// - `None` when hooks are not available (parallel mode, for now)
    pub hooks: Option<&'a HookRunner>,

    /// Event channel for streaming progress updates.
    /// - `Some` for parallel mode (sends events to TUI)
    /// - `None` for serial mode (uses direct output)
    pub event_tx: Option<mpsc::Sender<ParallelEvent>>,
}

impl<'a> ExecutionContext<'a> {
    /// Create a new execution context with required fields.
    pub fn new(change_id: &'a str, config: &'a OrchestratorConfig) -> Self {
        Self {
            change_id,
            workspace_path: None,
            config,
            hooks: None,
            event_tx: None,
        }
    }

    /// Set the workspace path for parallel mode execution.
    #[allow(dead_code)]
    pub fn with_workspace(mut self, path: &'a Path) -> Self {
        self.workspace_path = Some(path);
        self
    }

    /// Set the hook runner for lifecycle hooks.
    #[allow(dead_code)]
    pub fn with_hooks(mut self, hooks: &'a HookRunner) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Set the event channel for progress streaming.
    #[allow(dead_code)]
    pub fn with_event_tx(mut self, tx: mpsc::Sender<ParallelEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Check if this context is for parallel mode execution.
    pub fn is_parallel(&self) -> bool {
        self.workspace_path.is_some()
    }

    /// Get the working directory for this execution.
    /// Returns the workspace path if set, otherwise returns None (main workspace).
    pub fn working_dir(&self) -> Option<&Path> {
        self.workspace_path
    }
}

/// Result of an execution operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionResult {
    /// Operation completed successfully
    Success,

    /// Operation failed with an error message
    Failed { message: String },

    /// Operation was cancelled (e.g., user interrupt, iteration limit)
    Cancelled { reason: String },
}

impl ExecutionResult {
    /// Create a success result.
    pub fn success() -> Self {
        Self::Success
    }

    /// Create a failed result with the given message.
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    /// Create a cancelled result with the given reason.
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self::Cancelled {
            reason: reason.into(),
        }
    }

    /// Check if this result represents success.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Check if this result represents failure.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Check if this result represents cancellation.
    #[allow(dead_code)]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }
}

/// Progress information for a change's task completion.
///
/// Tracks how many tasks have been completed out of the total,
/// and provides utility methods for calculating progress.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProgressInfo {
    /// Number of completed tasks
    pub completed: u32,

    /// Total number of tasks
    pub total: u32,
}

impl ProgressInfo {
    /// Create a new progress info with the given values.
    pub fn new(completed: u32, total: u32) -> Self {
        Self { completed, total }
    }

    /// Calculate the completion percentage (0-100).
    ///
    /// Returns 0 if total is 0 to avoid division by zero.
    pub fn percentage(&self) -> u32 {
        if self.total == 0 {
            0
        } else {
            (self.completed * 100) / self.total
        }
    }

    /// Check if all tasks are completed.
    pub fn is_complete(&self) -> bool {
        self.total > 0 && self.completed >= self.total
    }

    /// Check if no tasks have been completed yet.
    pub fn is_empty(&self) -> bool {
        self.completed == 0
    }

    /// Get the number of remaining tasks.
    pub fn remaining(&self) -> u32 {
        self.total.saturating_sub(self.completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === ExecutionContext tests ===

    #[test]
    fn test_execution_context_new() {
        let config = OrchestratorConfig::default();
        let ctx = ExecutionContext::new("test-change", &config);

        assert_eq!(ctx.change_id, "test-change");
        assert!(ctx.workspace_path.is_none());
        assert!(ctx.hooks.is_none());
        assert!(ctx.event_tx.is_none());
    }

    #[test]
    fn test_execution_context_is_parallel() {
        let config = OrchestratorConfig::default();
        let ctx = ExecutionContext::new("test-change", &config);
        assert!(!ctx.is_parallel());

        let workspace = std::path::PathBuf::from("/tmp/ws");
        let ctx_parallel = ExecutionContext::new("test-change", &config).with_workspace(&workspace);
        assert!(ctx_parallel.is_parallel());
    }

    #[test]
    fn test_execution_context_working_dir() {
        let config = OrchestratorConfig::default();
        let ctx = ExecutionContext::new("test-change", &config);
        assert!(ctx.working_dir().is_none());

        let workspace = std::path::PathBuf::from("/tmp/ws");
        let ctx_with_ws = ExecutionContext::new("test-change", &config).with_workspace(&workspace);
        assert_eq!(ctx_with_ws.working_dir(), Some(workspace.as_path()));
    }

    // === ExecutionResult tests ===

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult::success();
        assert!(result.is_success());
        assert!(!result.is_failed());
        assert!(!result.is_cancelled());
    }

    #[test]
    fn test_execution_result_failed() {
        let result = ExecutionResult::failed("some error");
        assert!(!result.is_success());
        assert!(result.is_failed());
        assert!(!result.is_cancelled());

        if let ExecutionResult::Failed { message } = result {
            assert_eq!(message, "some error");
        } else {
            panic!("Expected Failed variant");
        }
    }

    #[test]
    fn test_execution_result_cancelled() {
        let result = ExecutionResult::cancelled("user interrupt");
        assert!(!result.is_success());
        assert!(!result.is_failed());
        assert!(result.is_cancelled());

        if let ExecutionResult::Cancelled { reason } = result {
            assert_eq!(reason, "user interrupt");
        } else {
            panic!("Expected Cancelled variant");
        }
    }

    #[test]
    fn test_execution_result_equality() {
        assert_eq!(ExecutionResult::success(), ExecutionResult::Success);
        assert_eq!(
            ExecutionResult::failed("err"),
            ExecutionResult::Failed {
                message: "err".to_string()
            }
        );
        assert_ne!(ExecutionResult::success(), ExecutionResult::failed("err"));
    }

    // === ProgressInfo tests ===

    #[test]
    fn test_progress_info_new() {
        let progress = ProgressInfo::new(5, 10);
        assert_eq!(progress.completed, 5);
        assert_eq!(progress.total, 10);
    }

    #[test]
    fn test_progress_info_default() {
        let progress = ProgressInfo::default();
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.total, 0);
    }

    #[test]
    fn test_progress_info_percentage() {
        assert_eq!(ProgressInfo::new(0, 10).percentage(), 0);
        assert_eq!(ProgressInfo::new(5, 10).percentage(), 50);
        assert_eq!(ProgressInfo::new(10, 10).percentage(), 100);
        assert_eq!(ProgressInfo::new(3, 10).percentage(), 30);
        assert_eq!(ProgressInfo::new(7, 10).percentage(), 70);
    }

    #[test]
    fn test_progress_info_percentage_zero_total() {
        // Should return 0 instead of panicking
        let progress = ProgressInfo::new(0, 0);
        assert_eq!(progress.percentage(), 0);
    }

    #[test]
    fn test_progress_info_percentage_edge_cases() {
        // Test integer division truncation
        assert_eq!(ProgressInfo::new(1, 3).percentage(), 33); // 33.33... -> 33
        assert_eq!(ProgressInfo::new(2, 3).percentage(), 66); // 66.66... -> 66
    }

    #[test]
    fn test_progress_info_is_complete() {
        assert!(!ProgressInfo::new(0, 10).is_complete());
        assert!(!ProgressInfo::new(5, 10).is_complete());
        assert!(ProgressInfo::new(10, 10).is_complete());
        assert!(ProgressInfo::new(11, 10).is_complete()); // Over-completion is also complete
        assert!(!ProgressInfo::new(0, 0).is_complete()); // Empty progress is not complete
    }

    #[test]
    fn test_progress_info_is_empty() {
        assert!(ProgressInfo::new(0, 10).is_empty());
        assert!(ProgressInfo::new(0, 0).is_empty());
        assert!(!ProgressInfo::new(1, 10).is_empty());
        assert!(!ProgressInfo::new(10, 10).is_empty());
    }

    #[test]
    fn test_progress_info_remaining() {
        assert_eq!(ProgressInfo::new(0, 10).remaining(), 10);
        assert_eq!(ProgressInfo::new(5, 10).remaining(), 5);
        assert_eq!(ProgressInfo::new(10, 10).remaining(), 0);
        assert_eq!(ProgressInfo::new(11, 10).remaining(), 0); // Saturating sub
    }

    #[test]
    fn test_progress_info_equality() {
        assert_eq!(ProgressInfo::new(5, 10), ProgressInfo::new(5, 10));
        assert_ne!(ProgressInfo::new(5, 10), ProgressInfo::new(6, 10));
        assert_ne!(ProgressInfo::new(5, 10), ProgressInfo::new(5, 11));
    }
}
