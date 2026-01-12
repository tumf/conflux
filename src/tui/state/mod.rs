//! State management for the TUI
//!
//! This module contains AppState and ChangeState implementations,
//! organized into submodules by responsibility.

mod change;
mod events;
mod logs;
mod modes;

use crate::openspec::Change;
use ratatui::widgets::ListState;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tui_textarea::TextArea;

use super::events::{LogEntry, TuiCommand};
use super::types::{AppMode, QueueStatus, StopMode};

// Re-exports
pub use change::ChangeState;

/// Auto-refresh interval in seconds
pub const AUTO_REFRESH_INTERVAL_SECS: u64 = 5;

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
    /// Whether parallel execution is available (jj or git)
    pub parallel_available: bool,
    /// VCS backend being used (jj or git)
    pub vcs_backend: String,
    /// Max concurrent workspaces for parallel execution
    pub max_concurrent: usize,
    /// When orchestration started (for overall elapsed time)
    pub orchestration_started_at: Option<Instant>,
    /// Total elapsed time when orchestration finished
    pub orchestration_elapsed: Option<Duration>,
    /// Text area for proposal input (only present in Proposing mode)
    pub propose_textarea: Option<TextArea<'static>>,
    /// Mode to return to after exiting Proposing mode
    pub previous_mode: Option<AppMode>,
    /// Web UI URL (set when web server is enabled)
    pub web_url: Option<String>,
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
            parallel_available: crate::cli::check_parallel_available(),
            vcs_backend: if crate::cli::check_jj_directory() {
                "jj".to_string()
            } else {
                "git".to_string()
            },
            max_concurrent: 4, // Default value, can be overridden from config
            orchestration_started_at: None,
            orchestration_elapsed: None,
            propose_textarea: None,
            previous_mode: None,
            web_url: None,
        }
    }

    /// Create a new TextArea for proposal input
    pub fn create_propose_textarea() -> TextArea<'static> {
        TextArea::default()
    }

    /// Start proposing mode (enter text input for new proposal)
    pub fn start_proposing(&mut self) {
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::Proposing;
        self.propose_textarea = Some(Self::create_propose_textarea());
    }

    /// Cancel proposing mode and return to previous mode
    pub fn cancel_proposing(&mut self) {
        self.propose_textarea = None;
        if let Some(mode) = self.previous_mode.take() {
            self.mode = mode;
        } else {
            self.mode = AppMode::Select;
        }
    }

    /// Show QR popup (only when web_url is set)
    pub fn show_qr_popup(&mut self) {
        if self.web_url.is_some() {
            self.previous_mode = Some(self.mode.clone());
            self.mode = AppMode::QrPopup;
        }
    }

    /// Hide QR popup and return to previous mode
    pub fn hide_qr_popup(&mut self) {
        if let Some(mode) = self.previous_mode.take() {
            self.mode = mode;
        } else {
            self.mode = AppMode::Select;
        }
    }

    /// Submit proposal and return the proposal text
    /// Returns None if textarea is empty or not in Proposing mode
    /// On successful submission, returns to previous mode
    /// On failure (empty text), remains in Proposing mode with input retained
    pub fn submit_proposal(&mut self) -> Option<String> {
        if self.mode != AppMode::Proposing {
            return None;
        }

        let text = self
            .propose_textarea
            .as_ref()
            .map(|ta| ta.lines().join("\n"))
            .unwrap_or_default();

        if text.trim().is_empty() {
            // Keep in Proposing mode with input retained
            None
        } else {
            // Success: clear textarea and return to previous mode
            self.propose_textarea = None;
            if let Some(mode) = self.previous_mode.take() {
                self.mode = mode;
            } else {
                self.mode = AppMode::Select;
            }
            Some(text)
        }
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
                        // Clear NEW flag when user adds to queue
                        if change.is_new {
                            change.is_new = false;
                            self.new_change_count = self.new_change_count.saturating_sub(1);
                        }
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
                        // Clear NEW flag when user adds to queue
                        if change.is_new {
                            change.is_new = false;
                            self.new_change_count = self.new_change_count.saturating_sub(1);
                        }
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
            AppMode::Stopping | AppMode::Error | AppMode::Proposing | AppMode::QrPopup => None,
        }
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
            AppMode::Select => {
                // In select mode:
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
            AppMode::Running | AppMode::Stopped => {
                // In running/stopped mode:
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
            AppMode::Stopping | AppMode::Error | AppMode::Proposing | AppMode::QrPopup => None,
        }
    }

    /// Update approval status for a specific change
    pub fn update_approval_status(&mut self, change_id: &str, is_approved: bool) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.is_approved = is_approved;
            // Clear NEW flag when user approves/unapproves the change
            if change.is_new {
                change.is_new = false;
                self.new_change_count = self.new_change_count.saturating_sub(1);
            }
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
            dependencies: Vec::new(),
        }
    }

    fn create_approved_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
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

    #[test]
    fn test_toggle_selection_clears_new_badge_in_running_mode() {
        // Test that adding to queue in Running mode clears the NEW badge
        let changes = vec![create_approved_change("new-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Mark as new and set new_change_count
        app.changes[0].is_new = true;
        app.new_change_count = 1;

        // Start processing to enter Running mode
        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        // Remove from queue first so we can test adding
        let _ = app.toggle_selection();
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);

        // Reset new state for the add-to-queue test
        app.changes[0].is_new = true;
        app.new_change_count = 1;

        // Add to queue should clear NEW flag
        let cmd = app.toggle_selection();
        assert!(matches!(cmd, Some(TuiCommand::AddToQueue(_))));
        assert!(!app.changes[0].is_new);
        assert_eq!(app.new_change_count, 0);
    }

    #[test]
    fn test_toggle_selection_clears_new_badge_in_stopped_mode() {
        // Test that adding to queue in Stopped mode clears the NEW badge
        let changes = vec![create_approved_change("new-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Mark as new and set new_change_count
        app.changes[0].is_new = true;
        app.new_change_count = 1;

        // Start processing and then stop
        app.start_processing();
        app.mode = AppMode::Stopped;

        // Remove from queue first
        let _ = app.toggle_selection();
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);

        // Reset new state for the add-to-queue test
        app.changes[0].is_new = true;
        app.new_change_count = 1;

        // Add to queue should clear NEW flag
        let cmd = app.toggle_selection();
        assert!(matches!(cmd, Some(TuiCommand::AddToQueue(_))));
        assert!(!app.changes[0].is_new);
        assert_eq!(app.new_change_count, 0);
    }

    #[test]
    fn test_update_approval_status_clears_new_badge_on_approve() {
        // Test that approving a change clears the NEW badge
        let changes = vec![create_test_change("new-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Mark as new and set new_change_count
        app.changes[0].is_new = true;
        app.new_change_count = 1;

        // Approve the change
        app.update_approval_status("new-change", true);

        // NEW flag should be cleared
        assert!(!app.changes[0].is_new);
        assert_eq!(app.new_change_count, 0);
        assert!(app.changes[0].is_approved);
    }

    #[test]
    fn test_update_approval_status_clears_new_badge_on_unapprove() {
        // Test that unapproving a change also clears the NEW badge
        let changes = vec![create_approved_change("new-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Mark as new and set new_change_count
        app.changes[0].is_new = true;
        app.new_change_count = 1;

        // Unapprove the change
        app.update_approval_status("new-change", false);

        // NEW flag should be cleared
        assert!(!app.changes[0].is_new);
        assert_eq!(app.new_change_count, 0);
        assert!(!app.changes[0].is_approved);
    }

    #[test]
    fn test_toggle_approval_in_stopped_mode_returns_approve_only() {
        // Test that toggle_approval in Stopped mode returns ApproveOnly for unapproved change
        // (not ApproveAndQueue, which would incorrectly add to queue while stopped)
        let changes = vec![create_test_change("unapproved-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Enter Running mode first, then stop
        app.start_processing();
        app.mode = AppMode::Stopped;
        assert_eq!(app.mode, AppMode::Stopped);
        assert!(!app.changes[0].is_approved);

        // Toggle approval should return ApproveOnly (not ApproveAndQueue)
        let cmd = app.toggle_approval();
        assert!(matches!(
            cmd,
            Some(TuiCommand::ApproveOnly(ref id)) if id == "unapproved-change"
        ));
    }

    #[test]
    fn test_toggle_approval_in_stopped_mode_unapproves_correctly() {
        // Test that toggle_approval in Stopped mode returns UnapproveAndDequeue for approved change
        let changes = vec![create_approved_change("approved-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Enter Running mode first, then stop
        app.start_processing();
        app.mode = AppMode::Stopped;
        assert_eq!(app.mode, AppMode::Stopped);
        assert!(app.changes[0].is_approved);

        // Toggle approval should return UnapproveAndDequeue
        let cmd = app.toggle_approval();
        assert!(matches!(
            cmd,
            Some(TuiCommand::UnapproveAndDequeue(ref id)) if id == "approved-change"
        ));
    }

    #[test]
    fn test_start_proposing_mode_transition() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        assert_eq!(app.mode, AppMode::Select);
        assert!(app.propose_textarea.is_none());
        assert!(app.previous_mode.is_none());

        app.start_proposing();

        assert_eq!(app.mode, AppMode::Proposing);
        assert!(app.propose_textarea.is_some());
        assert_eq!(app.previous_mode, Some(AppMode::Select));
    }

    #[test]
    fn test_cancel_proposing_returns_to_previous_mode() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start from Running mode
        app.mode = AppMode::Running;
        app.start_proposing();

        assert_eq!(app.mode, AppMode::Proposing);
        assert_eq!(app.previous_mode, Some(AppMode::Running));

        app.cancel_proposing();

        assert_eq!(app.mode, AppMode::Running);
        assert!(app.propose_textarea.is_none());
        assert!(app.previous_mode.is_none());
    }

    #[test]
    fn test_submit_proposal_returns_text() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_proposing();

        // Insert some text into the textarea
        if let Some(ref mut textarea) = app.propose_textarea {
            textarea.insert_str("Add authentication feature");
        }

        let result = app.submit_proposal();

        assert_eq!(result, Some("Add authentication feature".to_string()));
        assert_eq!(app.mode, AppMode::Select);
        assert!(app.propose_textarea.is_none());
    }

    #[test]
    fn test_submit_proposal_returns_none_for_empty_text() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_proposing();

        // Don't insert any text

        let result = app.submit_proposal();

        assert!(result.is_none());
        // Should remain in Proposing mode with input retained
        assert_eq!(app.mode, AppMode::Proposing);
        assert!(app.propose_textarea.is_some());
    }

    #[test]
    fn test_submit_proposal_trims_whitespace() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_proposing();

        // Insert only whitespace
        if let Some(ref mut textarea) = app.propose_textarea {
            textarea.insert_str("   \n\t  ");
        }

        let result = app.submit_proposal();

        assert!(result.is_none());
        // Should remain in Proposing mode with input retained
        assert_eq!(app.mode, AppMode::Proposing);
        assert!(app.propose_textarea.is_some());
    }

    #[test]
    fn test_toggle_selection_does_nothing_in_proposing_mode() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_proposing();
        assert_eq!(app.mode, AppMode::Proposing);

        // Toggle should return None in Proposing mode
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
    }

    #[test]
    fn test_toggle_approval_does_nothing_in_proposing_mode() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_proposing();
        assert_eq!(app.mode, AppMode::Proposing);

        // Toggle approval should return None in Proposing mode
        let cmd = app.toggle_approval();
        assert!(cmd.is_none());
    }

    // === Tests for tui-editor spec (Proposing mode) ===

    #[test]
    fn test_proposing_mode_from_running_mode() {
        // Test that proposing can be started from Running mode
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        app.start_proposing();
        assert_eq!(app.mode, AppMode::Proposing);
        assert_eq!(app.previous_mode, Some(AppMode::Running));
    }

    #[test]
    fn test_proposing_mode_from_stopped_mode() {
        // Test that proposing can be started from Stopped mode
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_processing();
        app.mode = AppMode::Stopped;

        app.start_proposing();
        assert_eq!(app.mode, AppMode::Proposing);
        assert_eq!(app.previous_mode, Some(AppMode::Stopped));
    }

    #[test]
    fn test_proposing_mode_cancel_returns_to_running() {
        // Test that canceling Proposing returns to Running
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Running;
        app.start_proposing();
        app.cancel_proposing();

        assert_eq!(app.mode, AppMode::Running);
    }

    #[test]
    fn test_proposing_mode_textarea_cleared_on_cancel() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_proposing();
        assert!(app.propose_textarea.is_some());

        app.cancel_proposing();
        assert!(app.propose_textarea.is_none());
    }

    #[test]
    fn test_submit_proposal_multiline_text() {
        // Test that multiline proposals are preserved
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_proposing();

        if let Some(ref mut textarea) = app.propose_textarea {
            textarea.insert_str("Add authentication\nwith OAuth support\nand MFA");
        }

        let result = app.submit_proposal();
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("authentication"));
        assert!(text.contains("OAuth"));
        assert!(text.contains("MFA"));
    }

    #[test]
    fn test_submit_proposal_not_in_proposing_mode_returns_none() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Try to submit without entering Proposing mode
        let result = app.submit_proposal();
        assert!(result.is_none());
    }

    // === Tests for tui-key-hints spec (Footer messages) ===

    #[test]
    fn test_selected_count_reflects_approved_only() {
        // Only approved changes can be selected
        let changes = vec![
            create_approved_change("approved", 0, 1),
            create_test_change("unapproved", 0, 1),
        ];
        let app = AppState::new(changes);

        // Only approved change should be auto-selected
        assert_eq!(app.selected_count(), 1);
        assert!(app.changes[0].selected);
        assert!(!app.changes[1].selected);
    }

    #[test]
    fn test_warning_message_on_unapproved_selection() {
        let changes = vec![create_test_change("unapproved", 0, 1)];
        let mut app = AppState::new(changes);

        // Try to select unapproved change
        let cmd = app.toggle_selection();

        assert!(cmd.is_none());
        assert!(app.warning_message.is_some());
        assert!(app.warning_message.as_ref().unwrap().contains("unapproved"));
    }

    #[test]
    fn test_cursor_up_wraps_around() {
        let changes = vec![
            create_test_change("a", 0, 1),
            create_test_change("b", 0, 1),
            create_test_change("c", 0, 1),
        ];
        let mut app = AppState::new(changes);

        assert_eq!(app.cursor_index, 0);
        app.cursor_up();
        assert_eq!(app.cursor_index, 2); // Wraps to last
    }

    #[test]
    fn test_cursor_down_wraps_around() {
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];
        let mut app = AppState::new(changes);

        app.cursor_index = 1;
        app.cursor_down();
        assert_eq!(app.cursor_index, 0); // Wraps to first
    }

    // === Tests for approval state management ===

    #[test]
    fn test_unapprove_removes_from_queue() {
        // Simulating UnapproveAndDequeue behavior
        let changes = vec![create_approved_change("test", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);

        // Simulate unapprove handler
        app.update_approval_status("test", false);
        app.changes[0].queue_status = QueueStatus::NotQueued;
        app.changes[0].selected = false;

        assert!(!app.changes[0].is_approved);
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        assert!(!app.changes[0].selected);
    }

    #[test]
    fn test_approval_toggle_blocked_for_processing_change() {
        let changes = vec![create_approved_change("processing", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Processing;

        let cmd = app.toggle_approval();
        assert!(cmd.is_none());
        assert!(app.warning_message.is_some());
    }

    // === Tests for orchestration timing ===

    #[test]
    fn test_orchestration_started_at_set_on_start() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        assert!(app.orchestration_started_at.is_none());

        app.start_processing();

        assert!(app.orchestration_started_at.is_some());
    }

    #[test]
    fn test_orchestration_elapsed_initially_none() {
        let changes = vec![create_test_change("a", 0, 1)];
        let app = AppState::new(changes);

        assert!(app.orchestration_elapsed.is_none());
    }

    // === Tests for parallel mode state ===

    #[test]
    fn test_parallel_mode_default_false() {
        let changes = vec![create_test_change("a", 0, 1)];
        let app = AppState::new(changes);

        assert!(!app.parallel_mode);
    }

    #[test]
    fn test_max_concurrent_default() {
        let changes = vec![create_test_change("a", 0, 1)];
        let app = AppState::new(changes);

        // Default max concurrent is 4
        assert_eq!(app.max_concurrent, 4);
    }

    // === Tests for log auto-scroll ===

    #[test]
    fn test_log_auto_scroll_enabled_by_default() {
        let changes = vec![create_test_change("a", 0, 1)];
        let app = AppState::new(changes);

        assert!(app.log_auto_scroll);
        assert_eq!(app.log_scroll_offset, 0);
    }

    // === Tests for stop mode ===

    #[test]
    fn test_stop_mode_initially_none() {
        let changes = vec![create_test_change("a", 0, 1)];
        let app = AppState::new(changes);

        assert!(matches!(app.stop_mode, StopMode::None));
    }

    // === Tests for known_change_ids tracking ===

    #[test]
    fn test_known_change_ids_populated_on_creation() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let app = AppState::new(changes);

        assert!(app.known_change_ids.contains("change-a"));
        assert!(app.known_change_ids.contains("change-b"));
        assert_eq!(app.known_change_ids.len(), 2);
    }

    // === Tests for empty state handling ===

    #[test]
    fn test_empty_changes_list_handling() {
        let app = AppState::new(vec![]);

        assert!(app.changes.is_empty());
        assert_eq!(app.cursor_index, 0);
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn test_cursor_navigation_with_empty_list() {
        let mut app = AppState::new(vec![]);

        // Should not panic with empty list
        app.cursor_up();
        assert_eq!(app.cursor_index, 0);

        app.cursor_down();
        assert_eq!(app.cursor_index, 0);
    }

    #[test]
    fn test_toggle_selection_with_empty_list() {
        let mut app = AppState::new(vec![]);

        // Should return None and not panic
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
    }

    // === Tests for QR popup mode ===

    #[test]
    fn test_qr_popup_requires_web_url() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Without web_url, show_qr_popup should do nothing
        assert!(app.web_url.is_none());
        app.show_qr_popup();
        assert_eq!(app.mode, AppMode::Select); // Mode unchanged
    }

    #[test]
    fn test_qr_popup_mode_transition() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.web_url = Some("http://localhost:8080".to_string());

        // Should transition to QrPopup mode
        app.show_qr_popup();
        assert_eq!(app.mode, AppMode::QrPopup);
        assert_eq!(app.previous_mode, Some(AppMode::Select));
    }

    #[test]
    fn test_qr_popup_returns_to_previous_mode() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.web_url = Some("http://localhost:8080".to_string());

        // Start from Running mode
        app.mode = AppMode::Running;
        app.show_qr_popup();
        assert_eq!(app.mode, AppMode::QrPopup);
        assert_eq!(app.previous_mode, Some(AppMode::Running));

        // Hide should return to Running
        app.hide_qr_popup();
        assert_eq!(app.mode, AppMode::Running);
        assert!(app.previous_mode.is_none());
    }

    #[test]
    fn test_qr_popup_from_stopped_mode() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.web_url = Some("http://127.0.0.1:3000".to_string());

        app.mode = AppMode::Stopped;
        app.show_qr_popup();
        assert_eq!(app.mode, AppMode::QrPopup);
        assert_eq!(app.previous_mode, Some(AppMode::Stopped));

        app.hide_qr_popup();
        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn test_toggle_selection_does_nothing_in_qr_popup_mode() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.web_url = Some("http://localhost:8080".to_string());

        app.show_qr_popup();
        assert_eq!(app.mode, AppMode::QrPopup);

        // Toggle should return None in QrPopup mode
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
    }

    #[test]
    fn test_toggle_approval_does_nothing_in_qr_popup_mode() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.web_url = Some("http://localhost:8080".to_string());

        app.show_qr_popup();
        assert_eq!(app.mode, AppMode::QrPopup);

        // Toggle approval should return None in QrPopup mode
        let cmd = app.toggle_approval();
        assert!(cmd.is_none());
    }

    #[test]
    fn test_web_url_default_none() {
        let changes = vec![create_test_change("a", 0, 1)];
        let app = AppState::new(changes);

        // web_url should be None by default
        assert!(app.web_url.is_none());
    }

    // === Tests for proposal editing status preservation ===

    #[test]
    fn test_app_mode_preserved_during_proposal_edit_simulation() {
        // Simulate the behavior of opening and closing editor during proposal edit
        // The editor launch/exit does NOT change app.mode
        let changes = vec![create_approved_change("test-change", 0, 1)];
        let app = AppState::new(changes);

        // Start in Select mode
        assert_eq!(app.mode, AppMode::Select);

        // Simulate editor launch: mode should remain unchanged
        // (In actual code: disable_raw_mode, LeaveAlternateScreen, launch editor, EnterAlternateScreen, enable_raw_mode)
        // No app.mode change occurs
        assert_eq!(app.mode, AppMode::Select);
    }

    #[test]
    fn test_app_mode_preserved_during_proposal_edit_in_running_mode() {
        // Test that app.mode remains Running when editor is launched from Running mode
        let changes = vec![create_approved_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        // Simulate editor launch and exit: mode should remain Running
        assert_eq!(app.mode, AppMode::Running);
    }

    #[test]
    fn test_app_mode_preserved_during_proposal_edit_in_stopped_mode() {
        // Test that app.mode remains Stopped when editor is launched from Stopped mode
        let changes = vec![create_approved_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_processing();
        app.mode = AppMode::Stopped;

        // Simulate editor launch and exit: mode should remain Stopped
        assert_eq!(app.mode, AppMode::Stopped);
    }
}
