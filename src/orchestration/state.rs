//! Shared state management for orchestration operations.
//!
//! Provides a unified state structure that tracks orchestration progress
//! across different execution modes (serial CLI, TUI, parallel).
//!
//! ## Integration Status
//!
//! - **Serial Orchestrator** (`src/orchestrator.rs`): Fully integrated. The orchestrator
//!   maintains a `shared_state: Arc<RwLock<OrchestratorState>>` instance and updates it
//!   via `apply_execution_event` when processing changes (ProcessingStarted, ApplyStarted,
//!   ApplyCompleted, ChangeArchived). The shared state is wrapped in Arc<RwLock<>> to enable
//!   sharing with TUI and Web monitoring.
//!
//! - **TUI** (`src/tui/state/mod.rs`): Integrated via optional reference. TUI AppState has
//!   a `shared_orchestrator_state` field that can be set via `set_shared_state()`. TUI can
//!   query this for pending/archived status, apply counts, and current change tracking while
//!   maintaining its own UI-specific state for rendering and interaction.
//!
//! - **Web** (`src/web/state.rs`): Integrated via optional reference. WebState has a
//!   `shared_orchestrator_state` field set via `set_shared_state()` (called automatically
//!   by `Orchestrator::set_web_state()`). When generating `OrchestratorStateSnapshot` via
//!   `from_changes_with_shared_state()`, WebState queries shared state to enrich change
//!   metadata with apply counts, pending/archived status, and iteration numbers.
//!
//! ## Usage
//!
//! The shared state provides a single source of truth for tracking:
//! - Pending, completed, and archived changes
//! - Apply counts per change
//! - Current change being processed
//! - Iteration counters and limits
//!
//! ### Integration Pattern
//!
//! 1. **Orchestrator creates and owns shared state:**
//!    ```rust,ignore
//!    let shared_state = Arc::new(RwLock::new(OrchestratorState::new(changes, max_iters)));
//!    ```
//!
//! 2. **Orchestrator updates state via events:**
//!    ```rust,ignore
//!    shared_state.write().await.apply_execution_event(&event);
//!    ```
//!
//! 3. **TUI/Web receive shared state reference:**
//!    ```rust,ignore
//!    app_state.set_shared_state(shared_state.clone());
//!    web_state.set_shared_state(shared_state.clone()).await;
//!    ```
//!
//! 4. **TUI/Web query shared state when needed:**
//!    ```rust,ignore
//!    if let Some(shared) = &app_state.shared_orchestrator_state {
//!        let guard = shared.read().await;
//!        let apply_count = guard.apply_count(change_id);
//!        let is_pending = guard.is_pending(change_id);
//!    }
//!    ```

use std::collections::{HashMap, HashSet};

/// Shared state for orchestration operations.
///
/// This structure tracks:
/// - Which changes are pending, completed, or archived
/// - Apply counts per change
/// - Iteration counters and progress
#[derive(Debug, Clone)]
pub struct OrchestratorState {
    /// Change IDs captured at run start (snapshot).
    /// Only changes present in this snapshot will be processed.
    initial_change_ids: HashSet<String>,

    /// Changes that are still pending (not yet archived).
    pending_changes: HashSet<String>,

    /// Changes that have been archived.
    archived_changes: HashSet<String>,

    /// Apply counts per change (how many times each change has been applied).
    apply_counts: HashMap<String, u32>,

    /// Task progress per change (completed_tasks, total_tasks).
    task_progress: HashMap<String, (u32, u32)>,

    /// Number of changes processed (archived).
    changes_processed: usize,

    /// Total number of changes at run start.
    total_changes: usize,

    /// Maximum iterations limit (0 = no limit).
    max_iterations: u32,

    /// Current iteration number.
    iteration: u32,

    /// Current change ID being processed.
    current_change_id: Option<String>,
}

#[allow(dead_code)] // Public API for future use by TUI/Web states
impl OrchestratorState {
    /// Create a new orchestrator state with the given initial changes.
    pub fn new(change_ids: Vec<String>, max_iterations: u32) -> Self {
        let initial_set: HashSet<String> = change_ids.iter().cloned().collect();
        let pending_set = initial_set.clone();
        let total = change_ids.len();

        Self {
            initial_change_ids: initial_set,
            pending_changes: pending_set,
            archived_changes: HashSet::new(),
            apply_counts: HashMap::new(),
            task_progress: HashMap::new(),
            changes_processed: 0,
            total_changes: total,
            max_iterations,
            iteration: 0,
            current_change_id: None,
        }
    }

    /// Get the initial snapshot of change IDs.
    pub fn initial_change_ids(&self) -> &HashSet<String> {
        &self.initial_change_ids
    }

    /// Get the set of pending changes.
    pub fn pending_changes(&self) -> &HashSet<String> {
        &self.pending_changes
    }

    /// Get the set of archived changes.
    pub fn archived_changes(&self) -> &HashSet<String> {
        &self.archived_changes
    }

    /// Get the number of changes processed.
    pub fn changes_processed(&self) -> usize {
        self.changes_processed
    }

    /// Get the total number of changes.
    pub fn total_changes(&self) -> usize {
        self.total_changes
    }

    /// Get the number of remaining changes.
    pub fn remaining_changes(&self) -> usize {
        self.pending_changes.len()
    }

    /// Get the current iteration number.
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// Get the maximum iterations limit.
    pub fn max_iterations(&self) -> u32 {
        self.max_iterations
    }

    /// Get the current change ID being processed.
    pub fn current_change_id(&self) -> Option<&String> {
        self.current_change_id.as_ref()
    }

    /// Get the apply count for a specific change.
    pub fn apply_count(&self, change_id: &str) -> u32 {
        *self.apply_counts.get(change_id).unwrap_or(&0)
    }

    /// Get the task progress for a specific change (completed_tasks, total_tasks).
    pub fn task_progress(&self, change_id: &str) -> (u32, u32) {
        *self.task_progress.get(change_id).unwrap_or(&(0, 0))
    }

    /// Update task progress for a change.
    pub fn set_task_progress(&mut self, change_id: String, completed: u32, total: u32) {
        self.task_progress.insert(change_id, (completed, total));
    }

    /// Check if a change is in the initial snapshot.
    pub fn is_in_snapshot(&self, change_id: &str) -> bool {
        self.initial_change_ids.contains(change_id)
    }

    /// Check if a change is pending.
    pub fn is_pending(&self, change_id: &str) -> bool {
        self.pending_changes.contains(change_id)
    }

    /// Check if a change is archived.
    pub fn is_archived(&self, change_id: &str) -> bool {
        self.archived_changes.contains(change_id)
    }

    /// Check if all changes are done.
    pub fn is_complete(&self) -> bool {
        self.pending_changes.is_empty()
    }

    /// Check if max iterations has been reached.
    pub fn is_iteration_limit_reached(&self) -> bool {
        self.max_iterations > 0 && self.iteration >= self.max_iterations
    }

    /// Check if we're approaching the iteration limit (80%).
    pub fn is_approaching_iteration_limit(&self) -> bool {
        if self.max_iterations == 0 {
            return false;
        }
        let threshold = (self.max_iterations as f32 * 0.8) as u32;
        self.iteration == threshold
    }

    /// Increment the iteration counter.
    pub fn increment_iteration(&mut self) {
        self.iteration += 1;
    }

    /// Set the current change being processed.
    pub fn set_current_change(&mut self, change_id: Option<String>) {
        self.current_change_id = change_id;
    }

    /// Increment the apply count for a change and return the new count.
    pub fn increment_apply_count(&mut self, change_id: &str) -> u32 {
        let count = self.apply_counts.entry(change_id.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    /// Mark a change as archived.
    ///
    /// This:
    /// - Moves the change from pending to archived
    /// - Increments the changes_processed counter
    /// - Clears the current change if it matches
    /// - Removes apply count tracking
    pub fn mark_archived(&mut self, change_id: &str) {
        if self.pending_changes.remove(change_id) {
            self.archived_changes.insert(change_id.to_string());
            self.changes_processed += 1;
            self.apply_counts.remove(change_id);

            if self.current_change_id.as_deref() == Some(change_id) {
                self.current_change_id = None;
            }
        }
    }

    /// Add a new change dynamically (during execution).
    ///
    /// This is used for dynamic queue support in TUI mode.
    pub fn add_dynamic_change(&mut self, change_id: String) {
        if !self.initial_change_ids.contains(&change_id)
            && !self.pending_changes.contains(&change_id)
            && !self.archived_changes.contains(&change_id)
        {
            self.initial_change_ids.insert(change_id.clone());
            self.pending_changes.insert(change_id);
            self.total_changes += 1;
        }
    }

    /// Remove a change from pending (e.g., due to failure).
    pub fn remove_from_pending(&mut self, change_id: &str) {
        self.pending_changes.remove(change_id);
        if self.current_change_id.as_deref() == Some(change_id) {
            self.current_change_id = None;
        }
    }

    /// Apply an ExecutionEvent to update the shared state.
    ///
    /// This is the single source of truth for state mutations driven by execution events.
    ///
    /// ## Current Usage
    ///
    /// - **Serial Orchestrator**: Calls this method in `src/orchestrator.rs` to track:
    ///   - `ProcessingStarted` - When a change begins processing
    ///   - `ApplyStarted` - When apply operation starts
    ///   - `ApplyCompleted` - When apply operation completes (increments apply count)
    ///   - `ChangeArchived` - When a change is successfully archived
    ///
    /// - **TUI/Web**: Currently maintain their own ExecutionEvent-driven state independently.
    ///   Future refactoring can make them query this shared state for unified tracking.
    pub fn apply_execution_event(&mut self, event: &crate::events::ExecutionEvent) {
        use crate::events::ExecutionEvent;

        match event {
            // Processing lifecycle
            ExecutionEvent::ProcessingStarted(change_id) => {
                self.set_current_change(Some(change_id.clone()));
            }
            ExecutionEvent::ProcessingCompleted(change_id) => {
                // Keep current_change_id set until archived
                let _ = change_id;
            }
            ExecutionEvent::ProcessingError { id, error: _ } => {
                self.remove_from_pending(id);
            }

            // Apply events
            ExecutionEvent::ApplyStarted {
                change_id,
                command: _,
            } => {
                self.set_current_change(Some(change_id.clone()));
            }
            ExecutionEvent::ApplyCompleted { change_id, .. } => {
                self.increment_apply_count(change_id);
            }
            ExecutionEvent::ApplyFailed { change_id, .. } => {
                self.remove_from_pending(change_id);
            }

            // Archive events
            ExecutionEvent::ChangeArchived(change_id) => {
                self.mark_archived(change_id);
            }

            // Dynamic queue support
            ExecutionEvent::ChangesRefreshed { changes, .. } => {
                // Refresh the initial snapshot if new changes appeared
                for change in changes {
                    if !self.initial_change_ids.contains(&change.id)
                        && !self.archived_changes.contains(&change.id)
                    {
                        self.add_dynamic_change(change.id.clone());
                    }
                }
            }

            // Other events don't affect shared state directly
            _ => {}
        }
    }
}

impl Default for OrchestratorState {
    fn default() -> Self {
        Self::new(Vec::new(), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let state =
            OrchestratorState::new(vec!["change-a".to_string(), "change-b".to_string()], 10);

        assert_eq!(state.total_changes(), 2);
        assert_eq!(state.remaining_changes(), 2);
        assert_eq!(state.changes_processed(), 0);
        assert_eq!(state.iteration(), 0);
        assert_eq!(state.max_iterations(), 10);
        assert!(state.current_change_id().is_none());
    }

    #[test]
    fn test_is_in_snapshot() {
        let state = OrchestratorState::new(vec!["change-a".to_string()], 0);

        assert!(state.is_in_snapshot("change-a"));
        assert!(!state.is_in_snapshot("change-b"));
    }

    #[test]
    fn test_apply_count_increment() {
        let mut state = OrchestratorState::new(vec!["change-a".to_string()], 0);

        assert_eq!(state.apply_count("change-a"), 0);
        assert_eq!(state.increment_apply_count("change-a"), 1);
        assert_eq!(state.increment_apply_count("change-a"), 2);
        assert_eq!(state.apply_count("change-a"), 2);
    }

    #[test]
    fn test_mark_archived() {
        let mut state =
            OrchestratorState::new(vec!["change-a".to_string(), "change-b".to_string()], 0);
        state.set_current_change(Some("change-a".to_string()));
        state.increment_apply_count("change-a");

        assert!(state.is_pending("change-a"));
        assert!(!state.is_archived("change-a"));
        assert_eq!(state.remaining_changes(), 2);

        state.mark_archived("change-a");

        assert!(!state.is_pending("change-a"));
        assert!(state.is_archived("change-a"));
        assert_eq!(state.remaining_changes(), 1);
        assert_eq!(state.changes_processed(), 1);
        assert!(state.current_change_id().is_none());
        assert_eq!(state.apply_count("change-a"), 0); // Cleared
    }

    #[test]
    fn test_is_complete() {
        let mut state = OrchestratorState::new(vec!["change-a".to_string()], 0);

        assert!(!state.is_complete());
        state.mark_archived("change-a");
        assert!(state.is_complete());
    }

    #[test]
    fn test_iteration_limit() {
        let mut state = OrchestratorState::new(vec![], 10);

        assert!(!state.is_iteration_limit_reached());

        for _ in 0..8 {
            state.increment_iteration();
        }
        assert!(state.is_approaching_iteration_limit()); // 80%
        assert!(!state.is_iteration_limit_reached());

        state.increment_iteration(); // 9
        assert!(!state.is_iteration_limit_reached());

        state.increment_iteration(); // 10
        assert!(state.is_iteration_limit_reached());
    }

    #[test]
    fn test_no_iteration_limit() {
        let mut state = OrchestratorState::new(vec![], 0);

        for _ in 0..100 {
            state.increment_iteration();
        }

        assert!(!state.is_iteration_limit_reached());
        assert!(!state.is_approaching_iteration_limit());
    }

    #[test]
    fn test_add_dynamic_change() {
        let mut state = OrchestratorState::new(vec!["change-a".to_string()], 0);

        assert_eq!(state.total_changes(), 1);
        assert!(!state.is_pending("change-b"));

        state.add_dynamic_change("change-b".to_string());

        assert_eq!(state.total_changes(), 2);
        assert!(state.is_pending("change-b"));
        assert!(state.is_in_snapshot("change-b"));
    }

    #[test]
    fn test_add_dynamic_change_idempotent() {
        let mut state = OrchestratorState::new(vec!["change-a".to_string()], 0);

        state.add_dynamic_change("change-a".to_string()); // Already exists
        state.add_dynamic_change("change-b".to_string());
        state.add_dynamic_change("change-b".to_string()); // Duplicate

        assert_eq!(state.total_changes(), 2); // Not 3 or 4
    }

    #[test]
    fn test_remove_from_pending() {
        let mut state = OrchestratorState::new(vec!["change-a".to_string()], 0);
        state.set_current_change(Some("change-a".to_string()));

        assert!(state.is_pending("change-a"));
        assert!(state.current_change_id().is_some());

        state.remove_from_pending("change-a");

        assert!(!state.is_pending("change-a"));
        assert!(!state.is_archived("change-a")); // Not archived, just removed
        assert!(state.current_change_id().is_none());
    }
}
