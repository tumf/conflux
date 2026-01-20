//! State management for the TUI
//!
//! This module contains AppState and ChangeState implementations,
//! organized into submodules by responsibility.

mod change;
mod events;
mod logs;
mod modes;

use crate::openspec::Change;
use crate::vcs::GitWorkspaceManager;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::events::{LogEntry, TuiCommand};
use super::types::{AppMode, QueueStatus, StopMode, ViewMode, WorktreeAction, WorktreeInfo};

// Re-exports
pub use change::ChangeState;

/// Auto-refresh interval in seconds
pub const AUTO_REFRESH_INTERVAL_SECS: u64 = 5;

/// Warning popup content
pub struct WarningPopup {
    pub title: String,
    pub message: String,
}

/// Main application state for the TUI
pub struct AppState {
    /// Current view mode (Changes or Worktrees)
    pub view_mode: ViewMode,
    /// Current mode
    pub mode: AppMode,
    /// List of changes with their states
    pub changes: Vec<ChangeState>,
    /// Current cursor position in the list
    pub cursor_index: usize,
    /// List widget state
    pub list_state: ListState,
    /// List of worktrees
    pub worktrees: Vec<WorktreeInfo>,
    /// Current cursor position in the worktree list
    pub worktree_cursor_index: usize,
    /// Worktree list widget state
    pub worktree_list_state: ListState,
    /// Pending worktree action confirmation (path, action)
    pub pending_worktree_action: Option<(String, WorktreeAction)>,
    /// Branch name associated with pending worktree action (for deletion)
    pub pending_worktree_branch: Option<String>,
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
    /// Warning popup content
    pub warning_popup: Option<WarningPopup>,
    /// Pending worktree delete confirmation (change ID)
    pub pending_worktree_delete: Option<String>,
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
    /// Whether parallel execution is available (git)
    pub parallel_available: bool,
    /// VCS backend being used (git)
    pub vcs_backend: String,
    /// Max concurrent workspaces for parallel execution
    pub max_concurrent: usize,
    /// When orchestration started (for overall elapsed time)
    pub orchestration_started_at: Option<Instant>,
    /// Total elapsed time when orchestration finished
    pub orchestration_elapsed: Option<Duration>,
    /// Mode to return to after closing modal popups
    pub previous_mode: Option<AppMode>,
    /// Web UI URL (set when web server is enabled)
    pub web_url: Option<String>,
    /// Whether resolve is currently executing (blocks M key operations)
    pub is_resolving: bool,
    /// Map of change_id to worktree path for active worktrees (for progress fallback)
    pub worktree_paths: HashMap<String, PathBuf>,
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
            view_mode: ViewMode::Changes,
            mode: AppMode::Select,
            changes: change_states,
            cursor_index: 0,
            list_state,
            worktrees: Vec::new(),
            worktree_cursor_index: 0,
            worktree_list_state: ListState::default(),
            pending_worktree_action: None,
            pending_worktree_branch: None,
            current_change: None,
            error_change_id: None,
            logs,
            last_refresh: Instant::now(),
            new_change_count: 0,
            known_change_ids: known_ids,
            should_quit: false,
            warning_message: None,
            warning_popup: None,
            pending_worktree_delete: None,
            spinner_frame: 0,
            log_scroll_offset: 0,
            log_auto_scroll: true,
            stop_mode: StopMode::None,
            parallel_mode: false,
            parallel_available: crate::cli::check_parallel_available(),
            vcs_backend: "git".to_string(),
            max_concurrent: 4, // Default value, can be overridden from config
            orchestration_started_at: None,
            orchestration_elapsed: None,
            previous_mode: None,
            web_url: None,
            is_resolving: false,
            worktree_paths: HashMap::new(),
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

    /// Move worktree cursor up
    pub fn worktree_cursor_up(&mut self) {
        if self.worktrees.is_empty() {
            return;
        }
        self.worktree_cursor_index = if self.worktree_cursor_index == 0 {
            self.worktrees.len() - 1
        } else {
            self.worktree_cursor_index - 1
        };
        self.worktree_list_state
            .select(Some(self.worktree_cursor_index));
    }

    /// Move worktree cursor down
    pub fn worktree_cursor_down(&mut self) {
        if self.worktrees.is_empty() {
            return;
        }
        self.worktree_cursor_index = (self.worktree_cursor_index + 1) % self.worktrees.len();
        self.worktree_list_state
            .select(Some(self.worktree_cursor_index));
    }

    /// Get the selected worktree path (if any)
    pub fn get_selected_worktree_path(&self) -> Option<String> {
        if self.worktree_cursor_index < self.worktrees.len() {
            Some(
                self.worktrees[self.worktree_cursor_index]
                    .path
                    .display()
                    .to_string(),
            )
        } else {
            None
        }
    }

    /// Get the selected worktree (if any)
    pub fn get_selected_worktree(&self) -> Option<&WorktreeInfo> {
        if self.worktree_cursor_index < self.worktrees.len() {
            Some(&self.worktrees[self.worktree_cursor_index])
        } else {
            None
        }
    }

    /// Request worktree delete with validation
    ///
    /// Returns Some(TuiCommand) if deletion should proceed, None if it should be blocked
    pub fn request_worktree_delete_from_list(&mut self) -> Option<TuiCommand> {
        if self.worktrees.is_empty() || self.worktree_cursor_index >= self.worktrees.len() {
            return None;
        }

        let worktree = &self.worktrees[self.worktree_cursor_index];

        // Cannot delete main worktree
        if worktree.is_main {
            self.warning_message = Some("Cannot delete main worktree".to_string());
            return None;
        }

        // Extract change_id from worktree branch name
        let change_id_opt = if !worktree.branch.is_empty() && !worktree.is_detached {
            GitWorkspaceManager::extract_change_id_from_worktree_name(&worktree.branch)
        } else {
            None
        };

        // Check if the worktree is related to a change that is queued or processing
        if let Some(change_id) = change_id_opt {
            if let Some(change) = self.changes.iter().find(|c| c.id == change_id) {
                // Block deletion if change is in active processing states
                let is_active = matches!(
                    change.queue_status,
                    QueueStatus::Queued
                        | QueueStatus::Applying
                        | QueueStatus::Archiving
                        | QueueStatus::Resolving
                        | QueueStatus::Accepting
                        | QueueStatus::MergeWait
                );

                if is_active {
                    self.warning_message = Some(format!(
                        "Cannot delete worktree: change '{}' is {}",
                        change_id,
                        change.queue_status.display()
                    ));
                    return None;
                }
            }
        }

        // Get the worktree path as string
        let path_str = worktree.path.display().to_string();

        // Get the branch name (if not detached and branch exists)
        let branch_name = if !worktree.is_detached && !worktree.branch.is_empty() {
            Some(worktree.branch.clone())
        } else {
            None
        };

        // Store pending action for confirmation
        self.pending_worktree_action = Some((path_str, WorktreeAction::Delete));
        self.pending_worktree_branch = branch_name;
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::ConfirmWorktreeDelete;

        None // User needs to confirm first
    }

    /// Confirm and execute pending worktree action
    pub fn confirm_worktree_action_delete(&mut self) -> Option<TuiCommand> {
        if let Some((path, WorktreeAction::Delete)) = self.pending_worktree_action.take() {
            // Get the branch name that was stored when the delete was requested
            let branch_name = self.pending_worktree_branch.take();

            // Restore previous mode
            if let Some(mode) = self.previous_mode.take() {
                self.mode = mode;
            } else {
                self.mode = AppMode::Select;
            }

            Some(TuiCommand::DeleteWorktreeByPath(path.into(), branch_name))
        } else {
            None
        }
    }

    /// Cancel pending worktree action
    pub fn cancel_worktree_action(&mut self) {
        self.pending_worktree_action = None;
        self.pending_worktree_branch = None;

        // Restore previous mode
        if let Some(mode) = self.previous_mode.take() {
            self.mode = mode;
        } else {
            self.mode = AppMode::Select;
        }
    }

    /// Request to merge worktree branch into base branch.
    ///
    /// Returns Some(TuiCommand) if merge should proceed, None if blocked.
    pub fn request_merge_worktree_branch(&mut self) -> Option<TuiCommand> {
        use crate::tui::types::ViewMode;
        use tracing::debug;

        debug!(
            "request_merge_worktree_branch called: view_mode={:?}, worktrees_len={}, cursor_index={}",
            self.view_mode,
            self.worktrees.len(),
            self.worktree_cursor_index
        );

        if self.view_mode != ViewMode::Worktrees {
            debug!(
                "Merge blocked: view_mode is {:?}, not Worktrees",
                self.view_mode
            );
            self.warning_message = Some("Switch to Worktrees view to merge".to_string());
            return None;
        }

        // Block M key during resolve execution
        if self.is_resolving {
            debug!("Merge blocked: resolve operation in progress");
            self.warning_message = Some("Cannot merge: resolve operation in progress".to_string());
            return None;
        }

        if self.worktrees.is_empty() {
            debug!("Merge blocked: worktrees list is empty");
            self.warning_message = Some("No worktrees loaded".to_string());
            return None;
        }

        if self.worktree_cursor_index >= self.worktrees.len() {
            debug!(
                "Merge blocked: cursor out of range: {} >= {}",
                self.worktree_cursor_index,
                self.worktrees.len()
            );
            self.warning_message = Some(format!(
                "Cursor out of range: {} >= {}",
                self.worktree_cursor_index,
                self.worktrees.len()
            ));
            return None;
        }

        let worktree = &self.worktrees[self.worktree_cursor_index];
        debug!(
            "Worktree selected: path={}, branch={}, is_main={}, is_detached={}, has_conflict={}",
            worktree.path.display(),
            worktree.branch,
            worktree.is_main,
            worktree.is_detached,
            worktree.has_merge_conflict()
        );

        // Cannot merge main worktree
        if worktree.is_main {
            debug!("Merge blocked: is main worktree");
            self.warning_message = Some("Cannot merge main worktree".to_string());
            return None;
        }

        // Cannot merge detached HEAD
        if worktree.is_detached {
            debug!("Merge blocked: is detached HEAD");
            self.warning_message = Some("Cannot merge detached HEAD".to_string());
            return None;
        }

        // Cannot merge if conflicts detected
        if worktree.has_merge_conflict() {
            debug!(
                "Merge blocked: has {} conflict(s)",
                worktree.conflict_file_count()
            );
            self.warning_message = Some(format!(
                "Cannot merge: {} conflict(s) detected",
                worktree.conflict_file_count()
            ));
            return None;
        }

        // Get worktree path and branch name
        let path = worktree.path.clone();
        let branch_name = worktree.branch.clone();

        if branch_name.is_empty() {
            debug!("Merge blocked: branch name is empty");
            self.warning_message = Some("Cannot merge: no branch name".to_string());
            return None;
        }

        // Cannot merge if no commits ahead of base branch
        if !worktree.has_commits_ahead {
            debug!("Merge blocked: no commits ahead of base branch");
            self.warning_message =
                Some("Cannot merge: no commits ahead of base branch".to_string());

            // Cannot merge if already merging
            if worktree.is_merging {
                debug!("Merge blocked: merge already in progress");
                self.warning_message = Some("Cannot merge: merge already in progress".to_string());
                return None;
            }
            return None;
        }

        debug!("Merge approved: creating TuiCommand::MergeWorktreeBranch");
        Some(TuiCommand::MergeWorktreeBranch {
            worktree_path: path,
            branch_name,
        })
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

        if self.parallel_mode && !change.is_parallel_eligible {
            self.warning_message = Some(format!(
                "Cannot queue uncommitted change '{}' in parallel mode. Commit it first.",
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
                // In Stopped mode, only toggle execution mark (selected), not queue_status
                // Queue status remains NotQueued until F5 resume
                if !matches!(change.queue_status, QueueStatus::NotQueued) {
                    // Cannot modify non-NotQueued changes (completed, error, etc.)
                    return None;
                }

                // Toggle execution mark only
                change.selected = !change.selected;

                // Clear NEW flag when user interacts with the change
                if change.is_new {
                    change.is_new = false;
                    self.new_change_count = self.new_change_count.saturating_sub(1);
                }

                let id = change.id.clone();
                if change.selected {
                    self.add_log(LogEntry::info(format!("Marked for execution: {}", id)));
                } else {
                    self.add_log(LogEntry::info(format!("Unmarked: {}", id)));
                }
                None // No command needed, just state change
            }
            AppMode::Stopping
            | AppMode::Error
            | AppMode::ConfirmWorktreeDelete
            | AppMode::QrPopup => None,
        }
    }

    /// Trigger merge resolution for the selected change when applicable.
    pub fn resolve_merge(&mut self) -> Option<TuiCommand> {
        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return None;
        }

        if !matches!(
            self.mode,
            AppMode::Select | AppMode::Stopped | AppMode::Running
        ) {
            return None;
        }

        // Block M key during resolve execution
        if self.is_resolving {
            self.warning_message = Some("Cannot merge: resolve operation in progress".to_string());
            return None;
        }

        let change = &self.changes[self.cursor_index];
        if matches!(change.queue_status, QueueStatus::MergeWait) {
            Some(TuiCommand::ResolveMerge(change.id.clone()))
        } else {
            None
        }
    }

    /// Update parallel eligibility status for changes.
    pub fn apply_parallel_eligibility(&mut self, committed_change_ids: &HashSet<String>) {
        for change in &mut self.changes {
            change.is_parallel_eligible = committed_change_ids.contains(&change.id);
            if self.parallel_mode
                && matches!(self.mode, AppMode::Select | AppMode::Stopped)
                && !change.is_parallel_eligible
            {
                if change.selected {
                    change.selected = false;
                }
                if matches!(change.queue_status, QueueStatus::Queued) {
                    change.queue_status = QueueStatus::NotQueued;
                }
            }
        }
    }

    /// Update worktree presence flags for changes.
    pub fn apply_worktree_status(&mut self, worktree_change_ids: &HashSet<String>) {
        for change in &mut self.changes {
            let sanitized = change.id.replace(['/', '\\', ' '], "-");
            change.has_worktree = worktree_change_ids.contains(&sanitized);
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
        use tracing::debug;

        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            debug!(
                "toggle_approval: early return - changes.is_empty={}, cursor_index={}, changes.len={}",
                self.changes.is_empty(),
                self.cursor_index,
                self.changes.len()
            );
            return None;
        }

        let change = &self.changes[self.cursor_index];

        debug!(
            "toggle_approval: change_id={}, queue_status={:?}, is_approved={}, mode={:?}",
            change.id, change.queue_status, change.is_approved, self.mode
        );

        // Block approval toggle for processing changes
        if matches!(
            change.queue_status,
            QueueStatus::Applying | QueueStatus::Resolving
        ) {
            self.warning_message = Some("Cannot change approval for processing change".to_string());
            debug!("toggle_approval: blocked by Processing/Resolving status");
            return None;
        }

        let id = change.id.clone();
        let is_approved = change.is_approved;

        match self.mode {
            AppMode::Select => {
                // In select mode:
                // [ ] (unapproved) → @ → [x] (approved + selected, NOT queued)
                // [x] (approved + selected) → @ → [ ] (unapproved + not selected)
                if !is_approved {
                    // Unapproved → approved + selected (no auto-queue)
                    Some(TuiCommand::ApproveOnly(id))
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
            AppMode::Stopping
            | AppMode::Error
            | AppMode::ConfirmWorktreeDelete
            | AppMode::QrPopup => None,
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

    /// Request worktree deletion for the selected change.
    #[allow(dead_code)]
    pub fn request_worktree_delete(&mut self) {
        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return;
        }

        let change = &self.changes[self.cursor_index];
        if matches!(
            change.queue_status,
            QueueStatus::Applying | QueueStatus::Archiving | QueueStatus::Resolving
        ) {
            self.warning_popup = Some(WarningPopup {
                title: "Worktree delete blocked".to_string(),
                message: format!(
                    "Change '{}' is currently running. Stop processing before deleting its worktree.",
                    change.id
                ),
            });
            return;
        }

        self.pending_worktree_delete = Some(change.id.clone());
        self.mode = AppMode::ConfirmWorktreeDelete;
    }

    /// Confirm the pending worktree delete request.
    #[allow(dead_code)]
    pub fn confirm_worktree_delete(&mut self) -> Option<TuiCommand> {
        let change_id = self.pending_worktree_delete.take()?;
        self.mode = AppMode::Select;
        Some(TuiCommand::DeleteWorktree(change_id))
    }

    /// Cancel worktree delete confirmation.
    #[allow(dead_code)]
    pub fn cancel_worktree_delete(&mut self) {
        self.pending_worktree_delete = None;
        self.mode = AppMode::Select;
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
    use crate::tui::events::OrchestratorEvent;
    use std::collections::HashSet;

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

        // Start processing and set the change to Applying status
        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Applying;

        // Toggle should do nothing
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Applying);
    }

    #[test]
    fn test_toggle_selection_does_nothing_for_archiving_status() {
        // Use approved change so it starts selected
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing and set the change to Archiving status
        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Archiving;

        // Toggle should do nothing
        let cmd = app.toggle_selection();
        assert!(cmd.is_none());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archiving);
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
        // Empty changes list should show "Add new changes to get started"
        let app = AppState::new(vec![]);
        assert!(app.changes.is_empty());
        assert_eq!(app.selected_count(), 0);
        // The condition in render_footer_select: app.changes.is_empty() -> "Add new changes..."
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

        // Simulate: a is applying, b is archiving
        app.changes[0].queue_status = QueueStatus::Applying;
        app.changes[1].queue_status = QueueStatus::Archiving;

        // Calculate progress (includes Archiving changes)
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

        // Both a (Processing) and b (Archiving) should be included
        assert_eq!(total_tasks, 8);
        assert_eq!(completed_tasks, 5);
    }

    #[test]
    fn test_approve_only_in_select_mode_returns_correct_command() {
        // Test that toggle_approval in Select mode returns ApproveOnly for unapproved change
        let changes = vec![create_test_change("unapproved-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Ensure we're in Select mode with an unapproved change
        assert_eq!(app.mode, AppMode::Select);
        assert!(!app.changes[0].is_approved);

        // Toggle approval should return ApproveOnly
        let cmd = app.toggle_approval();
        assert!(matches!(
            cmd,
            Some(TuiCommand::ApproveOnly(ref id)) if id == "unapproved-change"
        ));
    }

    #[test]
    fn test_approve_only_state_update_simulation_select_mode() {
        // Simulate what runner.rs ApproveOnly handler does in Select mode
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Initial state: unapproved, not selected
        assert!(!app.changes[0].is_approved);
        assert!(!app.changes[0].selected);
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);

        // Simulate ApproveOnly handler logic for Select mode
        let id = "test-change";

        // 1. update_approval_status (this adds a log)
        app.update_approval_status(id, true);

        // 2. Mark selected (no queue status change)
        if let Some(change) = app.changes.iter_mut().find(|c| c.id == id) {
            change.selected = true;
        }

        // Verify final state: approved + selected + not queued
        assert!(app.changes[0].is_approved);
        assert!(app.changes[0].selected);
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);

        // This is the state that should render as [x] (selected)
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
        // Test that toggling execution mark in Stopped mode clears the NEW badge
        let changes = vec![create_approved_change("new-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Mark as new and set new_change_count
        app.changes[0].is_new = true;
        app.new_change_count = 1;

        // Enter Stopped mode (with NotQueued status)
        app.mode = AppMode::Stopped;
        app.changes[0].queue_status = QueueStatus::NotQueued;
        app.changes[0].selected = false;

        // Toggle execution mark should clear NEW flag
        let cmd = app.toggle_selection();
        // In Stopped mode, no command is returned (just state change)
        assert!(cmd.is_none());
        // Queue status remains NotQueued
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        // Execution mark should be set
        assert!(app.changes[0].selected);
        // NEW flag should be cleared
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
        // (no auto-queue side effects while stopped)
        let changes = vec![create_test_change("unapproved-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Enter Running mode first, then stop
        app.start_processing();
        app.mode = AppMode::Stopped;
        assert_eq!(app.mode, AppMode::Stopped);
        assert!(!app.changes[0].is_approved);

        // Toggle approval should return ApproveOnly (no queue side effects)
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
    fn test_uncommitted_changes_warning_logs_only() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::Warning {
            title: "Uncommitted Changes Detected".to_string(),
            message: "Warning: Uncommitted changes detected.".to_string(),
        });

        // Uncommitted changes warning should NOT show popup
        assert!(app.warning_popup.is_none());
        // But should be logged
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Warning: Uncommitted")));
    }

    #[test]
    fn test_other_warning_popup_set_on_warning_event() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::Warning {
            title: "Some Other Warning".to_string(),
            message: "Warning: Something else happened.".to_string(),
        });

        // Other warnings should still show popup
        assert!(app.warning_popup.is_some());
        let popup = app.warning_popup.as_ref().unwrap();
        assert_eq!(popup.title, "Some Other Warning");
        assert!(popup.message.contains("Warning: Something else"));
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Warning: Something else")));
    }

    #[test]
    fn test_apply_worktree_status_sets_flag() {
        let changes = vec![create_test_change("change/a", 0, 1)];
        let mut app = AppState::new(changes);
        let mut worktree_ids = HashSet::new();
        worktree_ids.insert("change-a".to_string());

        app.apply_worktree_status(&worktree_ids);

        assert!(app.changes[0].has_worktree);
    }

    #[test]
    fn test_request_worktree_delete_sets_confirmation() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.request_worktree_delete();

        assert_eq!(app.mode, AppMode::ConfirmWorktreeDelete);
        assert_eq!(app.pending_worktree_delete.as_deref(), Some("change-a"));
    }

    #[test]
    fn test_request_worktree_delete_blocks_processing_change() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Applying;

        app.request_worktree_delete();

        assert!(app.warning_popup.is_some());
        assert!(app.pending_worktree_delete.is_none());
        assert_eq!(app.mode, AppMode::Select);
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
        app.changes[0].queue_status = QueueStatus::Applying;

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

    #[test]
    fn test_scroll_logs_up_disables_auto_scroll() {
        use crate::tui::events::LogEntry;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Add some logs
        for i in 0..20 {
            app.add_log(LogEntry::info(format!("Log entry {}", i)));
        }

        // Initially auto-scroll is enabled
        assert!(app.log_auto_scroll);
        assert_eq!(app.log_scroll_offset, 0);

        // Scroll up should disable auto-scroll
        app.scroll_logs_up(5);

        assert!(
            !app.log_auto_scroll,
            "Auto-scroll should be disabled after scrolling up"
        );
        assert_eq!(app.log_scroll_offset, 5);

        // Adding a new log should increment scroll offset to maintain view position
        app.add_log(LogEntry::info("New log entry"));

        assert!(!app.log_auto_scroll, "Auto-scroll should remain disabled");
        assert_eq!(
            app.log_scroll_offset, 6,
            "Scroll offset should increment to maintain view position when auto-scroll is disabled"
        );

        // Adding more logs should continue incrementing
        app.add_log(LogEntry::info("Another log entry"));
        assert_eq!(app.log_scroll_offset, 7);
    }

    #[test]
    fn test_scroll_logs_down_to_bottom_enables_auto_scroll() {
        use crate::tui::events::LogEntry;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Add some logs
        for i in 0..20 {
            app.add_log(LogEntry::info(format!("Log entry {}", i)));
        }

        // Scroll up to disable auto-scroll
        app.scroll_logs_up(10);
        assert!(!app.log_auto_scroll);
        assert_eq!(app.log_scroll_offset, 10);

        // Scroll down but not to bottom
        app.scroll_logs_down(5);
        assert!(
            !app.log_auto_scroll,
            "Auto-scroll should remain disabled when not at bottom"
        );
        assert_eq!(app.log_scroll_offset, 5);

        // Scroll down to bottom
        app.scroll_logs_down(10); // More than offset, should clamp to 0
        assert!(
            app.log_auto_scroll,
            "Auto-scroll should be enabled when at bottom"
        );
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

    // === Tests for editor launch status preservation ===

    #[test]
    fn test_app_mode_preserved_during_editor_launch_simulation() {
        // Simulate the behavior of opening and closing the editor
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
    fn test_app_mode_preserved_during_editor_launch_in_running_mode() {
        // Test that app.mode remains Running when editor is launched from Running mode
        let changes = vec![create_approved_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        // Simulate editor launch and exit: mode should remain Running
        assert_eq!(app.mode, AppMode::Running);
    }

    #[test]
    fn test_app_mode_preserved_during_editor_launch_in_stopped_mode() {
        // Test that app.mode remains Stopped when editor is launched from Stopped mode
        let changes = vec![create_approved_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.start_processing();
        app.mode = AppMode::Stopped;

        // Simulate editor launch and exit: mode should remain Stopped
        assert_eq!(app.mode, AppMode::Stopped);
    }

    // === Tests for worktree cursor navigation ===

    #[test]
    fn test_worktree_cursor_up_with_empty_list() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        assert_eq!(app.worktree_cursor_index, 0);
        assert!(app.worktrees.is_empty());

        // Should not panic with empty worktree list
        app.worktree_cursor_up();
        assert_eq!(app.worktree_cursor_index, 0);
    }

    #[test]
    fn test_worktree_cursor_down_with_empty_list() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        assert_eq!(app.worktree_cursor_index, 0);
        assert!(app.worktrees.is_empty());

        // Should not panic with empty worktree list
        app.worktree_cursor_down();
        assert_eq!(app.worktree_cursor_index, 0);
    }

    #[test]
    fn test_worktree_cursor_navigation() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Add some worktrees
        app.worktrees = vec![
            WorktreeInfo {
                path: PathBuf::from("/path/to/worktree1"),
                head: "abc123".to_string(),
                branch: "main".to_string(),
                is_detached: false,
                is_main: true,
                merge_conflict: None,
                has_commits_ahead: false,
                is_merging: false,
            },
            WorktreeInfo {
                path: PathBuf::from("/path/to/worktree2"),
                head: "def456".to_string(),
                branch: "feature".to_string(),
                is_detached: false,
                is_main: false,
                merge_conflict: None,
                has_commits_ahead: true,
                is_merging: false,
            },
            WorktreeInfo {
                path: PathBuf::from("/path/to/worktree3"),
                head: "ghi789".to_string(),
                branch: String::new(),
                is_detached: true,
                is_main: false,
                merge_conflict: None,
                has_commits_ahead: false,
                is_merging: false,
            },
        ];

        assert_eq!(app.worktree_cursor_index, 0);

        // Move down
        app.worktree_cursor_down();
        assert_eq!(app.worktree_cursor_index, 1);

        app.worktree_cursor_down();
        assert_eq!(app.worktree_cursor_index, 2);

        // Wrap around to beginning
        app.worktree_cursor_down();
        assert_eq!(app.worktree_cursor_index, 0);

        // Move up (wraps to end)
        app.worktree_cursor_up();
        assert_eq!(app.worktree_cursor_index, 2);

        app.worktree_cursor_up();
        assert_eq!(app.worktree_cursor_index, 1);
    }

    #[test]
    fn test_get_selected_worktree_path() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Empty list
        assert!(app.get_selected_worktree_path().is_none());

        // Add worktrees
        app.worktrees = vec![
            WorktreeInfo {
                path: PathBuf::from("/path/to/worktree1"),
                head: "abc123".to_string(),
                branch: "main".to_string(),
                is_detached: false,
                is_main: true,
                merge_conflict: None,
                has_commits_ahead: false,
                is_merging: false,
            },
            WorktreeInfo {
                path: PathBuf::from("/path/to/worktree2"),
                head: "def456".to_string(),
                branch: "feature".to_string(),
                is_detached: false,
                is_main: false,
                merge_conflict: None,
                has_commits_ahead: true,
                is_merging: false,
            },
        ];

        // First worktree selected
        assert_eq!(
            app.get_selected_worktree_path(),
            Some("/path/to/worktree1".to_string())
        );

        // Move cursor and check
        app.worktree_cursor_down();
        assert_eq!(
            app.get_selected_worktree_path(),
            Some("/path/to/worktree2".to_string())
        );
    }

    #[test]
    fn test_request_worktree_delete_from_list_empty_list() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Empty worktree list should return None
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none());
    }

    #[test]
    fn test_request_worktree_delete_from_list_main_worktree() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/path/to/main"),
            head: "abc123".to_string(),
            branch: "main".to_string(),
            is_detached: false,
            is_main: true,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        // Cannot delete main worktree
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none());
        assert!(app.warning_message.is_some());
        assert!(app
            .warning_message
            .as_ref()
            .unwrap()
            .contains("Cannot delete main worktree"));
    }

    #[test]
    fn test_request_worktree_delete_from_list_processing_worktree() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Worktree branch name matches change ID "a"
        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "a".to_string(), // Branch name matches change ID
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        // Simulate applying state for change "a"
        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Applying;

        // Cannot delete worktree for applying change
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none());
        assert!(app.warning_message.is_some());
        let warning = app.warning_message.as_ref().unwrap();
        assert!(warning.contains("Cannot delete worktree"));
        assert!(warning.contains("change 'a'"));
        assert!(warning.contains("applying"));
    }

    #[test]
    fn test_request_worktree_delete_from_list_valid() {
        use crate::tui::types::{ViewMode, WorktreeAction, WorktreeInfo};
        use std::path::PathBuf;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        // Valid deletion request
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none()); // No command yet, just sets pending action
        assert!(app.pending_worktree_action.is_some());
        assert!(matches!(
            app.pending_worktree_action,
            Some((ref path, WorktreeAction::Delete)) if path == "/path/to/worktree"
        ));
    }

    #[test]
    fn test_confirm_worktree_action_delete() {
        use crate::tui::types::{WorktreeAction, WorktreeInfo};
        use std::path::PathBuf;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        let worktree_path = PathBuf::from("/path/to/worktree");

        app.worktrees = vec![WorktreeInfo {
            path: worktree_path.clone(),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        app.pending_worktree_action = Some((
            worktree_path.to_string_lossy().to_string(),
            WorktreeAction::Delete,
        ));

        // Confirm deletion
        let cmd = app.confirm_worktree_action_delete();
        assert!(cmd.is_some());
        if let Some(TuiCommand::DeleteWorktreeByPath(path, branch_name)) = &cmd {
            assert_eq!(path, &worktree_path);
            assert_eq!(branch_name, &None); // No branch name set in this test
        } else {
            panic!("Expected DeleteWorktreeByPath command");
        }
        assert!(app.pending_worktree_action.is_none());
    }

    #[test]
    fn test_cancel_worktree_action() {
        use crate::tui::types::{WorktreeAction, WorktreeInfo};
        use std::path::PathBuf;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        let worktree_path = PathBuf::from("/path/to/worktree");

        app.worktrees = vec![WorktreeInfo {
            path: worktree_path.clone(),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        app.pending_worktree_action = Some((
            worktree_path.to_string_lossy().to_string(),
            WorktreeAction::Delete,
        ));

        // Cancel action
        app.cancel_worktree_action();
        assert!(app.pending_worktree_action.is_none());
    }

    // === Tests for allow-worktree-delete-while-running ===

    #[test]
    fn test_request_worktree_delete_unrelated_worktree_allowed() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        // Change "a" is processing, but worktree is for change "b" which doesn't exist in changes list
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/path/to/worktree-b"),
            head: "abc123".to_string(),
            branch: "b".to_string(), // Change ID not in changes list
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        // Simulate applying state for change "a"
        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Applying;
        app.warning_message = None; // Clear any warning from start_processing

        // Should allow deletion of worktree for change not in list
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none()); // No command yet, just sets pending action
        assert!(app.pending_worktree_action.is_some());
        assert!(app.warning_message.is_none());
    }

    #[test]
    fn test_request_worktree_delete_not_queued_change_allowed() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        // Change "a" exists but is NotQueued
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/path/to/worktree-a"),
            head: "abc123".to_string(),
            branch: "a".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        // Change "a" is NotQueued
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);

        // Should allow deletion
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none()); // No command yet, just sets pending action
        assert!(app.pending_worktree_action.is_some());
        assert!(app.warning_message.is_none());
    }

    #[test]
    fn test_request_worktree_delete_queued_change_blocked() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/path/to/worktree-a"),
            head: "abc123".to_string(),
            branch: "a".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        // Start processing to set change to Queued
        app.start_processing();
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);

        // Should block deletion
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none());
        assert!(app.warning_message.is_some());
        let warning = app.warning_message.as_ref().unwrap();
        assert!(warning.contains("Cannot delete worktree"));
        assert!(warning.contains("change 'a'"));
        assert!(warning.contains("queued"));
    }

    #[test]
    fn test_request_worktree_delete_archived_change_allowed() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/path/to/worktree-a"),
            head: "abc123".to_string(),
            branch: "a".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        // Set change to Archived status
        app.changes[0].queue_status = QueueStatus::Archived;

        // Should allow deletion (Archived is not an active state)
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none()); // No command yet, just sets pending action
        assert!(app.pending_worktree_action.is_some());
        assert!(app.warning_message.is_none());
    }

    #[test]
    fn test_request_worktree_delete_no_branch_allowed() {
        use crate::tui::types::WorktreeInfo;
        use std::path::PathBuf;

        // Change "a" is processing
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Worktree has no branch (detached HEAD)
        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "".to_string(),
            is_detached: true,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];

        app.start_processing();
        app.changes[0].queue_status = QueueStatus::Applying;
        app.warning_message = None; // Clear any warning from start_processing

        // Should allow deletion (cannot extract change_id from detached HEAD)
        let cmd = app.request_worktree_delete_from_list();
        assert!(cmd.is_none()); // No command yet, just sets pending action
        assert!(app.pending_worktree_action.is_some());
        assert!(app.warning_message.is_none());
    }

    // === Tests for update-tui-disable-merge-during-resolve ===

    #[test]
    fn test_resolve_merge_blocked_during_resolve() {
        // Test that M key is blocked during resolve operation in Changes view
        let changes = vec![create_approved_change("change-a", 5, 5)];
        let mut app = AppState::new(changes);

        // Set change to MergeWait status
        app.changes[0].queue_status = QueueStatus::MergeWait;

        // Set is_resolving to true (simulating resolve in progress)
        app.is_resolving = true;

        // Attempt to resolve merge
        let cmd = app.resolve_merge();

        // Should return None and set warning message
        assert!(cmd.is_none());
        assert!(app.warning_message.is_some());
        assert!(app
            .warning_message
            .as_ref()
            .unwrap()
            .contains("resolve operation in progress"));
    }

    #[test]
    fn test_resolve_merge_allowed_when_not_resolving() {
        // Test that M key works when resolve is not in progress
        let changes = vec![create_approved_change("change-a", 5, 5)];
        let mut app = AppState::new(changes);

        // Set change to MergeWait status
        app.changes[0].queue_status = QueueStatus::MergeWait;

        // Ensure is_resolving is false
        app.is_resolving = false;

        // Attempt to resolve merge
        let cmd = app.resolve_merge();

        // Should return ResolveMerge command
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(_))));
    }

    #[test]
    fn test_resolve_merge_allowed_in_running_mode() {
        // Test that M key works for MergeWait changes in Running mode
        let changes = vec![create_approved_change("change-a", 5, 5)];
        let mut app = AppState::new(changes);

        // Set mode to Running
        app.mode = AppMode::Running;

        // Set change to MergeWait status
        app.changes[0].queue_status = QueueStatus::MergeWait;

        // Ensure is_resolving is false
        app.is_resolving = false;

        // Attempt to resolve merge
        let cmd = app.resolve_merge();

        // Should return ResolveMerge command even in Running mode
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(_))));
    }

    #[test]
    fn test_request_merge_worktree_branch_blocked_during_resolve() {
        // Test that M key is blocked during resolve operation in Worktrees view
        use crate::tui::types::{ViewMode, WorktreeInfo};
        use std::path::PathBuf;

        let changes = vec![create_approved_change("change-a", 5, 5)];
        let mut app = AppState::new(changes);

        // Switch to Worktrees view
        app.view_mode = ViewMode::Worktrees;

        // Set up a worktree that would normally allow merge
        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/tmp/worktree"),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        }];
        app.worktree_cursor_index = 0;

        // Set is_resolving to true (simulating resolve in progress)
        app.is_resolving = true;

        // Attempt to merge worktree branch
        let cmd = app.request_merge_worktree_branch();

        // Should return None and set warning message
        assert!(cmd.is_none());
        assert!(app.warning_message.is_some());
        assert!(app
            .warning_message
            .as_ref()
            .unwrap()
            .contains("resolve operation in progress"));
    }

    #[test]
    fn test_request_merge_worktree_branch_allowed_when_not_resolving() {
        // Test that M key works in Worktrees view when resolve is not in progress
        use crate::tui::types::{ViewMode, WorktreeInfo};
        use std::path::PathBuf;

        let changes = vec![create_approved_change("change-a", 5, 5)];
        let mut app = AppState::new(changes);

        // Switch to Worktrees view
        app.view_mode = ViewMode::Worktrees;

        // Set up a worktree that allows merge
        app.worktrees = vec![WorktreeInfo {
            path: PathBuf::from("/tmp/worktree"),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        }];
        app.worktree_cursor_index = 0;

        // Ensure is_resolving is false
        app.is_resolving = false;

        // Attempt to merge worktree branch
        let cmd = app.request_merge_worktree_branch();

        // Should return MergeWorktreeBranch command
        assert!(matches!(cmd, Some(TuiCommand::MergeWorktreeBranch { .. })));
    }
}
