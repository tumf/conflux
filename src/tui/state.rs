//! State management for the TUI
//!
//! This module contains AppState and ChangeState implementations,
//! organized into submodules by responsibility.
//!
//! ## Shared State Integration
//!
//! The TUI can reference the shared orchestration state from `crate::orchestration::state::OrchestratorState`
//! for unified state tracking across TUI and Web interfaces. The shared state provides:
//! - Pending/archived change tracking
//! - Apply count tracking per change
//! - Current change being processed
//! - Iteration counters
//!
//! Both TUI and Web states are updated via `ExecutionEvent` messages, ensuring consistency.

use crate::openspec::Change;
use crate::task_parser;
use crate::tui::events::{LogEntry, LogLevel, OrchestratorEvent, TuiCommand};
use crate::tui::types::{AppMode, QueueStatus, StopMode, ViewMode, WorktreeAction, WorktreeInfo};
use crate::vcs::GitWorkspaceManager;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

// ============================================================================
// Constants
// ============================================================================

/// Auto-refresh interval in seconds
pub const AUTO_REFRESH_INTERVAL_SECS: u64 = 5;

/// Maximum number of log entries to keep
pub const MAX_LOG_ENTRIES: usize = 1000;

// ============================================================================
// Type Definitions
// ============================================================================

/// Warning popup content
pub struct WarningPopup {
    pub title: String,
    pub message: String,
}

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
    /// Reference to shared orchestration state (for unified state tracking)
    /// TUI can query this for pending/archived status, apply counts, etc.
    pub shared_orchestrator_state:
        Option<std::sync::Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>>,
}

// ============================================================================
// ChangeState Implementation
// ============================================================================

impl ChangeState {
    /// Create a new ChangeState from a Change
    ///
    /// Note: This method initializes state from the Change object. Task progress
    /// is synchronized with shared orchestrator state in update_changes(), which
    /// populates OrchestratorState::task_progress() from fetched changes and then
    /// queries it back when updating UI state. This ensures consistency between
    /// TUI and orchestrator for progress tracking.
    pub fn from_change(change: &Change, selected: bool) -> Self {
        Self {
            id: change.id.clone(),
            // Initial values from Change object; synchronized with shared state in update_changes()
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

// ============================================================================
// AppState Core Implementation
// ============================================================================

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
            shared_orchestrator_state: None,
        }
    }

    /// Set reference to shared orchestration state for unified tracking.
    /// This allows TUI to query core orchestration state (pending/archived, apply counts, etc.)
    pub fn set_shared_state(
        &mut self,
        shared_state: std::sync::Arc<
            tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>,
        >,
    ) {
        self.shared_orchestrator_state = Some(shared_state);
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
        debug!(
            "request_merge_worktree_branch called: view_mode={:?}, worktrees_len={}, cursor_index={}",
            self.view_mode,
            self.worktrees.len(),
            self.worktree_cursor_index
        );

        // Validate view mode
        if let guards::MergeGuardResult::Blocked(msg) = guards::validate_view_mode(self.view_mode) {
            debug!(
                "Merge blocked: view_mode is {:?}, not Worktrees",
                self.view_mode
            );
            self.warning_message = Some(msg);
            return None;
        }

        // Validate not resolving
        if let guards::MergeGuardResult::Blocked(msg) =
            guards::validate_not_resolving(self.is_resolving)
        {
            debug!("Merge blocked: resolve operation in progress");
            self.warning_message = Some(msg);
            return None;
        }

        // Validate worktrees not empty
        if let guards::MergeGuardResult::Blocked(msg) =
            guards::validate_worktrees_not_empty(self.worktrees.len())
        {
            debug!("Merge blocked: worktrees list is empty");
            self.warning_message = Some(msg);
            return None;
        }

        // Validate cursor in bounds
        if let guards::MergeGuardResult::Blocked(msg) =
            guards::validate_cursor_in_bounds(self.worktree_cursor_index, self.worktrees.len())
        {
            debug!(
                "Merge blocked: cursor out of range: {} >= {}",
                self.worktree_cursor_index,
                self.worktrees.len()
            );
            self.warning_message = Some(msg);
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

        // Validate worktree is mergeable
        if let guards::MergeGuardResult::Blocked(msg) =
            guards::validate_worktree_mergeable(worktree)
        {
            debug!("Merge blocked: worktree validation failed");
            self.warning_message = Some(msg);
            return None;
        }

        // Get worktree path and branch name
        let path = worktree.path.clone();
        let branch_name = worktree.branch.clone();

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

        // Validate that the change can be toggled
        if let guards::ToggleGuardResult::Blocked(msg) = guards::validate_change_toggleable(
            change.is_approved,
            change.is_parallel_eligible,
            self.parallel_mode,
            &change.queue_status,
            &change.id,
        ) {
            self.warning_message = Some(msg);
            return None;
        }

        // Dispatch to mode-specific handlers
        match self.mode {
            AppMode::Select => {
                match guards::handle_toggle_select_mode(change, &mut self.new_change_count) {
                    guards::ToggleActionResult::StateOnly(log_msg) => {
                        if let Some(msg) = log_msg {
                            self.add_log(LogEntry::info(msg));
                        }
                        None
                    }
                    guards::ToggleActionResult::Command(cmd, log_msg) => {
                        if let Some(msg) = log_msg {
                            self.add_log(LogEntry::info(msg));
                        }
                        Some(cmd)
                    }
                    guards::ToggleActionResult::None => None,
                }
            }
            AppMode::Running => {
                match guards::handle_toggle_running_mode(change, &mut self.new_change_count) {
                    guards::ToggleActionResult::StateOnly(log_msg) => {
                        if let Some(msg) = log_msg {
                            self.add_log(LogEntry::info(msg));
                        }
                        None
                    }
                    guards::ToggleActionResult::Command(cmd, log_msg) => {
                        if let Some(msg) = log_msg {
                            self.add_log(LogEntry::info(msg));
                        }
                        Some(cmd)
                    }
                    guards::ToggleActionResult::None => None,
                }
            }
            AppMode::Stopped => {
                match guards::handle_toggle_stopped_mode(change, &mut self.new_change_count) {
                    guards::ToggleActionResult::StateOnly(log_msg) => {
                        if let Some(msg) = log_msg {
                            self.add_log(LogEntry::info(msg));
                        }
                        None
                    }
                    guards::ToggleActionResult::Command(cmd, log_msg) => {
                        if let Some(msg) = log_msg {
                            self.add_log(LogEntry::info(msg));
                        }
                        Some(cmd)
                    }
                    guards::ToggleActionResult::None => None,
                }
            }
            AppMode::Stopping
            | AppMode::Error
            | AppMode::ConfirmWorktreeDelete
            | AppMode::QrPopup => None,
        }
    }

    /// Check if resolve operation is available for the current cursor position.
    ///
    /// This method centralizes the logic for determining if "M: resolve" should be shown
    /// and if resolve_merge() can be executed.
    pub fn is_resolve_available(&self) -> bool {
        // Must have valid cursor position
        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return false;
        }

        // Must be in correct mode
        if !matches!(
            self.mode,
            AppMode::Select | AppMode::Stopped | AppMode::Running
        ) {
            return false;
        }

        // Must not be currently resolving
        if self.is_resolving {
            return false;
        }

        // Current change must be in MergeWait status
        let change = &self.changes[self.cursor_index];
        matches!(change.queue_status, QueueStatus::MergeWait)
    }

    /// Trigger merge resolution for the selected change when applicable.
    pub fn resolve_merge(&mut self) -> Option<TuiCommand> {
        // Use centralized availability check
        if !self.is_resolve_available() {
            if self.is_resolving {
                self.warning_message =
                    Some("Cannot merge: resolve operation in progress".to_string());
            }
            return None;
        }

        let change = &mut self.changes[self.cursor_index];
        let change_id = change.id.clone();
        // Transition to ResolveWait immediately after M key press
        change.queue_status = QueueStatus::ResolveWait;
        Some(TuiCommand::ResolveMerge(change_id))
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

        // Block approval toggle for active (in-flight) changes
        // MergeWait and ResolveWait allow approval toggle (without queue changes)
        if change.queue_status.is_active() {
            self.warning_message = Some(format!(
                "Cannot change approval for change '{}' while it is {}",
                change.id,
                change.queue_status.display()
            ));
            debug!("toggle_approval: blocked by is_active status");
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
                    // Approved → unapproved
                    // For wait states, do not touch queue status or DynamicQueue.
                    if matches!(
                        change.queue_status,
                        QueueStatus::MergeWait | QueueStatus::ResolveWait
                    ) {
                        Some(TuiCommand::UnapproveOnly(id))
                    } else {
                        Some(TuiCommand::UnapproveAndDequeue(id))
                    }
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
                    // Approved → unapproved
                    // For wait states, do not touch queue status or DynamicQueue.
                    if matches!(
                        change.queue_status,
                        QueueStatus::MergeWait | QueueStatus::ResolveWait
                    ) {
                        Some(TuiCommand::UnapproveOnly(id))
                    } else {
                        Some(TuiCommand::UnapproveAndDequeue(id))
                    }
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
}

// ============================================================================
// Mode-related Methods
// ============================================================================

impl AppState {
    /// Toggle parallel mode (only if git is available)
    ///
    /// Returns true if the mode was toggled, false if git is not available
    /// or if the mode cannot be changed in current state.
    pub fn toggle_parallel_mode(&mut self) -> bool {
        // Only allow toggling in Select or Stopped mode
        if !matches!(self.mode, AppMode::Select | AppMode::Stopped) {
            self.warning_message = Some("Cannot toggle parallel mode while processing".to_string());
            return false;
        }

        // Check if parallel execution is available (git)
        if !self.parallel_available {
            self.warning_message = Some("Parallel mode not available (requires git)".to_string());
            return false;
        }

        self.parallel_mode = !self.parallel_mode;
        let status = if self.parallel_mode {
            "enabled"
        } else {
            "disabled"
        };

        if self.parallel_mode {
            let mut removed = Vec::new();
            for change in &mut self.changes {
                if !change.is_parallel_eligible && change.selected {
                    change.selected = false;
                    if matches!(change.queue_status, QueueStatus::Queued) {
                        change.queue_status = QueueStatus::NotQueued;
                    }
                    removed.push(change.id.clone());
                }
            }
            if !removed.is_empty() {
                self.warning_message = Some(format!(
                    "Removed uncommitted changes from queue in parallel mode: {}",
                    removed.join(", ")
                ));
            }
        }

        self.add_log(LogEntry::info(format!("Parallel mode {}", status)));
        true
    }

    /// Reset stop/cancel state before a new run
    pub fn reset_for_run(&mut self) {
        self.stop_mode = StopMode::None;
        self.current_change = None;
        self.error_change_id = None;
        self.orchestration_started_at = Some(Instant::now());
        self.orchestration_elapsed = None;
    }

    /// Start processing selected changes
    pub fn start_processing(&mut self) -> Option<TuiCommand> {
        if self.mode != AppMode::Select {
            return None;
        }

        // Exclude MergeWait and ResolveWait from StartProcessing
        // These changes require explicit resolve operation (M key)
        let selected: Vec<String> = self
            .changes
            .iter()
            .filter(|c| {
                c.selected
                    && !matches!(
                        c.queue_status,
                        QueueStatus::MergeWait | QueueStatus::ResolveWait
                    )
            })
            .map(|c| c.id.clone())
            .collect();

        if self.parallel_mode {
            let ineligible: Vec<String> = self
                .changes
                .iter()
                .filter(|c| {
                    c.selected
                        && !c.is_parallel_eligible
                        && !matches!(
                            c.queue_status,
                            QueueStatus::MergeWait | QueueStatus::ResolveWait
                        )
                })
                .map(|c| c.id.clone())
                .collect();
            if !ineligible.is_empty() {
                self.warning_message = Some(format!(
                    "Parallel mode requires committed changes. Uncommitted: {}",
                    ineligible.join(", ")
                ));
                return None;
            }
        }

        if selected.is_empty() {
            self.warning_message = Some("No changes selected".to_string());
            return None;
        }

        // Mark selected changes as queued (excluding MergeWait/ResolveWait)
        for change in &mut self.changes {
            if change.selected
                && !matches!(
                    change.queue_status,
                    QueueStatus::MergeWait | QueueStatus::ResolveWait
                )
            {
                change.queue_status = QueueStatus::Queued;
            }
        }

        self.reset_for_run();
        self.mode = AppMode::Running;
        self.add_log(LogEntry::info(format!(
            "Starting processing {} change(s)",
            selected.len()
        )));

        Some(TuiCommand::StartProcessing(selected))
    }

    /// Resume processing from Stopped mode
    /// Converts execution-marked (selected) changes to Queued and starts processing
    pub fn resume_processing(&mut self) -> Option<TuiCommand> {
        if self.mode != AppMode::Stopped {
            return None;
        }

        // Find execution-marked changes (selected=true, queue_status=NotQueued)
        let marked_ids: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.selected && matches!(c.queue_status, QueueStatus::NotQueued))
            .map(|c| c.id.clone())
            .collect();

        if marked_ids.is_empty() {
            self.warning_message = Some("No changes marked for execution".to_string());
            return None;
        }

        // Convert execution-marked changes to Queued
        for change in &mut self.changes {
            if marked_ids.contains(&change.id) {
                change.queue_status = QueueStatus::Queued;
            }
        }

        self.reset_for_run();
        self.mode = AppMode::Running;
        self.add_log(LogEntry::info(format!(
            "Resuming processing {} change(s)...",
            marked_ids.len()
        )));

        Some(TuiCommand::StartProcessing(marked_ids))
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
        self.reset_for_run();
        self.mode = AppMode::Running;

        Some(TuiCommand::StartProcessing(error_ids))
    }
}

// ============================================================================
// Log Management
// ============================================================================

impl AppState {
    /// Get the latest log entry for a specific change_id
    ///
    /// Returns the most recent log entry that matches the given change_id.
    /// Used for displaying log previews in the change list.
    pub fn get_latest_log_for_change(&self, change_id: &str) -> Option<&LogEntry> {
        self.logs
            .iter()
            .rev()
            .find(|entry| entry.change_id.as_deref() == Some(change_id))
    }

    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        // Send to tracing for debug file output (if --logs enabled)
        // Include change_id, operation, iteration, and workspace_path in tracing output for context matching
        let change_id = entry.change_id.as_deref().unwrap_or("-");
        let operation = entry.operation.as_deref().unwrap_or("-");
        let iteration = entry.iteration.unwrap_or(0);
        let workspace_path = entry.workspace_path.as_deref().unwrap_or("-");

        match entry.level {
            LogLevel::Info | LogLevel::Success => {
                info!(
                    target: "tui_log",
                    change_id = change_id,
                    operation = operation,
                    iteration = iteration,
                    workspace_path = workspace_path,
                    "{}",
                    entry.message
                );
            }
            LogLevel::Warn => {
                warn!(
                    target: "tui_log",
                    change_id = change_id,
                    operation = operation,
                    iteration = iteration,
                    workspace_path = workspace_path,
                    "{}",
                    entry.message
                );
            }
            LogLevel::Error => {
                error!(
                    target: "tui_log",
                    change_id = change_id,
                    operation = operation,
                    iteration = iteration,
                    workspace_path = workspace_path,
                    "{}",
                    entry.message
                );
            }
        }

        self.logs.push(entry);

        // Handle buffer trimming when exceeding max entries
        if self.logs.len() > MAX_LOG_ENTRIES {
            self.logs.remove(0);
        }

        // Auto-scroll to bottom if enabled, otherwise freeze view position
        if self.log_auto_scroll {
            self.log_scroll_offset = 0;
        } else {
            // When auto-scroll is disabled, freeze the displayed log range
            // by incrementing offset for new log additions
            self.log_scroll_offset += 1;

            // When buffer is trimmed, we don't decrement offset because we want
            // to keep showing the same log content (freeze position)
            // However, if trimming pushed us out of range, clamp to oldest available
            let max_offset = self.logs.len().saturating_sub(1);
            if self.log_scroll_offset > max_offset {
                self.log_scroll_offset = max_offset;
            }
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
}

// ============================================================================
// Event Handling
// ============================================================================

impl AppState {
    /// Handle an event from the orchestrator
    ///
    /// This is the main entry point for event handling, dispatching to specialized handlers.
    pub fn handle_orchestrator_event(&mut self, event: OrchestratorEvent) {
        match event {
            // Processing lifecycle events
            OrchestratorEvent::ProcessingStarted(id) => self.handle_processing_started(id),
            OrchestratorEvent::ProcessingCompleted(id) => self.handle_processing_completed(id),
            OrchestratorEvent::ProcessingError { id, error } => {
                self.handle_processing_error(id, error)
            }
            OrchestratorEvent::AllCompleted => self.handle_all_completed(),
            OrchestratorEvent::Stopped => self.handle_stopped(),

            // Progress events
            OrchestratorEvent::ProgressUpdated {
                change_id,
                completed,
                total,
            } => self.handle_progress_updated(change_id, completed, total),

            // Stage events
            OrchestratorEvent::ApplyStarted { change_id, command } => {
                self.handle_apply_started(change_id, command)
            }
            OrchestratorEvent::ArchiveStarted { change_id, command } => {
                self.handle_archive_started(change_id, command)
            }
            OrchestratorEvent::ChangeArchived(id) => self.handle_change_archived(id),
            OrchestratorEvent::ResolveStarted { change_id, command } => {
                self.handle_resolve_started(change_id, command)
            }
            OrchestratorEvent::ResolveCompleted {
                change_id,
                worktree_change_ids,
            } => self.handle_resolve_completed(change_id, worktree_change_ids),
            OrchestratorEvent::MergeCompleted {
                change_id,
                revision: _,
            } => self.handle_merge_completed(change_id),
            OrchestratorEvent::BranchMergeStarted { branch_name } => {
                self.handle_branch_merge_started(branch_name)
            }
            OrchestratorEvent::BranchMergeCompleted { branch_name } => {
                self.handle_branch_merge_completed(branch_name)
            }

            // Completion and error events
            OrchestratorEvent::ApplyFailed { change_id, error } => {
                self.handle_apply_failed(change_id, error)
            }
            OrchestratorEvent::ArchiveFailed { change_id, error } => {
                self.handle_archive_failed(change_id, error)
            }
            OrchestratorEvent::ResolveFailed { change_id, error } => {
                self.handle_resolve_failed(change_id, error)
            }
            OrchestratorEvent::MergeDeferred { change_id, reason } => {
                self.handle_merge_deferred(change_id, reason)
            }
            OrchestratorEvent::AcceptanceStarted { change_id, command } => {
                self.handle_acceptance_started(change_id, command)
            }
            OrchestratorEvent::AcceptanceCompleted { change_id } => {
                self.handle_acceptance_completed(change_id)
            }
            OrchestratorEvent::BranchMergeFailed { branch_name, error } => {
                self.handle_branch_merge_failed(branch_name, error)
            }

            // Refresh events
            OrchestratorEvent::ChangesRefreshed {
                changes,
                committed_change_ids,
                worktree_change_ids,
                worktree_paths,
                worktree_not_ahead_ids,
                merge_wait_ids,
            } => self.handle_changes_refreshed(
                changes,
                committed_change_ids,
                worktree_change_ids,
                worktree_paths,
                worktree_not_ahead_ids,
                merge_wait_ids,
            ),
            OrchestratorEvent::WorktreesRefreshed { worktrees } => {
                self.handle_worktrees_refreshed(worktrees)
            }
            OrchestratorEvent::ChangeSkipped { change_id, reason } => {
                self.handle_change_skipped(change_id, reason)
            }
            OrchestratorEvent::DependencyBlocked {
                change_id,
                dependency_ids: _,
            } => self.handle_dependency_blocked(change_id),
            OrchestratorEvent::DependencyResolved { change_id } => {
                self.handle_dependency_resolved(change_id)
            }

            // Output events
            OrchestratorEvent::ApplyOutput {
                change_id,
                output,
                iteration,
            } => self.handle_apply_output(change_id, output, iteration),
            OrchestratorEvent::ArchiveOutput {
                change_id,
                output,
                iteration,
            } => self.handle_archive_output(change_id, output, iteration),
            OrchestratorEvent::AcceptanceOutput {
                change_id,
                output,
                iteration,
            } => self.handle_acceptance_output(change_id, output, iteration),
            OrchestratorEvent::AnalysisOutput { output, iteration } => {
                self.handle_analysis_output(output, iteration)
            }
            OrchestratorEvent::ResolveOutput {
                change_id,
                output,
                iteration,
            } => self.handle_resolve_output(change_id, output, iteration),

            // Message events
            OrchestratorEvent::Log(entry) => self.handle_log(entry),
            OrchestratorEvent::Warning { title, message } => self.handle_warning(title, message),
            OrchestratorEvent::Error { message } => self.handle_error(message),

            // Ignore other parallel-specific events that don't affect TUI state
            _ => {
                // Other events (workspace, merge, group events) are for status tracking
                // and don't need to be displayed in the log
            }
        }
    }

    // Processing lifecycle event handlers
    fn handle_processing_started(&mut self, id: String) {
        self.current_change = Some(id.clone());
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Applying;
            change.started_at = Some(Instant::now());
            change.elapsed_time = None;
        }
        self.add_log(LogEntry::info(format!("Processing: {}", id)));
    }

    fn handle_processing_completed(&mut self, id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Archiving;
            // Reload final progress from tasks.md to preserve it
            if let Ok(progress) = task_parser::parse_change(&id) {
                change.completed_tasks = progress.completed;
                change.total_tasks = progress.total;
            }
        }
        self.add_log(LogEntry::success(format!("Completed: {}", id)));
    }

    fn handle_processing_error(&mut self, id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Error(error.clone());
            change.selected = true;
            // Record elapsed time on error
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!("Error in {}: {}", id, error)));
        // ProcessingError does NOT transition to Error mode (change-level failure only)
        // AppMode remains Running to allow processing continuation
        // error_change_id is set to track which change failed, but mode stays Running
        self.error_change_id = Some(id.clone());
        self.current_change = None;
    }

    fn handle_all_completed(&mut self) {
        if matches!(self.mode, AppMode::Stopped | AppMode::Error) {
            if let Some(started) = self.orchestration_started_at {
                self.orchestration_elapsed = Some(started.elapsed());
            }
            return;
        }

        self.mode = AppMode::Select;
        self.current_change = None;
        self.stop_mode = StopMode::None;
        // Record final orchestration time
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }
        self.add_log(LogEntry::success("All changes processed successfully"));
    }

    fn handle_stopped(&mut self) {
        self.mode = AppMode::Stopped;
        self.current_change = None;
        self.stop_mode = StopMode::None;
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }
        // Reset queue status to NotQueued while preserving execution mark (selected)
        // This implements the policy: queued only during active execution
        for change in &mut self.changes {
            if matches!(
                change.queue_status,
                QueueStatus::Applying
                    | QueueStatus::Accepting
                    | QueueStatus::Archiving
                    | QueueStatus::Queued
            ) {
                // Record elapsed time before resetting status (for in-flight changes)
                if let Some(started) = change.started_at {
                    change.elapsed_time = Some(started.elapsed());
                }
                // Reset to NotQueued but preserve execution mark (selected field)
                change.queue_status = QueueStatus::NotQueued;
                // Keep change.selected as-is to preserve execution mark
            }
        }
        self.add_log(LogEntry::warn("Processing stopped"));
    }

    // Progress event handler
    fn handle_progress_updated(&mut self, change_id: String, completed: u32, total: u32) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            // Update progress for all states when valid data is available.
            // Only update if total > 0 to avoid resetting progress on retrieval failure.
            // Progress retrieval failure (0/0) should preserve existing progress.
            if total > 0 {
                change.completed_tasks = completed;
                change.total_tasks = total;
            }
            // Never modify queue_status here.
            // In Stopped mode, task completion does not trigger auto-queue.
        }
    }

    // Stage event handlers
    fn handle_apply_started(&mut self, change_id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Applying;
            change.elapsed_time = None;
        }
        self.add_log(
            LogEntry::info(format!("Apply started: {}", change_id))
                .with_operation("apply")
                .with_change_id(&change_id),
        );
        self.add_log(
            LogEntry::info(format!("  Command: {}", command))
                .with_operation("apply")
                .with_change_id(&change_id),
        );
    }

    fn handle_archive_started(&mut self, id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Archiving;
            // Reload final progress from tasks.md to preserve it before archiving
            // Use comprehensive fallback to read from uncommitted changes
            let worktree_path = self.worktree_paths.get(&id).map(|p| p.as_path());
            match task_parser::parse_progress_with_fallback(&id, worktree_path) {
                Ok(progress) => {
                    // Only update if valid progress (not 0/0)
                    if progress.total > 0 {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If 0/0, preserve existing progress
                }
                Err(_) => {
                    // Preserve existing progress if parsing fails
                }
            }
        }
        self.add_log(
            LogEntry::info(format!("Archiving: {}", id))
                .with_operation("archive")
                .with_change_id(&id),
        );
        self.add_log(
            LogEntry::info(format!("  Command: {}", command))
                .with_operation("archive")
                .with_change_id(&id),
        );
    }

    fn handle_change_archived(&mut self, id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Archived;
            // Record final elapsed time
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            // Reload progress from archived tasks.md (with fallback guard)
            // Use comprehensive fallback to read from uncommitted archive
            let worktree_path = self.worktree_paths.get(&id).map(|p| p.as_path());
            match task_parser::parse_progress_with_fallback(&id, worktree_path) {
                Ok(progress) => {
                    // Only update if valid progress (not 0/0)
                    if progress.total > 0 {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If 0/0, preserve existing progress
                }
                Err(_) => {
                    // Preserve existing progress if parsing fails
                }
            }
        }
        self.add_log(LogEntry::info(format!("Archived: {}", id)));
    }

    fn handle_resolve_started(&mut self, change_id: String, command: String) {
        self.is_resolving = true;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Resolving;
            change.elapsed_time = None;
        }
        self.add_log(
            LogEntry::info(format!("Resolving merge for '{}'", change_id))
                .with_operation("resolve")
                .with_change_id(&change_id),
        );
        self.add_log(
            LogEntry::info(format!("  Command: {}", command))
                .with_operation("resolve")
                .with_change_id(&change_id),
        );
    }

    fn handle_resolve_completed(
        &mut self,
        change_id: String,
        worktree_change_ids: Option<HashSet<String>>,
    ) {
        self.is_resolving = false;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Merged;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            // Reload progress from archived tasks.md (with fallback guard)
            // Use comprehensive fallback to read from uncommitted archive
            let worktree_path = self.worktree_paths.get(&change_id).map(|p| p.as_path());
            match task_parser::parse_progress_with_fallback(&change_id, worktree_path) {
                Ok(progress) => {
                    // Only update if valid progress (not 0/0)
                    if progress.total > 0 {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If 0/0, preserve existing progress
                }
                Err(_) => {
                    // Preserve existing progress if parsing fails
                }
            }
        }
        if let Some(ids) = worktree_change_ids {
            self.apply_worktree_status(&ids);
        }
        self.add_log(LogEntry::success(format!(
            "Merge resolved for '{}'",
            change_id
        )));
    }

    fn handle_merge_completed(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Merged;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
            // Reload progress from archived tasks.md (with fallback guard)
            // Use comprehensive fallback to read from uncommitted archive
            let worktree_path = self.worktree_paths.get(&change_id).map(|p| p.as_path());
            match task_parser::parse_progress_with_fallback(&change_id, worktree_path) {
                Ok(progress) => {
                    // Only update if valid progress (not 0/0)
                    if progress.total > 0 {
                        change.completed_tasks = progress.completed;
                        change.total_tasks = progress.total;
                    }
                    // If 0/0, preserve existing progress
                }
                Err(_) => {
                    // Preserve existing progress if parsing fails
                }
            }
        }
        self.add_log(LogEntry::success(format!(
            "Merge completed for '{}'",
            change_id
        )));
    }

    fn handle_branch_merge_started(&mut self, branch_name: String) {
        self.add_log(LogEntry::info(format!(
            "merging branch '{}'...",
            branch_name
        )));
        // Set is_merging flag on the worktree with this branch
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = true;
        }
    }

    fn handle_branch_merge_completed(&mut self, branch_name: String) {
        self.add_log(LogEntry::success(format!(
            "merged branch '{}' successfully",
            branch_name
        )));
        // Clear is_merging flag and update has_commits_ahead
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = false;
            wt.has_commits_ahead = false; // Merged to base, so no longer ahead
        }
    }

    // Completion and error event handlers
    fn handle_apply_failed(&mut self, change_id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Error(error.clone());
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!(
            "Apply failed for {}: {}",
            change_id, error
        )));
    }

    fn handle_archive_failed(&mut self, change_id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Error(error.clone());
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!(
            "Archive failed for {}: {}",
            change_id, error
        )));
    }

    fn handle_resolve_failed(&mut self, change_id: String, error: String) {
        self.is_resolving = false;
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::MergeWait;
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        let message = format!("Failed to resolve merge for '{}': {}", change_id, error);
        self.warning_popup = Some(WarningPopup {
            title: "Merge resolve failed".to_string(),
            message: message.clone(),
        });
        self.add_log(LogEntry::error(message));
    }

    fn handle_merge_deferred(&mut self, change_id: String, reason: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::MergeWait;
        }
        self.add_log(LogEntry::warn(format!(
            "Merge deferred for {}: {}",
            change_id, reason
        )));
    }

    fn handle_acceptance_started(&mut self, change_id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Accepting;
        }
        self.add_log(
            LogEntry::info(format!("Acceptance started: {}", change_id))
                .with_operation("acceptance")
                .with_change_id(&change_id),
        );
        self.add_log(
            LogEntry::info(format!("  Command: {}", command))
                .with_operation("acceptance")
                .with_change_id(&change_id),
        );
    }

    fn handle_acceptance_completed(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Archiving;
        }
        self.add_log(LogEntry::info(format!(
            "Acceptance completed: {}",
            change_id
        )));
    }

    fn handle_change_skipped(&mut self, change_id: String, reason: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Error(reason.clone());
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::warn(format!("Skipped {}: {}", change_id, reason)));
    }

    fn handle_branch_merge_failed(&mut self, branch_name: String, error: String) {
        self.warning_popup = Some(WarningPopup {
            title: "Merge failed".to_string(),
            message: format!("Failed to merge '{}': {}", branch_name, error),
        });
        self.add_log(LogEntry::error(format!(
            "Merge failed for '{}': {}",
            branch_name, error
        )));
        // Clear is_merging flag on failure
        if let Some(wt) = self.worktrees.iter_mut().find(|w| w.branch == branch_name) {
            wt.is_merging = false;
        }
    }

    fn handle_dependency_blocked(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.queue_status = QueueStatus::Blocked;
        }
        self.add_log(LogEntry::info(format!(
            "Change '{}' blocked by dependencies",
            change_id
        )));
    }

    fn handle_dependency_resolved(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            // Only update if currently blocked, otherwise preserve the current state
            if change.queue_status == QueueStatus::Blocked {
                change.queue_status = QueueStatus::Queued;
            }
        }
        self.add_log(LogEntry::info(format!(
            "Change '{}' dependencies resolved",
            change_id
        )));
    }

    // Refresh event handlers
    fn handle_changes_refreshed(
        &mut self,
        changes: Vec<Change>,
        committed_change_ids: HashSet<String>,
        worktree_change_ids: HashSet<String>,
        worktree_paths: HashMap<String, PathBuf>,
        worktree_not_ahead_ids: HashSet<String>,
        merge_wait_ids: HashSet<String>,
    ) {
        self.worktree_paths = worktree_paths;
        self.update_changes(changes);
        self.apply_parallel_eligibility(&committed_change_ids);
        self.apply_worktree_status(&worktree_change_ids);
        // Auto-clear MergeWait for changes whose worktrees don't exist or are not ahead
        self.auto_clear_merge_wait(&worktree_change_ids, &worktree_not_ahead_ids);
        // Apply MergeWait status for archived changes waiting for merge
        self.apply_merge_wait_status(&merge_wait_ids);
    }

    fn handle_worktrees_refreshed(&mut self, worktrees: Vec<WorktreeInfo>) {
        self.worktrees = worktrees;

        // Adjust cursor if it's out of bounds
        if self.worktree_cursor_index >= self.worktrees.len() && !self.worktrees.is_empty() {
            self.worktree_cursor_index = self.worktrees.len() - 1;
        }
    }

    // Output event handlers
    fn handle_apply_output(&mut self, change_id: String, output: String, iteration: Option<u32>) {
        // Update iteration number in change state
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.iteration_number = iteration;
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("apply")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    fn handle_archive_output(&mut self, change_id: String, output: String, iteration: u32) {
        // Update iteration number in change state
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.iteration_number = Some(iteration);
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("archive")
                .with_iteration(iteration),
        );
    }

    fn handle_acceptance_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        // Update iteration number in change state
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.iteration_number = iteration;
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("acceptance")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    fn handle_analysis_output(&mut self, output: String, iteration: u32) {
        self.add_log(
            LogEntry::info(output)
                .with_operation("analysis")
                .with_iteration(iteration),
        );
    }

    fn handle_resolve_output(&mut self, change_id: String, output: String, iteration: Option<u32>) {
        // Update iteration number in change state
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.iteration_number = iteration;
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(&change_id)
                .with_operation("resolve")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    // Message event handlers
    fn handle_log(&mut self, entry: LogEntry) {
        self.add_log(entry);
    }

    fn handle_warning(&mut self, title: String, message: String) {
        // For uncommitted changes warnings in TUI, only log without popup
        if title != "Uncommitted Changes Detected" {
            self.warning_popup = Some(WarningPopup {
                title: title.clone(),
                message: message.clone(),
            });
        }
        self.add_log(LogEntry::warn(message));
    }

    fn handle_error(&mut self, message: String) {
        self.add_log(LogEntry::error(message.clone()));
        self.mode = AppMode::Error;
        self.error_change_id = None;
        self.current_change = None;
    }
}

// ============================================================================
// Helper Methods
// ============================================================================

impl AppState {
    /// Update changes from a refresh
    ///
    /// Updates task progress (completed_tasks, total_tasks) from fetched changes and
    /// enriches change metadata from shared orchestration state when available (apply counts,
    /// pending/archived tracking).
    ///
    /// IMPORTANT: This method does NOT modify queue_status. In Stopped mode, task completion
    /// does not trigger auto-queue. Changes are only queued through explicit user action (Space key).
    ///
    /// Note: Task progress is synchronized with shared orchestration state.
    /// When changes are fetched from openspec CLI, their task progress is written
    /// to OrchestratorState::task_progress(). When updating UI state, progress is
    /// read from shared state to ensure consistency across TUI and orchestrator.
    fn update_changes(&mut self, fetched_changes: Vec<Change>) {
        // Populate shared orchestration state with task progress from fetched changes
        // This ensures shared state reflects the current file-system state from openspec list
        if let Some(shared_state) = &self.shared_orchestrator_state {
            if let Ok(mut guard) = shared_state.try_write() {
                for change in &fetched_changes {
                    if change.total_tasks > 0 {
                        guard.set_task_progress(
                            change.id.clone(),
                            change.completed_tasks,
                            change.total_tasks,
                        );
                    }
                }
            }
        }

        // Detect new changes
        let new_ids: Vec<String> = fetched_changes
            .iter()
            .filter(|c| !self.known_change_ids.contains(&c.id))
            .map(|c| c.id.clone())
            .collect();

        // Update existing changes
        for fetched in &fetched_changes {
            if let Some(existing) = self.changes.iter_mut().find(|c| c.id == fetched.id) {
                let was_archived = existing.queue_status == QueueStatus::Archived;
                let is_merge_wait = existing.queue_status == QueueStatus::MergeWait;
                let is_resolve_wait = existing.queue_status == QueueStatus::ResolveWait;

                // Get task progress from shared state (with fallback to fetched data)
                let (completed, total) = if let Some(shared_state) = &self.shared_orchestrator_state
                {
                    if let Ok(guard) = shared_state.try_read() {
                        let progress = guard.task_progress(&fetched.id);
                        if progress.1 > 0 {
                            progress
                        } else {
                            (fetched.completed_tasks, fetched.total_tasks)
                        }
                    } else {
                        (fetched.completed_tasks, fetched.total_tasks)
                    }
                } else {
                    (fetched.completed_tasks, fetched.total_tasks)
                };

                if was_archived {
                    // If change still exists after archiving, it means archive failed
                    // Revert to NotQueued status
                    existing.queue_status = QueueStatus::NotQueued;
                    // Update progress for unarchived changes
                    if total > 0 {
                        existing.completed_tasks = completed;
                        existing.total_tasks = total;
                    }
                    // If total == 0, preserve existing progress
                } else if is_merge_wait {
                    // Preserve MergeWait status during auto-refresh
                    // MergeWait is a persistent state that requires explicit user action (M key)
                    // to transition to Resolving, and should not be cleared by progress updates
                    // Update progress for all states (including MergeWait)
                    if total > 0 {
                        existing.completed_tasks = completed;
                        existing.total_tasks = total;
                    }
                    // If total == 0, preserve existing progress
                } else if is_resolve_wait {
                    // Preserve ResolveWait status during auto-refresh
                    // ResolveWait is a persistent state indicating archive is complete
                    // and the change is waiting for resolve execution
                    // Update progress for ResolveWait changes
                    if total > 0 {
                        existing.completed_tasks = completed;
                        existing.total_tasks = total;
                    }
                    // If total == 0, preserve existing progress
                } else {
                    // Update progress for all other states when valid data is available
                    // Only update if total > 0 to avoid resetting progress on retrieval failure
                    if total > 0 {
                        existing.completed_tasks = completed;
                        existing.total_tasks = total;
                    } else {
                        // fetched.total_tasks == 0: Retrieval failed, preserve existing progress
                        // For archiving/resolving/archived/merged, try worktree fallback
                        let worktree_path =
                            self.worktree_paths.get(&fetched.id).map(|p| p.as_path());

                        match existing.queue_status {
                            QueueStatus::Archiving
                            | QueueStatus::Resolving
                            | QueueStatus::Archived
                            | QueueStatus::Merged => {
                                // Use comprehensive fallback: worktree active -> worktree archive -> base active -> base archive
                                if let Ok(progress) = task_parser::parse_progress_with_fallback(
                                    &fetched.id,
                                    worktree_path,
                                ) {
                                    // Only update if valid progress (not 0/0)
                                    if progress.total > 0 {
                                        existing.completed_tasks = progress.completed;
                                        existing.total_tasks = progress.total;
                                    }
                                    // If 0/0, preserve existing progress
                                }
                                // If fails or returns 0/0, preserve existing progress
                            }
                            _ => {
                                // For all other states: preserve existing progress (do nothing)
                            }
                        }
                    }
                }
            }
        }

        // Add new changes
        for id in &new_ids {
            if let Some(fetched) = fetched_changes.iter().find(|c| &c.id == id) {
                let mut new_state = ChangeState::from_change(fetched, false); // New changes are not selected
                new_state.is_new = true;
                self.changes.push(new_state);
            }
        }

        // Track all known IDs (new + existing)
        self.known_change_ids.extend(new_ids);

        self.new_change_count = self.changes.iter().filter(|c| c.is_new).count();
        self.last_refresh = Instant::now();

        // Enrich change metadata from shared orchestration state if available
        // This provides apply counts (iteration_number) for display
        if let Some(shared_state) = &self.shared_orchestrator_state {
            // Attempt to read shared state, but don't block if lock is held
            if let Ok(guard) = shared_state.try_read() {
                for change in &mut self.changes {
                    // Set iteration_number from apply_count if available
                    // Use monotonic merge: only update if new value is greater than existing
                    let apply_count = guard.apply_count(&change.id);
                    if apply_count > 0 {
                        match change.iteration_number {
                            Some(existing) => {
                                // Only update if new value is greater (monotonic increase)
                                if apply_count > existing {
                                    change.iteration_number = Some(apply_count);
                                }
                            }
                            None => {
                                // No existing value, set the new value
                                change.iteration_number = Some(apply_count);
                            }
                        }
                    }
                }
            }
        }

        // Remove changes that no longer exist (have been archived externally)
        let current_ids: HashSet<String> = fetched_changes.iter().map(|c| c.id.clone()).collect();
        self.changes.retain(|c| {
            // Keep if still exists, apply started in this session, or in a terminal state.
            current_ids.contains(&c.id)
                || c.started_at.is_some()
                || matches!(
                    c.queue_status,
                    QueueStatus::Archiving
                        | QueueStatus::Archived
                        | QueueStatus::Merged
                        | QueueStatus::MergeWait
                        | QueueStatus::Resolving
                        | QueueStatus::ResolveWait
                        | QueueStatus::Error(_)
                )
        });

        // Ensure cursor is valid
        if self.cursor_index >= self.changes.len() && !self.changes.is_empty() {
            self.cursor_index = self.changes.len() - 1;
            self.list_state.select(Some(self.cursor_index));
        }
    }

    /// Auto-clear MergeWait status when conditions are met.
    ///
    /// Clears MergeWait to Queued when:
    /// - Worktree doesn't exist (not in worktree_change_ids), OR
    /// - Worktree exists but is not ahead of base (in worktree_not_ahead_ids)
    fn auto_clear_merge_wait(
        &mut self,
        worktree_change_ids: &HashSet<String>,
        worktree_not_ahead_ids: &HashSet<String>,
    ) {
        let mut cleared_changes = Vec::new();

        for change in &mut self.changes {
            if change.queue_status == QueueStatus::MergeWait {
                let has_worktree = worktree_change_ids.contains(&change.id);
                let not_ahead = worktree_not_ahead_ids.contains(&change.id);

                // Auto-clear conditions:
                // 1. Worktree doesn't exist
                // 2. Worktree exists but not ahead of base
                if !has_worktree || not_ahead {
                    change.queue_status = QueueStatus::Queued;
                    let reason = if !has_worktree {
                        "worktree removed"
                    } else {
                        "worktree merged to base"
                    };
                    cleared_changes.push((change.id.clone(), reason));
                }
            }
        }

        // Log after modifying changes to avoid borrow conflict
        for (id, reason) in cleared_changes {
            self.add_log(LogEntry::info(format!(
                "Auto-cleared MergeWait for '{}': {}",
                id, reason
            )));
        }
    }

    /// Apply MergeWait status for changes detected in WorkspaceState::Archived.
    ///
    /// Sets MergeWait for changes that:
    /// - Have a worktree in WorkspaceState::Archived state
    /// - Are not currently in active processing states (Applying, Archiving, Resolving, ResolveWait)
    ///
    /// This implements idempotent restoration of MergeWait from repository state.
    /// ResolveWait is preserved to avoid overwriting single-launch wait states.
    fn apply_merge_wait_status(&mut self, merge_wait_ids: &HashSet<String>) {
        for change in &mut self.changes {
            if merge_wait_ids.contains(&change.id) {
                // Only set MergeWait if not in active processing or ResolveWait
                // (to avoid overwriting active processing states or single-launch wait)
                if !matches!(
                    change.queue_status,
                    QueueStatus::Applying
                        | QueueStatus::Archiving
                        | QueueStatus::Resolving
                        | QueueStatus::ResolveWait
                ) {
                    change.queue_status = QueueStatus::MergeWait;
                }
            }
        }
    }
}

// ============================================================================
// Guard Logic
// ============================================================================

mod guards {
    use super::{ChangeState, QueueStatus, TuiCommand, ViewMode, WorktreeInfo};

    /// Result type for merge validation
    pub enum MergeGuardResult {
        /// Merge is allowed
        Allowed,
        /// Merge is blocked with a warning message
        Blocked(String),
    }

    /// Validates that the view mode is correct for merge operations
    pub fn validate_view_mode(view_mode: ViewMode) -> MergeGuardResult {
        if view_mode != ViewMode::Worktrees {
            MergeGuardResult::Blocked("Switch to Worktrees view to merge".to_string())
        } else {
            MergeGuardResult::Allowed
        }
    }

    /// Validates that no resolve operation is in progress
    pub fn validate_not_resolving(is_resolving: bool) -> MergeGuardResult {
        if is_resolving {
            MergeGuardResult::Blocked("Cannot merge: resolve operation in progress".to_string())
        } else {
            MergeGuardResult::Allowed
        }
    }

    /// Validates that worktrees list is not empty
    pub fn validate_worktrees_not_empty(worktrees_len: usize) -> MergeGuardResult {
        if worktrees_len == 0 {
            MergeGuardResult::Blocked("No worktrees loaded".to_string())
        } else {
            MergeGuardResult::Allowed
        }
    }

    /// Validates that cursor index is within bounds
    pub fn validate_cursor_in_bounds(
        cursor_index: usize,
        worktrees_len: usize,
    ) -> MergeGuardResult {
        if cursor_index >= worktrees_len {
            MergeGuardResult::Blocked(format!(
                "Cursor out of range: {} >= {}",
                cursor_index, worktrees_len
            ))
        } else {
            MergeGuardResult::Allowed
        }
    }

    /// Validates worktree-specific constraints for merging
    pub fn validate_worktree_mergeable(worktree: &WorktreeInfo) -> MergeGuardResult {
        // Cannot merge main worktree
        if worktree.is_main {
            return MergeGuardResult::Blocked("Cannot merge main worktree".to_string());
        }

        // Cannot merge detached HEAD
        if worktree.is_detached {
            return MergeGuardResult::Blocked("Cannot merge detached HEAD".to_string());
        }

        // Cannot merge if conflicts detected
        if worktree.has_merge_conflict() {
            return MergeGuardResult::Blocked(format!(
                "Cannot merge: {} conflict(s) detected",
                worktree.conflict_file_count()
            ));
        }

        // Branch name must not be empty
        if worktree.branch.is_empty() {
            return MergeGuardResult::Blocked("Cannot merge: no branch name".to_string());
        }

        // Cannot merge if no commits ahead of base branch
        if !worktree.has_commits_ahead {
            return MergeGuardResult::Blocked(
                "Cannot merge: no commits ahead of base branch".to_string(),
            );
        }

        // Cannot merge if already merging (redundant check after has_commits_ahead,
        // but kept for explicit validation)
        if worktree.is_merging {
            return MergeGuardResult::Blocked(
                "Cannot merge: merge already in progress".to_string(),
            );
        }

        MergeGuardResult::Allowed
    }

    /// Result type for toggle selection validation
    pub enum ToggleGuardResult {
        /// Operation is allowed
        Allowed,
        /// Operation is blocked with a warning message
        Blocked(String),
    }

    /// Validates that a change can be toggled for selection
    pub fn validate_change_toggleable(
        is_approved: bool,
        is_parallel_eligible: bool,
        parallel_mode: bool,
        queue_status: &QueueStatus,
        change_id: &str,
    ) -> ToggleGuardResult {
        // Cannot toggle active (in-flight) changes
        if queue_status.is_active() {
            return ToggleGuardResult::Blocked(format!(
                "Cannot toggle change '{}' while it is {}",
                change_id,
                queue_status.display()
            ));
        }

        // Cannot select unapproved changes
        if !is_approved {
            return ToggleGuardResult::Blocked(format!(
                "Cannot queue unapproved change '{}'. Press @ to approve first.",
                change_id
            ));
        }

        // Cannot select uncommitted changes in parallel mode
        if parallel_mode && !is_parallel_eligible {
            return ToggleGuardResult::Blocked(format!(
                "Cannot queue uncommitted change '{}' in parallel mode. Commit it first.",
                change_id
            ));
        }

        // MergeWait and ResolveWait can toggle execution mark (selected)
        // but cannot change queue_status or modify DynamicQueue
        // This is handled by the mode-specific handlers
        ToggleGuardResult::Allowed
    }

    /// Result of toggle selection action
    pub enum ToggleActionResult {
        /// No command needed (state change only), with optional log message
        StateOnly(Option<String>),
        /// Return a TuiCommand, with optional log message
        Command(TuiCommand, Option<String>),
        /// Do nothing (no state change, no command)
        None,
    }

    /// Handle toggle selection in Select mode
    pub fn handle_toggle_select_mode(
        change: &mut ChangeState,
        new_change_count: &mut usize,
    ) -> ToggleActionResult {
        change.selected = !change.selected;
        // Clear NEW flag when user interacts with the change
        if change.is_new {
            change.is_new = false;
            *new_change_count = new_change_count.saturating_sub(1);
        }
        ToggleActionResult::StateOnly(None)
    }

    /// Handle toggle selection in Running mode
    pub fn handle_toggle_running_mode(
        change: &mut ChangeState,
        new_change_count: &mut usize,
    ) -> ToggleActionResult {
        match &change.queue_status {
            QueueStatus::NotQueued => {
                // Add to queue
                change.queue_status = QueueStatus::Queued;
                change.selected = true;
                // Clear NEW flag when user adds to queue
                if change.is_new {
                    change.is_new = false;
                    *new_change_count = new_change_count.saturating_sub(1);
                }
                let id = change.id.clone();
                let log_msg = format!("Added to queue: {}", id);
                ToggleActionResult::Command(TuiCommand::AddToQueue(id), Some(log_msg))
            }
            QueueStatus::Queued => {
                // Remove from queue
                change.queue_status = QueueStatus::NotQueued;
                change.selected = false;
                let id = change.id.clone();
                let log_msg = format!("Removed from queue: {}", id);
                ToggleActionResult::Command(TuiCommand::RemoveFromQueue(id), Some(log_msg))
            }
            QueueStatus::MergeWait | QueueStatus::ResolveWait => {
                // Only toggle execution mark (selected), do not modify queue_status or DynamicQueue
                change.selected = !change.selected;
                // Clear NEW flag when user interacts with the change
                if change.is_new {
                    change.is_new = false;
                    *new_change_count = new_change_count.saturating_sub(1);
                }
                let id = change.id.clone();
                let log_msg = if change.selected {
                    format!("Marked for execution: {}", id)
                } else {
                    format!("Unmarked: {}", id)
                };
                ToggleActionResult::StateOnly(Some(log_msg))
            }
            // Processing, Completed, Archived, Error - cannot change status
            _ => ToggleActionResult::None,
        }
    }

    /// Handle toggle selection in Stopped mode
    pub fn handle_toggle_stopped_mode(
        change: &mut ChangeState,
        new_change_count: &mut usize,
    ) -> ToggleActionResult {
        // In Stopped mode, only toggle execution mark (selected), not queue_status.
        // For wait states (MergeWait/ResolveWait), queue_status MUST remain unchanged.
        // For NotQueued, queue_status remains NotQueued until resume.
        if !matches!(
            change.queue_status,
            QueueStatus::NotQueued | QueueStatus::MergeWait | QueueStatus::ResolveWait
        ) {
            // Cannot modify processing/completed/error states.
            return ToggleActionResult::None;
        }

        // Toggle execution mark only
        change.selected = !change.selected;

        // Clear NEW flag when user interacts with the change
        if change.is_new {
            change.is_new = false;
            *new_change_count = new_change_count.saturating_sub(1);
        }

        let id = change.id.clone();
        let log_msg = if change.selected {
            format!("Marked for execution: {}", id)
        } else {
            format!("Unmarked: {}", id)
        };
        ToggleActionResult::StateOnly(Some(log_msg))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::events::OrchestratorEvent;

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
}
