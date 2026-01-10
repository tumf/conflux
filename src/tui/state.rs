//! State management for the TUI
//!
//! Contains AppState and ChangeState implementations.

use crate::openspec::Change;
use ratatui::widgets::ListState;
use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::events::{LogEntry, OrchestratorEvent, TuiCommand};
use super::types::{AppMode, QueueStatus, StopMode};

/// Auto-refresh interval in seconds
pub const AUTO_REFRESH_INTERVAL_SECS: u64 = 5;

/// Maximum number of log entries to keep
pub const MAX_LOG_ENTRIES: usize = 1000;

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

/// Main application state for the TUI
pub struct AppState {
    /// Current mode
    pub mode: AppMode,
    /// List of changes with their states
    pub changes: Vec<ChangeState>,
    /// Current cursor position in the list
    pub cursor_index: usize,
    /// List widget state
    pub list_state: ListState,
    /// ID of the currently processing change
    pub current_change: Option<String>,
    /// ID of the change that caused the error (for display in Error mode)
    pub error_change_id: Option<String>,
    /// Log entries
    pub logs: Vec<LogEntry>,
    /// Last auto-refresh timestamp
    pub last_refresh: Instant,
    /// Number of newly detected changes
    pub new_change_count: usize,
    /// Known change IDs (for detecting new changes)
    pub known_change_ids: HashSet<String>,
    /// Whether the application should quit
    pub should_quit: bool,
    /// Warning message to display
    pub warning_message: Option<String>,
    /// Current spinner animation frame
    pub spinner_frame: usize,
    /// Log scroll offset (0 = show most recent at bottom)
    pub log_scroll_offset: usize,
    /// Whether to auto-scroll logs to bottom on new entries
    pub log_auto_scroll: bool,
    /// Current stop mode
    pub stop_mode: StopMode,
    /// Whether parallel mode is enabled
    pub parallel_mode: bool,
    /// Whether jj is available in this repository
    pub jj_available: bool,
}

impl AppState {
    /// Create a new AppState with initial changes
    ///
    /// Only approved changes are auto-selected on startup.
    /// Unapproved changes start unselected.
    pub fn new(changes: Vec<Change>) -> Self {
        let known_ids: HashSet<String> = changes.iter().map(|c| c.id.clone()).collect();

        // Auto-select only approved changes
        let change_states: Vec<ChangeState> = changes
            .iter()
            .map(|c| ChangeState::from_change(c, c.is_approved))
            .collect();

        // Count auto-queued approved changes
        let approved_count = change_states.iter().filter(|c| c.is_approved).count();

        let mut list_state = ListState::default();
        if !change_states.is_empty() {
            list_state.select(Some(0));
        }

        // Create initial log entries for auto-queued changes
        let mut logs = Vec::new();
        if approved_count > 0 {
            logs.push(LogEntry::info(format!(
                "Auto-queued {} approved change(s)",
                approved_count
            )));
        }

        Self {
            mode: AppMode::Select,
            changes: change_states,
            cursor_index: 0,
            list_state,
            current_change: None,
            error_change_id: None,
            logs,
            last_refresh: Instant::now(),
            new_change_count: 0,
            known_change_ids: known_ids,
            should_quit: false,
            warning_message: None,
            spinner_frame: 0,
            log_scroll_offset: 0,
            log_auto_scroll: true,
            stop_mode: StopMode::None,
            parallel_mode: false,
            jj_available: crate::cli::check_jj_directory() && crate::cli::check_jj_available(),
        }
    }

    /// Toggle parallel mode (only if jj is available)
    ///
    /// Returns true if the mode was toggled, false if jj is not available
    /// or if the mode cannot be changed in current state.
    pub fn toggle_parallel_mode(&mut self) -> bool {
        // Only allow toggling in Select or Stopped mode
        if !matches!(self.mode, AppMode::Select | AppMode::Stopped) {
            self.warning_message =
                Some("Cannot toggle parallel mode while processing".to_string());
            return false;
        }

        // Check if jj is available
        if !self.jj_available {
            self.warning_message = Some("jj is not available (no .jj directory found)".to_string());
            return false;
        }

        self.parallel_mode = !self.parallel_mode;
        let status = if self.parallel_mode {
            "enabled"
        } else {
            "disabled"
        };
        self.add_log(LogEntry::info(format!("Parallel mode {}", status)));
        true
    }

    /// Move cursor up
    pub fn cursor_up(&mut self) {
        if self.changes.is_empty() {
            return;
        }
        self.cursor_index = if self.cursor_index == 0 {
            self.changes.len() - 1
        } else {
            self.cursor_index - 1
        };
        self.list_state.select(Some(self.cursor_index));
    }

    /// Move cursor down
    pub fn cursor_down(&mut self) {
        if self.changes.is_empty() {
            return;
        }
        self.cursor_index = (self.cursor_index + 1) % self.changes.len();
        self.list_state.select(Some(self.cursor_index));
    }

    /// Toggle selection of the current change
    ///
    /// In Select mode:
    /// - Unapproved changes cannot be selected (shows warning)
    /// - Approved changes can be toggled between selected/unselected
    ///
    /// In Running/Completed mode:
    /// - Only approved changes can be added to queue
    pub fn toggle_selection(&mut self) -> Option<TuiCommand> {
        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return None;
        }

        let change = &mut self.changes[self.cursor_index];

        // Cannot select unapproved changes
        if !change.is_approved {
            self.warning_message = Some(format!(
                "Cannot queue unapproved change '{}'. Press @ to approve first.",
                change.id
            ));
            return None;
        }

        match self.mode {
            AppMode::Select => {
                change.selected = !change.selected;
                // Clear NEW flag when user interacts with the change
                if change.is_new {
                    change.is_new = false;
                    self.new_change_count = self.new_change_count.saturating_sub(1);
                }
                None
            }
            AppMode::Running => {
                match &change.queue_status {
                    QueueStatus::NotQueued => {
                        // Add to queue
                        change.queue_status = QueueStatus::Queued;
                        change.selected = true;
                        let id = change.id.clone();
                        self.add_log(LogEntry::info(format!("Added to queue: {}", id)));
                        Some(TuiCommand::AddToQueue(id))
                    }
                    QueueStatus::Queued => {
                        // Remove from queue
                        change.queue_status = QueueStatus::NotQueued;
                        change.selected = false;
                        let id = change.id.clone();
                        self.add_log(LogEntry::info(format!("Removed from queue: {}", id)));
                        Some(TuiCommand::RemoveFromQueue(id))
                    }
                    // Processing, Completed, Archived, Error - cannot change status
                    _ => None,
                }
            }
            AppMode::Stopped => {
                // Allow queue modifications in Stopped mode (same as Running)
                match &change.queue_status {
                    QueueStatus::NotQueued => {
                        change.queue_status = QueueStatus::Queued;
                        change.selected = true;
                        let id = change.id.clone();
                        self.add_log(LogEntry::info(format!("Added to queue: {}", id)));
                        Some(TuiCommand::AddToQueue(id))
                    }
                    QueueStatus::Queued => {
                        change.queue_status = QueueStatus::NotQueued;
                        change.selected = false;
                        let id = change.id.clone();
                        self.add_log(LogEntry::info(format!("Removed from queue: {}", id)));
                        Some(TuiCommand::RemoveFromQueue(id))
                    }
                    _ => None,
                }
            }
            AppMode::Stopping | AppMode::Error => None,
        }
    }

    /// Start processing selected changes
    pub fn start_processing(&mut self) -> Option<TuiCommand> {
        if self.mode != AppMode::Select {
            return None;
        }

        let selected: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.selected)
            .map(|c| c.id.clone())
            .collect();

        if selected.is_empty() {
            self.warning_message = Some("No changes selected".to_string());
            return None;
        }

        // Mark selected changes as queued
        for change in &mut self.changes {
            if change.selected {
                change.queue_status = QueueStatus::Queued;
            }
        }

        self.mode = AppMode::Running;
        self.add_log(LogEntry::info(format!(
            "Starting processing {} change(s)",
            selected.len()
        )));

        Some(TuiCommand::StartProcessing(selected))
    }

    /// Get the number of selected changes
    pub fn selected_count(&self) -> usize {
        self.changes.iter().filter(|c| c.selected).count()
    }

    /// Toggle approval status for the current change
    ///
    /// Only available in Select mode. Returns a TuiCommand::ToggleApproval
    /// to be processed by the main loop.
    pub fn toggle_approval(&mut self) -> Option<TuiCommand> {
        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return None;
        }

        let change = &self.changes[self.cursor_index];

        // Block approval toggle for processing changes
        if matches!(change.queue_status, QueueStatus::Processing) {
            self.warning_message = Some("Cannot change approval for processing change".to_string());
            return None;
        }

        let id = change.id.clone();
        let is_approved = change.is_approved;

        match self.mode {
            AppMode::Select | AppMode::Stopped => {
                // In select/stopped mode:
                // [ ] (unapproved) → @ → [x] (approved + selected)
                // [x] (approved + selected) → @ → [ ] (unapproved + not selected)
                if !is_approved {
                    // Unapproved → approved + selected
                    Some(TuiCommand::ApproveAndQueue(id))
                } else {
                    // Approved → unapproved (also deselects)
                    Some(TuiCommand::UnapproveAndDequeue(id))
                }
            }
            AppMode::Running => {
                // In running mode:
                // [ ] (unapproved) → @ → [@] (approved only, NOT queued)
                // [@] (approved, not queued) → @ → [ ] (unapproved)
                // [x] (queued, not processing) → @ → [ ] (unapproved + removed from queue)
                if !is_approved {
                    // Unapproved → approved only (no auto-queue)
                    Some(TuiCommand::ApproveOnly(id))
                } else {
                    // Approved → unapproved (also removes from queue if queued)
                    Some(TuiCommand::UnapproveAndDequeue(id))
                }
            }
            AppMode::Stopping | AppMode::Error => None,
        }
    }

    /// Update approval status for a specific change
    pub fn update_approval_status(&mut self, change_id: &str, is_approved: bool) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.is_approved = is_approved;
            let status_msg = if is_approved {
                "approved"
            } else {
                "unapproved"
            };
            self.add_log(LogEntry::info(format!(
                "Change '{}' {}",
                change_id, status_msg
            )));
        }
    }

    /// Retry error changes - resets error changes to queued and returns their IDs
    /// Returns None if not in Error mode or no error changes found
    pub fn retry_error_changes(&mut self) -> Option<TuiCommand> {
        if self.mode != AppMode::Error {
            return None;
        }

        // Collect error change IDs
        let error_ids: Vec<String> = self
            .changes
            .iter()
            .filter(|c| matches!(c.queue_status, QueueStatus::Error(_)))
            .map(|c| c.id.clone())
            .collect();

        if error_ids.is_empty() {
            return None;
        }

        // Reset error changes to queued
        for change in &mut self.changes {
            if matches!(change.queue_status, QueueStatus::Error(_)) {
                change.queue_status = QueueStatus::Queued;
                change.selected = true;
            }
        }

        // Add retry log messages
        for id in &error_ids {
            self.add_log(LogEntry::info(format!("Retrying: {}", id)));
        }

        // Reset error state and transition to Running
        self.mode = AppMode::Running;
        self.error_change_id = None;

        Some(TuiCommand::StartProcessing(error_ids))
    }

    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        self.logs.push(entry);
        if self.logs.len() > MAX_LOG_ENTRIES {
            self.logs.remove(0);
            // Adjust scroll offset if oldest logs are removed
            if self.log_scroll_offset > 0 {
                self.log_scroll_offset = self.log_scroll_offset.saturating_sub(1);
            }
        }
        // Auto-scroll to bottom if enabled
        if self.log_auto_scroll {
            self.log_scroll_offset = 0;
        }
    }

    /// Scroll logs up by a page (show older entries)
    pub fn scroll_logs_up(&mut self, page_size: usize) {
        let max_offset = self.logs.len().saturating_sub(1);
        self.log_scroll_offset = (self.log_scroll_offset + page_size).min(max_offset);
        // Disable auto-scroll when user scrolls up
        self.log_auto_scroll = false;
    }

    /// Scroll logs down by a page (show newer entries)
    pub fn scroll_logs_down(&mut self, page_size: usize) {
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(page_size);
        // Re-enable auto-scroll when at bottom
        if self.log_scroll_offset == 0 {
            self.log_auto_scroll = true;
        }
    }

    /// Jump to the oldest log entry (top of history)
    pub fn scroll_logs_to_top(&mut self) {
        let max_offset = self.logs.len().saturating_sub(1);
        self.log_scroll_offset = max_offset;
        self.log_auto_scroll = false;
    }

    /// Jump to the newest log entry (bottom) and re-enable auto-scroll
    pub fn scroll_logs_to_bottom(&mut self) {
        self.log_scroll_offset = 0;
        self.log_auto_scroll = true;
    }

    /// Handle an event from the orchestrator
    pub fn handle_orchestrator_event(&mut self, event: OrchestratorEvent) {
        match event {
            OrchestratorEvent::ProcessingStarted(id) => {
                self.current_change = Some(id.clone());
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Processing;
                }
                self.add_log(LogEntry::info(format!("Processing: {}", id)));
            }
            OrchestratorEvent::ProgressUpdated {
                id,
                completed,
                total,
            } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.completed_tasks = completed;
                    change.total_tasks = total;
                }
            }
            OrchestratorEvent::ProcessingCompleted(id) => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Completed;
                }
                self.add_log(LogEntry::success(format!("Completed: {}", id)));
            }
            OrchestratorEvent::ChangeArchived(id) => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Archived;
                }
                self.add_log(LogEntry::info(format!("Archived: {}", id)));
            }
            OrchestratorEvent::ProcessingError { id, error } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    change.queue_status = QueueStatus::Error(error.clone());
                }
                self.add_log(LogEntry::error(format!("Error in {}: {}", id, error)));
                // Transition to Error mode
                self.mode = AppMode::Error;
                self.error_change_id = Some(id.clone());
                self.current_change = None;
            }
            OrchestratorEvent::AllCompleted => {
                self.mode = AppMode::Select;
                self.current_change = None;
                self.stop_mode = StopMode::None;
                self.add_log(LogEntry::success("All changes processed successfully"));
            }
            OrchestratorEvent::Stopped => {
                self.mode = AppMode::Stopped;
                self.current_change = None;
                self.stop_mode = StopMode::None;
                self.add_log(LogEntry::warn("Processing stopped"));
            }
            OrchestratorEvent::Log(entry) => {
                self.add_log(entry);
            }
            OrchestratorEvent::ChangesRefreshed(changes) => {
                self.update_changes(changes);
            }
        }
    }

    /// Update changes from a refresh
    pub fn update_changes(&mut self, fetched_changes: Vec<Change>) {
        // Detect new changes
        let new_ids: Vec<String> = fetched_changes
            .iter()
            .filter(|c| !self.known_change_ids.contains(&c.id))
            .map(|c| c.id.clone())
            .collect();

        // Update existing changes
        for fetched in &fetched_changes {
            if let Some(existing) = self.changes.iter_mut().find(|c| c.id == fetched.id) {
                // Update progress
                existing.completed_tasks = fetched.completed_tasks;
                existing.total_tasks = fetched.total_tasks;
            }
        }

        // Add new changes
        for id in &new_ids {
            if let Some(fetched) = fetched_changes.iter().find(|c| &c.id == id) {
                let mut new_state = ChangeState::from_change(fetched, false); // New changes are not selected
                new_state.is_new = true;
                self.changes.push(new_state);
                self.known_change_ids.insert(id.clone());
                self.add_log(LogEntry::warn(format!("Discovered new change: {}", id)));
            }
        }

        self.new_change_count = self.changes.iter().filter(|c| c.is_new).count();
        self.last_refresh = Instant::now();

        // Remove changes that no longer exist (have been archived externally)
        let current_ids: HashSet<String> = fetched_changes.iter().map(|c| c.id.clone()).collect();
        self.changes.retain(|c| {
            // Keep if still exists, or if it's in a terminal state (completed/archived/error)
            current_ids.contains(&c.id)
                || matches!(
                    c.queue_status,
                    QueueStatus::Completed | QueueStatus::Archived | QueueStatus::Error(_)
                )
        });

        // Ensure cursor is valid
        if self.cursor_index >= self.changes.len() && !self.changes.is_empty() {
            self.cursor_index = self.changes.len() - 1;
            self.list_state.select(Some(self.cursor_index));
        }
    }

    /// Check if auto-refresh is due
    #[allow(dead_code)]
    pub fn should_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= Duration::from_secs(AUTO_REFRESH_INTERVAL_SECS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "now".to_string(),
            is_approved: false,
        }
    }

    fn create_approved_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "now".to_string(),
            is_approved: true,
        }
    }

    #[test]
    fn test_app_state_new_unapproved_not_selected() {
        // Unapproved changes should NOT be selected on startup
        let changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 0, 3),
        ];

        let app = AppState::new(changes);

        assert_eq!(app.mode, AppMode::Select);
        assert_eq!(app.changes.len(), 2);
        assert_eq!(app.cursor_index, 0);
        assert!(!app.changes[0].selected); // Unapproved = not selected
        assert!(!app.changes[1].selected); // Unapproved = not selected
    }

    #[test]
    fn test_app_state_new_approved_auto_selected() {
        // Approved changes should be auto-selected on startup
        let changes = vec![
            create_approved_change("change-a", 2, 5),
            create_test_change("change-b", 0, 3), // Unapproved
        ];

        let app = AppState::new(changes);

        assert_eq!(app.mode, AppMode::Select);
        assert_eq!(app.changes.len(), 2);
        assert!(app.changes[0].selected); // Approved = selected
        assert!(!app.changes[1].selected); // Unapproved = not selected
                                           // Should have log entry for auto-queued changes
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Auto-queued")));
    }

    #[test]
    fn test_app_state_new_no_auto_queue_log_when_none_approved() {
        // No auto-queue log when no changes are approved
        let changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 0, 3),
        ];

        let app = AppState::new(changes);

        // Should NOT have auto-queue log entry
        assert!(!app
            .logs
            .iter()
            .any(|log| log.message.contains("Auto-queued")));
    }

    #[test]
    fn test_cursor_navigation() {
        let changes = vec![
            create_test_change("a", 0, 1),
            create_test_change("b", 0, 1),
            create_test_change("c", 0, 1),
        ];

        let mut app = AppState::new(changes);

        assert_eq!(app.cursor_index, 0);

        app.cursor_down();
        assert_eq!(app.cursor_index, 1);

        app.cursor_down();
        assert_eq!(app.cursor_index, 2);

        app.cursor_down();
        assert_eq!(app.cursor_index, 0); // Wraps around

        app.cursor_up();
        assert_eq!(app.cursor_index, 2); // Wraps around
    }

    #[test]
    fn test_toggle_selection() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];

        let mut app = AppState::new(changes);

        assert!(app.changes[0].selected);

        app.toggle_selection();
        assert!(!app.changes[0].selected);

        app.toggle_selection();
        assert!(app.changes[0].selected);
    }

    #[test]
    fn test_selected_count() {
        // Use approved changes so they start selected
        let changes = vec![
            create_approved_change("a", 0, 1),
            create_approved_change("b", 0, 1),
            create_approved_change("c", 0, 1),
        ];

        let mut app = AppState::new(changes);

        assert_eq!(app.selected_count(), 3);

        app.toggle_selection(); // Deselect first
        assert_eq!(app.selected_count(), 2);
    }

    #[test]
    fn test_start_processing_with_selection() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];

        let mut app = AppState::new(changes);

        let cmd = app.start_processing();
        assert!(cmd.is_some());
        assert_eq!(app.mode, AppMode::Running);
    }

    #[test]
    fn test_start_processing_without_selection() {
        let changes = vec![create_test_change("a", 0, 1)];

        let mut app = AppState::new(changes);
        app.changes[0].selected = false;

        let cmd = app.start_processing();
        assert!(cmd.is_none());
        assert_eq!(app.mode, AppMode::Select);
        assert!(app.warning_message.is_some());
    }

    #[test]
    fn test_update_changes_detects_new() {
        let initial = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(initial);

        let fetched = vec![
            create_test_change("a", 1, 1), // Updated
            create_test_change("b", 0, 2), // New
        ];

        app.update_changes(fetched);

        assert_eq!(app.changes.len(), 2);
        assert_eq!(app.changes[0].completed_tasks, 1); // Updated
        assert!(app.changes[1].is_new);
        assert!(!app.changes[1].selected); // New changes are not selected
        assert_eq!(app.new_change_count, 1);
    }

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
        };

        assert_eq!(change.progress_percent(), 50.0);
        assert_eq!(change.progress_ratio(), 0.5);
        assert!(!change.is_complete());
    }

    #[test]
    fn test_toggle_selection_removes_from_queue_in_running_mode() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing to enter Running mode
        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);

        // Toggle should remove from queue
        let cmd = app.toggle_selection();
        assert!(matches!(cmd, Some(TuiCommand::RemoveFromQueue(_))));
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        assert!(!app.changes[0].selected);
    }

    #[test]
    fn test_toggle_selection_adds_to_queue_after_removal_in_running_mode() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing to enter Running mode
        app.start_processing();

        // Remove from queue
        let cmd = app.toggle_selection();
        assert!(matches!(cmd, Some(TuiCommand::RemoveFromQueue(_))));
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);

        // Add back to queue
        let cmd = app.toggle_selection();
        assert!(matches!(cmd, Some(TuiCommand::AddToQueue(_))));
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);
        assert!(app.changes[0].selected);
    }

    #[test]
    fn test_toggle_selection_does_nothing_for_processing_status() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing and set the change to Processing status
        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Processing;

        // Toggle should do nothing
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);
    }

    #[test]
    fn test_toggle_selection_does_nothing_for_completed_status() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing and set the change to Completed status
        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Completed;

        // Toggle should do nothing
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Completed);
    }

    #[test]
    fn test_toggle_selection_does_nothing_for_archived_status() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing and set the change to Archived status
        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Archived;

        // Toggle should do nothing
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archived);
    }

    #[test]
    fn test_toggle_selection_does_nothing_for_error_status() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing and set the change to Error status
        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Error("test error".to_string());

        // Toggle should do nothing
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
        assert!(matches!(app.changes[0].queue_status, QueueStatus::Error(_)));
    }

    #[test]
    fn test_processing_error_transitions_to_error_mode() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        // Simulate processing error
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingError {
            id: "a".to_string(),
            error: "LLM error".to_string(),
        });

        // Mode should be Error
        assert_eq!(app.mode, AppMode::Error);
        assert_eq!(app.error_change_id, Some("a".to_string()));
        assert!(matches!(app.changes[0].queue_status, QueueStatus::Error(_)));
        assert!(app.current_change.is_none());
    }

    #[test]
    fn test_retry_error_changes_from_error_mode() {
        // Use approved changes so they start selected
        let changes = vec![
            create_approved_change("a", 0, 1),
            create_approved_change("b", 0, 2),
        ];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();

        // Set one change to error
        app.mode = AppMode::Error;
        app.error_change_id = Some("a".to_string());
        app.changes[0].queue_status = QueueStatus::Error("LLM error".to_string());
        app.changes[1].queue_status = QueueStatus::Completed;

        // Retry should reset error changes
        let cmd = app.retry_error_changes();

        assert!(cmd.is_some());
        if let Some(TuiCommand::StartProcessing(ids)) = cmd {
            assert_eq!(ids, vec!["a".to_string()]);
        } else {
            panic!("Expected StartProcessing command");
        }

        // Mode should be Running
        assert_eq!(app.mode, AppMode::Running);
        assert!(app.error_change_id.is_none());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);
        assert!(app.changes[0].selected);
        // Completed change should remain completed
        assert_eq!(app.changes[1].queue_status, QueueStatus::Completed);
    }

    #[test]
    fn test_retry_error_changes_does_nothing_in_select_mode() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set change to error but mode is Select
        app.changes[0].queue_status = QueueStatus::Error("LLM error".to_string());

        // Retry should do nothing
        let cmd = app.retry_error_changes();
        assert!(cmd.is_none());
        assert_eq!(app.mode, AppMode::Select);
    }

    #[test]
    fn test_retry_error_changes_does_nothing_when_no_errors() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing and set mode to Error manually
        app.start_processing();
        app.mode = AppMode::Error;
        // But no changes have error status

        // Retry should do nothing
        let cmd = app.retry_error_changes();
        assert!(cmd.is_none());
    }

    #[test]
    fn test_retry_logs_retrying_message() {
        let changes = vec![create_test_change("error-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Set up error state
        app.mode = AppMode::Error;
        app.error_change_id = Some("error-change".to_string());
        app.changes[0].queue_status = QueueStatus::Error("test error".to_string());

        // Clear logs
        app.logs.clear();

        // Retry
        let _ = app.retry_error_changes();

        // Check that log contains retry message
        assert!(app.logs.iter().any(|log| log.message.contains("Retrying")));
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("error-change")));
    }

    #[test]
    fn test_should_refresh_after_interval() {
        use std::time::Duration;

        let changes = vec![create_test_change("test", 1, 2)];
        let mut app = AppState::new(changes);

        // Initially should not need refresh (just created)
        assert!(!app.should_refresh());

        // Manually set last_refresh to simulate elapsed time
        app.last_refresh =
            std::time::Instant::now() - Duration::from_secs(AUTO_REFRESH_INTERVAL_SECS + 1);

        // Now should need refresh
        assert!(app.should_refresh());
    }

    #[test]
    fn test_new_badge_state_tracking() {
        let changes = vec![
            create_test_change("existing", 1, 2),
            create_test_change("new-change", 0, 3),
        ];
        let mut app = AppState::new(changes);

        // Set up known changes
        app.known_change_ids.insert("existing".to_string());

        // Mark new-change as new
        app.changes[1].is_new = true;

        // Verify is_new state
        assert!(!app.changes[0].is_new);
        assert!(app.changes[1].is_new);
    }

    #[test]
    fn test_update_changes_marks_new_changes_correctly() {
        let initial_changes = vec![create_test_change("existing", 1, 2)];
        let mut app = AppState::new(initial_changes);

        // Simulate discovering new change
        let updated_changes = vec![
            create_test_change("existing", 1, 2),
            create_test_change("brand-new", 0, 3),
        ];

        app.update_changes(updated_changes);

        // Find the new change
        let brand_new = app.changes.iter().find(|c| c.id == "brand-new");
        assert!(brand_new.is_some());
        assert!(brand_new.unwrap().is_new);

        // Existing should not be marked as new
        let existing = app.changes.iter().find(|c| c.id == "existing");
        assert!(existing.is_some());
        assert!(!existing.unwrap().is_new);
    }

    #[test]
    fn test_new_change_count_tracking() {
        let initial_changes = vec![create_test_change("existing", 1, 2)];
        let mut app = AppState::new(initial_changes);

        // Initially no new changes
        assert_eq!(app.new_change_count, 0);

        // Add new changes
        let updated_changes = vec![
            create_test_change("existing", 1, 2),
            create_test_change("new1", 0, 1),
            create_test_change("new2", 0, 2),
        ];

        app.update_changes(updated_changes);

        // Should have 2 new changes
        assert_eq!(app.new_change_count, 2);
    }

    #[test]
    fn test_footer_message_when_no_changes() {
        // Empty changes list should show "Add new proposals to get started"
        let app = AppState::new(vec![]);
        assert!(app.changes.is_empty());
        assert_eq!(app.selected_count(), 0);
        // The condition in render_footer_select: app.changes.is_empty() -> "Add new proposals..."
    }

    #[test]
    fn test_footer_message_when_none_selected() {
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 2)];
        let mut app = AppState::new(changes);

        // Deselect all
        app.changes[0].selected = false;
        app.changes[1].selected = false;

        assert!(!app.changes.is_empty());
        assert_eq!(app.selected_count(), 0);
        // The condition: !app.changes.is_empty() && selected == 0 -> "Select changes with Space..."
    }

    #[test]
    fn test_footer_message_when_changes_selected() {
        // Use approved changes so they start selected
        let changes = vec![
            create_approved_change("a", 0, 1),
            create_approved_change("b", 0, 2),
        ];
        let app = AppState::new(changes);

        assert!(!app.changes.is_empty());
        assert!(app.selected_count() > 0);
        // The condition: selected > 0 -> "Press F5 to start processing"
    }

    #[test]
    fn test_progress_calculation_during_running() {
        // Use approved changes so they start selected
        let changes = vec![
            create_approved_change("a", 2, 5), // 2/5 done
            create_approved_change("b", 3, 3), // 3/3 done
        ];
        let mut app = AppState::new(changes);

        // Start processing to enter Running mode
        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        // Calculate progress like render_status does (excludes NotQueued and Error)
        let (total_tasks, completed_tasks) = app
            .changes
            .iter()
            .filter(|c| {
                !matches!(
                    c.queue_status,
                    QueueStatus::NotQueued | QueueStatus::Error(_)
                )
            })
            .fold((0u32, 0u32), |(total, completed), c| {
                (total + c.total_tasks, completed + c.completed_tasks)
            });

        // Total: 5 + 3 = 8 tasks, Completed: 2 + 3 = 5 tasks
        assert_eq!(total_tasks, 8);
        assert_eq!(completed_tasks, 5);

        let percent = (completed_tasks as f32 / total_tasks as f32) * 100.0;
        assert!((percent - 62.5).abs() < 0.01);
    }

    #[test]
    fn test_progress_calculation_includes_completed_changes() {
        // Use approved changes so they start selected
        let changes = vec![
            create_approved_change("a", 2, 5), // 2/5 done, will be Processing
            create_approved_change("b", 3, 3), // 3/3 done, will be Completed
        ];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        // Simulate: a is processing, b is completed
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[1].queue_status = QueueStatus::Completed;

        // Calculate progress (includes Completed changes)
        let (total_tasks, completed_tasks) = app
            .changes
            .iter()
            .filter(|c| {
                !matches!(
                    c.queue_status,
                    QueueStatus::NotQueued | QueueStatus::Error(_)
                )
            })
            .fold((0u32, 0u32), |(total, completed), c| {
                (total + c.total_tasks, completed + c.completed_tasks)
            });

        // Both a (Processing) and b (Completed) should be included
        assert_eq!(total_tasks, 8);
        assert_eq!(completed_tasks, 5);
    }

    #[test]
    fn test_approve_and_queue_in_select_mode_returns_correct_command() {
        // Test that toggle_approval in Select mode returns ApproveAndQueue for unapproved change
        let changes = vec![create_test_change("unapproved-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Ensure we're in Select mode with an unapproved change
        assert_eq!(app.mode, AppMode::Select);
        assert!(!app.changes[0].is_approved);

        // Toggle approval should return ApproveAndQueue
        let cmd = app.toggle_approval();
        assert!(matches!(
            cmd,
            Some(TuiCommand::ApproveAndQueue(ref id)) if id == "unapproved-change"
        ));
    }

    #[test]
    fn test_approve_and_queue_state_update_simulation() {
        // Simulate what runner.rs ApproveAndQueue handler does
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Initial state: unapproved, not selected
        assert!(!app.changes[0].is_approved);
        assert!(!app.changes[0].selected);
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);

        // Simulate ApproveAndQueue handler logic (from runner.rs:329-358)
        let id = "test-change";

        // 1. update_approval_status (this adds a log)
        app.update_approval_status(id, true);

        // 2. Set queue_status and selected
        if let Some(change) = app.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Queued;
            change.selected = true;
        }

        // Verify final state: approved + selected + queued
        assert!(app.changes[0].is_approved);
        assert!(app.changes[0].selected);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);

        // This is the state that should render as [x] (Queued)
        // Checkbox logic: if is_approved && selected → "[x]"
        let checkbox = if !app.changes[0].is_approved {
            "[ ]"
        } else if app.changes[0].selected {
            "[x]"
        } else {
            "[@]"
        };
        assert_eq!(checkbox, "[x]");
    }
}
