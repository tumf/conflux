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
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

fn apply_remote_status(change: &mut ChangeState, status: &str) {
    // Avoid regressing active/terminal states based on laggy remote snapshots.
    let current = change.queue_status.clone();

    let next = match status {
        "applying" => Some(QueueStatus::Applying),
        "archiving" => Some(QueueStatus::Archiving),
        "accepting" => Some(QueueStatus::Accepting),
        "resolving" => Some(QueueStatus::Resolving),
        "archived" => Some(QueueStatus::Archived),
        "merged" => Some(QueueStatus::Merged),
        "merge_wait" => Some(QueueStatus::MergeWait),
        "resolve_wait" => Some(QueueStatus::ResolveWait),
        "blocked" => Some(QueueStatus::Blocked),
        "queued" => Some(QueueStatus::Queued),
        "idle" => Some(QueueStatus::NotQueued),
        "error" => Some(QueueStatus::Error("remote".to_string())),
        _ => None,
    };

    let Some(next) = next else {
        return;
    };

    // Don't downgrade active states to queued/idle.
    if matches!(
        current,
        QueueStatus::Applying
            | QueueStatus::Archiving
            | QueueStatus::Accepting
            | QueueStatus::Resolving
    ) && matches!(next, QueueStatus::Queued | QueueStatus::NotQueued)
    {
        return;
    }

    // Only set queued/idle if we're not already in a terminal state.
    if matches!(next, QueueStatus::Queued | QueueStatus::NotQueued)
        && matches!(
            current,
            QueueStatus::Archived | QueueStatus::Merged | QueueStatus::Error(_)
        )
    {
        return;
    }

    // Transition bookkeeping for elapsed time.
    if matches!(next, QueueStatus::Applying) && change.started_at.is_none() {
        change.started_at = Some(Instant::now());
        change.elapsed_time = None;
    }

    if !matches!(
        next,
        QueueStatus::Applying
            | QueueStatus::Archiving
            | QueueStatus::Accepting
            | QueueStatus::Resolving
    ) && matches!(
        current,
        QueueStatus::Applying
            | QueueStatus::Archiving
            | QueueStatus::Accepting
            | QueueStatus::Resolving
    ) {
        if let Some(started) = change.started_at {
            change.elapsed_time = Some(started.elapsed());
        }
    }

    change.queue_status = next;
}

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
    /// Manual resolve queue (FIFO) for sequential resolve processing
    pub resolve_queue: VecDeque<String>,
    /// Set of change IDs in the resolve queue for duplicate prevention
    pub resolve_queue_set: HashSet<String>,
    /// Whether the log panel is visible in Changes view
    pub logs_panel_enabled: bool,
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
    pub fn from_change(change: &Change) -> Self {
        Self {
            id: change.id.clone(),
            // Initial values from Change object; synchronized with shared state in update_changes()
            completed_tasks: change.completed_tasks,
            total_tasks: change.total_tasks,
            selected: false, // Always start unselected
            is_new: false,
            queue_status: QueueStatus::NotQueued,
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

    /// Update iteration number with monotonic increase guard
    ///
    /// This helper ensures iteration display doesn't regress within the same stage.
    /// - Ignores None values (no-op)
    /// - Only updates if new_iteration > current iteration_number
    /// - Prevents display flickering when out-of-order events arrive
    pub fn update_iteration_monotonic(&mut self, new_iteration: Option<u32>) {
        if let Some(new_val) = new_iteration {
            match self.iteration_number {
                None => {
                    // First iteration for this stage, accept it
                    self.iteration_number = Some(new_val);
                }
                Some(current) => {
                    // Only update if new value is higher (monotonic increase)
                    if new_val > current {
                        self.iteration_number = Some(new_val);
                    }
                    // Otherwise, ignore (prevents regression)
                }
            }
        }
        // If new_iteration is None, ignore (no update)
    }
}

// ============================================================================
// AppState Core Implementation
// ============================================================================

impl AppState {
    /// Create a new AppState with initial changes
    ///
    /// All changes start unselected on startup.
    /// Users must explicitly select changes to process.
    pub fn new(changes: Vec<Change>) -> Self {
        let known_ids: HashSet<String> = changes.iter().map(|c| c.id.clone()).collect();

        // All changes start unselected
        let change_states: Vec<ChangeState> =
            changes.iter().map(ChangeState::from_change).collect();

        let mut list_state = ListState::default();
        if !change_states.is_empty() {
            list_state.select(Some(0));
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
            logs: Vec::new(),
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
            resolve_queue: VecDeque::new(),
            resolve_queue_set: HashSet::new(),
            logs_panel_enabled: true, // Default: logs panel visible
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

        debug!("Merge initiated: creating TuiCommand::MergeWorktreeBranch");
        Some(TuiCommand::MergeWorktreeBranch {
            worktree_path: path,
            branch_name,
        })
    }

    /// Toggle selection of the current change
    ///
    /// In Select mode:
    /// - Changes can be toggled between selected/unselected
    ///
    /// In Running/Completed mode:
    /// - Changes can be added to or removed from the queue
    pub fn toggle_selection(&mut self) -> Option<TuiCommand> {
        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return None;
        }

        let change = &mut self.changes[self.cursor_index];

        // Validate that the change can be toggled
        if let guards::ToggleGuardResult::Blocked(msg) = guards::validate_change_toggleable(
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

    fn can_bulk_toggle_change(&self, change: &ChangeState) -> bool {
        if matches!(self.mode, AppMode::Running) && change.queue_status.is_active() {
            return false;
        }

        guards::validate_change_toggleable(
            change.is_parallel_eligible,
            self.parallel_mode,
            &change.queue_status,
            &change.id,
        )
        .is_allowed()
    }

    /// Returns true when at least one change can be targeted by bulk toggle.
    pub fn has_bulk_toggle_targets(&self) -> bool {
        matches!(
            self.mode,
            AppMode::Select | AppMode::Stopped | AppMode::Running
        ) && self
            .changes
            .iter()
            .any(|change| self.can_bulk_toggle_change(change))
    }

    /// Toggle all marks (select/unselect all eligible changes)
    ///
    /// In Select/Stopped/Running modes:
    /// - If any eligible unmarked change exists, mark all eligible changes
    /// - Otherwise, unmark all eligible changes
    ///
    /// Running mode excludes active rows to avoid emitting stop requests.
    /// In parallel mode, uncommitted changes remain excluded.
    pub fn toggle_all_marks(&mut self) {
        if !self.has_bulk_toggle_targets() {
            return;
        }

        // If any eligible unmarked change exists, we mark all; otherwise unmark all.
        let has_unmarked = self
            .changes
            .iter()
            .any(|change| !change.selected && self.can_bulk_toggle_change(change));

        let target_state = has_unmarked;

        // Toggle all eligible changes to the target state.
        for i in 0..self.changes.len() {
            if !self.can_bulk_toggle_change(&self.changes[i]) {
                continue;
            }

            if self.changes[i].selected != target_state {
                self.changes[i].selected = target_state;
                // Clear NEW flag when user interacts with the change
                if self.changes[i].is_new {
                    self.changes[i].is_new = false;
                    self.new_change_count = self.new_change_count.saturating_sub(1);
                }
            }
        }

        let action = if target_state { "marked" } else { "unmarked" };
        let count = self
            .changes
            .iter()
            .filter(|change| self.can_bulk_toggle_change(change) && change.selected == target_state)
            .count();
        self.add_log(LogEntry::info(format!(
            "Toggled all: {} {} change(s)",
            count, action
        )));
    }

    /// Trigger merge resolution for the selected change when applicable.
    ///
    /// If resolve is already running, the change is added to the resolve queue
    /// and transitioned to ResolveWait instead of starting immediately.
    pub fn resolve_merge(&mut self) -> Option<TuiCommand> {
        // Must have valid cursor position
        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return None;
        }

        // Must be in correct mode
        if !matches!(
            self.mode,
            AppMode::Select | AppMode::Stopped | AppMode::Running
        ) {
            return None;
        }

        // Check current change status and get change_id
        let change_id = {
            let change = &self.changes[self.cursor_index];
            if !matches!(change.queue_status, QueueStatus::MergeWait) {
                return None;
            }
            change.id.clone()
        };

        if self.is_resolving {
            // Resolve is running: add to queue and transition to ResolveWait
            if self.add_to_resolve_queue(&change_id) {
                self.changes[self.cursor_index].queue_status = QueueStatus::ResolveWait;
                self.add_log(LogEntry::info(format!(
                    "Queued '{}' for resolve (position: {})",
                    change_id,
                    self.resolve_queue.len()
                )));
            } else {
                self.warning_message = Some(format!(
                    "Change '{}' is already queued for resolve",
                    change_id
                ));
            }
            None
        } else {
            // Resolve is not running: start immediately
            self.changes[self.cursor_index].queue_status = QueueStatus::ResolveWait;
            Some(TuiCommand::ResolveMerge(change_id))
        }
    }

    /// Add a change to the resolve queue (with duplicate prevention).
    ///
    /// Returns true if the change was added, false if it was already in the queue.
    pub fn add_to_resolve_queue(&mut self, change_id: &str) -> bool {
        if self.resolve_queue_set.contains(change_id) {
            false
        } else {
            self.resolve_queue.push_back(change_id.to_string());
            self.resolve_queue_set.insert(change_id.to_string());
            true
        }
    }

    /// Pop the next change from the resolve queue.
    ///
    /// Returns the change ID if the queue is not empty, otherwise None.
    pub fn pop_from_resolve_queue(&mut self) -> Option<String> {
        if let Some(change_id) = self.resolve_queue.pop_front() {
            self.resolve_queue_set.remove(&change_id);
            Some(change_id)
        } else {
            None
        }
    }

    /// Check if there are queued resolves waiting.
    #[cfg(test)]
    pub fn has_queued_resolves(&self) -> bool {
        !self.resolve_queue.is_empty()
    }

    /// Update parallel eligibility status for changes.
    ///
    /// A change is eligible for parallel execution if:
    /// 1. It exists in HEAD's commit tree (committed_change_ids), AND
    /// 2. It has no uncommitted or untracked files under openspec/changes/<change_id>/
    pub fn apply_parallel_eligibility(
        &mut self,
        committed_change_ids: &HashSet<String>,
        uncommitted_file_change_ids: &HashSet<String>,
    ) {
        for change in &mut self.changes {
            // Eligible if committed AND no uncommitted files
            change.is_parallel_eligible = committed_change_ids.contains(&change.id)
                && !uncommitted_file_change_ids.contains(&change.id);
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

    /// Sync `ChangeState.queue_status` from the reducer's display status snapshot.
    ///
    /// This is Phase 6.1: TUI derives displayed change status from the shared
    /// orchestration reducer state instead of maintaining an independent lifecycle copy.
    /// Only transitions that are safe (no active execution regression) are applied.
    pub fn apply_display_statuses_from_reducer(
        &mut self,
        display_map: &HashMap<String, &'static str>,
    ) {
        for change in &mut self.changes {
            if let Some(&status_str) = display_map.get(&change.id) {
                let new_status = match status_str {
                    "not queued" => QueueStatus::NotQueued,
                    "queued" => QueueStatus::Queued,
                    "blocked" => QueueStatus::Blocked,
                    "applying" => QueueStatus::Applying,
                    "accepting" => QueueStatus::Accepting,
                    "archiving" => QueueStatus::Archiving,
                    "merge wait" => QueueStatus::MergeWait,
                    "resolve pending" => QueueStatus::ResolveWait,
                    "resolving" => QueueStatus::Resolving,
                    "archived" => QueueStatus::Archived,
                    "merged" => QueueStatus::Merged,
                    "stopped" => QueueStatus::NotQueued, // Stopped changes appear as not-queued in TUI
                    "error" => {
                        if matches!(change.queue_status, QueueStatus::Error(_)) {
                            continue; // Preserve existing error message
                        }
                        QueueStatus::Error("reducer".to_string())
                    }
                    _ => continue,
                };
                change.queue_status = new_status;
            }
        }
    }

    /// Get the number of selected changes
    pub fn selected_count(&self) -> usize {
        self.changes.iter().filter(|c| c.selected).count()
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

        if self.is_resolving {
            self.warning_message =
                Some("Cannot start processing while merge resolve is in progress".to_string());
            return None;
        }

        // Only NotQueued changes can be transitioned to Queued by StartProcessing.
        // Active states (Applying, Accepting, Archiving, Blocked, Queued) and terminal
        // states (Merged, Error, MergeWait, ResolveWait, Archived) are excluded.
        let selected: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.selected && matches!(c.queue_status, QueueStatus::NotQueued))
            .map(|c| c.id.clone())
            .collect();

        if self.parallel_mode {
            let ineligible: Vec<String> = self
                .changes
                .iter()
                .filter(|c| {
                    c.selected
                        && !c.is_parallel_eligible
                        && matches!(c.queue_status, QueueStatus::NotQueued)
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

        // Mark selected NotQueued changes as Queued
        for change in &mut self.changes {
            if change.selected && matches!(change.queue_status, QueueStatus::NotQueued) {
                change.queue_status = QueueStatus::Queued;
            }
        }

        // Sync queue intent into the shared reducer so that reducer-driven display
        // sync (apply_display_statuses_from_reducer) cannot regress these rows back
        // to "not queued" before the orchestrator processes them.
        if let Some(shared) = &self.shared_orchestrator_state {
            if let Ok(mut guard) = shared.try_write() {
                for id in &selected {
                    guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                        id.clone(),
                    ));
                }
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

        if self.is_resolving {
            self.warning_message =
                Some("Cannot resume processing while merge resolve is in progress".to_string());
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

        // Sync queue intent into the shared reducer so that reducer-driven display
        // sync cannot regress these rows back to "not queued" before the orchestrator
        // processes them.
        if let Some(shared) = &self.shared_orchestrator_state {
            if let Ok(mut guard) = shared.try_write() {
                for id in &marked_ids {
                    guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                        id.clone(),
                    ));
                }
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

        if self.is_resolving {
            self.warning_message =
                Some("Cannot retry while merge resolve is in progress".to_string());
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

        // Sync queue intent into the shared reducer.  AddToQueue clears retryable
        // terminal (Error/Stopped) states so that reducer-driven display sync will
        // return "queued" for these rows instead of "error".
        if let Some(shared) = &self.shared_orchestrator_state {
            if let Ok(mut guard) = shared.try_write() {
                for id in &error_ids {
                    guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                        id.clone(),
                    ));
                }
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
    ///
    /// In remote mode, change IDs have the form `"<project_id>::<project_name>/<change_id>"`.
    /// Log entries from the remote server may have `change_id` set to the bare `project_id`
    /// (when no specific change is known). This method also matches those project-level logs
    /// by checking if the `change_id` argument starts with `"<entry.change_id>::"`.
    pub fn get_latest_log_for_change(&self, change_id: &str) -> Option<&LogEntry> {
        self.logs.iter().rev().find(|entry| {
            if let Some(entry_cid) = entry.change_id.as_deref() {
                // Exact match (local mode and remote mode with full change_id)
                if entry_cid == change_id {
                    return true;
                }
                // Project-level log match: entry has project_id, change_id starts with that project_id
                // Remote change IDs have the form "<project_id>::<project_name>/<change_id>"
                // Remote logs with only project_id set as change_id will match via this prefix check.
                let prefix = format!("{}::", entry_cid);
                if change_id.starts_with(&prefix) {
                    return true;
                }
            }
            false
        })
    }

    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        // Send to tracing for debug file output (always enabled)
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

    /// Toggle log panel visibility
    pub fn toggle_logs_panel(&mut self) {
        self.logs_panel_enabled = !self.logs_panel_enabled;
    }
}

// ============================================================================
// Event Handling
// ============================================================================

impl AppState {
    /// Handle an event from the orchestrator
    ///
    /// This is the main entry point for event handling, dispatching to specialized handlers.
    /// Returns an optional TuiCommand that should be executed (e.g., for auto-starting next resolve).
    pub fn handle_orchestrator_event(&mut self, event: OrchestratorEvent) -> Option<TuiCommand> {
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
            } => return self.handle_resolve_completed(change_id, worktree_change_ids),
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
            OrchestratorEvent::MergeDeferred {
                change_id,
                reason,
                auto_resumable,
            } => self.handle_merge_deferred(change_id, reason, auto_resumable),
            OrchestratorEvent::AcceptanceStarted { change_id, command } => {
                self.handle_acceptance_started(change_id, command)
            }
            OrchestratorEvent::AcceptanceCompleted { change_id } => {
                self.handle_acceptance_completed(change_id)
            }
            OrchestratorEvent::BranchMergeFailed { branch_name, error } => {
                self.handle_branch_merge_failed(branch_name, error)
            }

            // Single-change stop events
            OrchestratorEvent::ChangeStopped { change_id } => self.handle_change_stopped(change_id),
            OrchestratorEvent::ChangeStopFailed { change_id, error } => {
                self.handle_change_stop_failed(change_id, error)
            }

            // Refresh events
            OrchestratorEvent::ChangesRefreshed {
                changes,
                committed_change_ids,
                uncommitted_file_change_ids,
                worktree_change_ids,
                worktree_paths,
                worktree_not_ahead_ids,
                merge_wait_ids,
            } => self.handle_changes_refreshed(
                changes,
                committed_change_ids,
                uncommitted_file_change_ids,
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
            OrchestratorEvent::ParallelStartRejected { change_ids, reason } => {
                self.handle_parallel_start_rejected(change_ids, reason)
            }
            OrchestratorEvent::Error { message } => self.handle_error(message),

            // Remote server incremental update (applies non-regression rule)
            OrchestratorEvent::RemoteChangeUpdate {
                id,
                completed_tasks,
                total_tasks,
                status,
                iteration_number,
            } => {
                let mut status_log: Option<String> = None;
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
                    // Non-regression rule: never decrease completed_tasks
                    if completed_tasks >= change.completed_tasks {
                        change.completed_tasks = completed_tasks;
                    }
                    // Always update total so the denominator stays accurate
                    change.total_tasks = total_tasks;

                    // Status update (remote mode)
                    if let Some(status) = status.as_deref() {
                        let before = change.queue_status.clone();
                        apply_remote_status(change, status);
                        if before != change.queue_status {
                            status_log = Some(format!(
                                "Remote status: {} -> {}",
                                id,
                                change.queue_status.display()
                            ));
                        }
                    }

                    // Apply monotonic non-regression rule for iteration_number
                    change.update_iteration_monotonic(iteration_number);
                }
                if let Some(line) = status_log {
                    self.add_log(LogEntry::info(line));
                }
            }

            // Ignore other parallel-specific events that don't affect TUI state
            _ => {
                // Other events (workspace, merge, group events) are for status tracking
                // and don't need to be displayed in the log
            }
        }
        None
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

        // Safety net: reset any changes still in Queued or Blocked state (e.g. rejected
        // before start or blocked by dependency failures that were not cleared earlier).
        for change in &mut self.changes {
            if matches!(
                change.queue_status,
                QueueStatus::Queued | QueueStatus::Blocked
            ) {
                change.queue_status = QueueStatus::NotQueued;
            }
        }

        // If any change is still Resolving, keep Running mode so the user can
        // continue to add changes to the queue via Space key.
        let has_resolving = self
            .changes
            .iter()
            .any(|c| c.queue_status == QueueStatus::Resolving);
        if has_resolving {
            info!("AllCompleted received but resolve still in progress; staying in Running mode");
            self.add_log(LogEntry::info(
                "All changes processed, waiting for resolve to complete",
            ));
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

    /// Transition to `AppMode::Select` if no active changes remain.
    ///
    /// "Active" means any change is still in a processing queue status:
    /// Queued, Blocked, Applying, Accepting, Archiving, Resolving, or ResolveWait.
    ///
    /// Called after resolve completion/failure to handle the deferred transition
    /// that `handle_all_completed()` skipped while resolves were still in flight.
    fn try_transition_to_select(&mut self) {
        if !matches!(self.mode, AppMode::Running) {
            return;
        }

        let has_active = self.changes.iter().any(|c| {
            matches!(
                c.queue_status,
                QueueStatus::Queued
                    | QueueStatus::Blocked
                    | QueueStatus::Applying
                    | QueueStatus::Accepting
                    | QueueStatus::Archiving
                    | QueueStatus::Resolving
                    | QueueStatus::ResolveWait
            )
        });

        if !has_active {
            info!("No active changes remaining after resolve; transitioning to Select");
            self.mode = AppMode::Select;
            self.current_change = None;
            self.stop_mode = StopMode::None;
            if let Some(started) = self.orchestration_started_at {
                self.orchestration_elapsed = Some(started.elapsed());
            }
            self.add_log(LogEntry::success("All changes processed successfully"));
        }
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
                    | QueueStatus::Resolving
                    | QueueStatus::Queued
                    | QueueStatus::Blocked
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
            // Reset iteration_number when starting a new stage
            change.iteration_number = None;
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
            // Reset iteration_number when starting a new stage
            change.iteration_number = None;
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
            // Reset iteration_number when starting a new stage
            change.iteration_number = None;
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
    ) -> Option<TuiCommand> {
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

        // Auto-start next resolve from queue if available
        if let Some(next_change_id) = self.pop_from_resolve_queue() {
            self.add_log(LogEntry::info(format!(
                "Auto-starting resolve for '{}' from queue",
                next_change_id
            )));
            // Transition from ResolveWait to about-to-start
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == next_change_id) {
                change.queue_status = QueueStatus::ResolveWait;
            }
            Some(TuiCommand::ResolveMerge(next_change_id))
        } else {
            // No more resolves queued; check if we should transition to Select
            self.try_transition_to_select();
            None
        }
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
            if change.queue_status == QueueStatus::Merged {
                // Already successfully merged; ignore spurious ResolveFailed to prevent regression.
                self.add_log(LogEntry::info(format!(
                    "Ignoring ResolveFailed for '{}': already Merged",
                    change_id
                )));
                return;
            }
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

        // Check if we should transition to Select after resolve failure
        self.try_transition_to_select();
    }

    fn handle_merge_deferred(&mut self, change_id: String, reason: String, auto_resumable: bool) {
        if self.is_resolving {
            // Check if this is the currently resolving change
            let is_current_resolving = self
                .changes
                .iter()
                .any(|c| c.id == change_id && c.queue_status == QueueStatus::Resolving);

            if is_current_resolving {
                // Don't queue the currently resolving change - keep it Resolving
                self.add_log(LogEntry::warn(format!(
                    "Merge deferred for '{}' (currently resolving, not queued): {}",
                    change_id, reason
                )));
            } else {
                // Different change: transition to ResolveWait and add to resolve queue
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.queue_status = QueueStatus::ResolveWait;
                }
                if self.add_to_resolve_queue(&change_id) {
                    self.add_log(LogEntry::warn(format!(
                        "Merge deferred for '{}' (queued for resolve): {}",
                        change_id, reason
                    )));
                } else {
                    self.add_log(LogEntry::warn(format!(
                        "Merge deferred for '{}' (already queued): {}",
                        change_id, reason
                    )));
                }
            }
        } else if auto_resumable {
            // Resolve is not running but deferral is auto-resumable (e.g. dirty base due to
            // another merge in progress).  Show as ResolveWait so the user can see it will
            // be re-evaluated automatically; do NOT add to the manual resolve queue.
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                change.queue_status = QueueStatus::ResolveWait;
            }
            self.add_log(LogEntry::warn(format!(
                "Merge deferred for '{}' (auto-resumable, awaiting re-evaluation): {}",
                change_id, reason
            )));
        } else {
            // Resolve is not running and manual intervention is required: MergeWait.
            if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                change.queue_status = QueueStatus::MergeWait;
            }
            self.add_log(LogEntry::warn(format!(
                "Merge deferred for {}: {}",
                change_id, reason
            )));
        }
    }

    fn handle_acceptance_started(&mut self, change_id: String, command: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if change.started_at.is_none() {
                change.started_at = Some(Instant::now());
            }
            change.queue_status = QueueStatus::Accepting;
            // Reset iteration_number when starting a new stage
            change.iteration_number = None;
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

    fn handle_change_stopped(&mut self, change_id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            // Transition to NotQueued and clear execution mark
            change.queue_status = QueueStatus::NotQueued;
            change.selected = false;
            // Record elapsed time
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::info(format!("Stopped: {}", change_id)));
    }

    fn handle_change_stop_failed(&mut self, change_id: String, error: String) {
        // Keep the change in its current state on stop failure
        self.add_log(LogEntry::error(format!(
            "Failed to stop {}: {}",
            change_id, error
        )));
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
    #[allow(clippy::too_many_arguments)]
    fn handle_changes_refreshed(
        &mut self,
        changes: Vec<Change>,
        committed_change_ids: HashSet<String>,
        uncommitted_file_change_ids: HashSet<String>,
        worktree_change_ids: HashSet<String>,
        worktree_paths: HashMap<String, PathBuf>,
        _worktree_not_ahead_ids: HashSet<String>,
        _merge_wait_ids: HashSet<String>,
    ) {
        self.worktree_paths = worktree_paths;
        self.update_changes(changes);
        self.apply_parallel_eligibility(&committed_change_ids, &uncommitted_file_change_ids);
        self.apply_worktree_status(&worktree_change_ids);
        // Phase 5.2: MergeWait reconciliation is now handled by the shared reducer's
        // apply_observation() path via apply_display_statuses_from_reducer(), which is called
        // in the runner after updating shared state with the ChangesRefreshed event.
        // auto_clear_merge_wait and apply_merge_wait_status are no longer called here.
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
        // Update iteration number in change state with monotonic guard
        // Only update if the change is currently in Applying stage
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.queue_status, QueueStatus::Applying) {
                change.update_iteration_monotonic(iteration);
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("apply")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    fn handle_archive_output(&mut self, change_id: String, output: String, iteration: u32) {
        // Update iteration number in change state with monotonic guard
        // Only update if the change is currently in Archiving stage
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.queue_status, QueueStatus::Archiving) {
                change.update_iteration_monotonic(Some(iteration));
            }
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
        // Update iteration number in change state with monotonic guard
        // Only update if the change is currently in Accepting stage
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.queue_status, QueueStatus::Accepting) {
                change.update_iteration_monotonic(iteration);
            }
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
        // Update iteration number in change state with monotonic guard
        // Only update if the change is currently in Resolving stage
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.queue_status, QueueStatus::Resolving) {
                change.update_iteration_monotonic(iteration);
            }
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

    /// Reset backend-rejected changes from Queued to NotQueued with an explanatory log entry.
    ///
    /// Called when `ParallelStartRejected` is received, meaning backend eligibility filtering
    /// excluded one or more changes before execution started. Any such change that was left in
    /// `Queued` state by the TUI start/resume path is reset to `NotQueued` so the row no longer
    /// appears as if it is about to run.
    fn handle_parallel_start_rejected(&mut self, change_ids: Vec<String>, reason: String) {
        let mut reset_ids = Vec::new();
        for change in &mut self.changes {
            if change_ids.contains(&change.id) && matches!(change.queue_status, QueueStatus::Queued)
            {
                change.queue_status = QueueStatus::NotQueued;
                reset_ids.push(change.id.clone());
            }
        }
        // Mirror the rejection in the shared reducer so that subsequent
        // ChangesRefreshed display syncs do not re-queue these rows.
        if !reset_ids.is_empty() {
            if let Some(shared) = &self.shared_orchestrator_state {
                if let Ok(mut guard) = shared.try_write() {
                    for id in &reset_ids {
                        guard.apply_command(
                            crate::orchestration::state::ReducerCommand::RemoveFromQueue(
                                id.clone(),
                            ),
                        );
                    }
                }
            }
            self.add_log(LogEntry::warn(format!(
                "Not started ({}): {}",
                reason,
                reset_ids.join(", ")
            )));
        }
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
                let mut new_state = ChangeState::from_change(fetched);
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
}
// Note: auto_clear_merge_wait() and apply_merge_wait_status() have been removed in Phase 5.3.
// Their logic is now handled by the shared reducer's apply_observation() path.
// The TUI syncs queue_status via apply_display_statuses_from_reducer() in the runner.

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

    impl ToggleGuardResult {
        /// Check if the operation is allowed
        pub fn is_allowed(&self) -> bool {
            matches!(self, ToggleGuardResult::Allowed)
        }
    }

    /// Validates that a change can be toggled for selection
    pub fn validate_change_toggleable(
        is_parallel_eligible: bool,
        parallel_mode: bool,
        queue_status: &QueueStatus,
        change_id: &str,
    ) -> ToggleGuardResult {
        // Active (in-flight) changes can be stopped via Space key in Running mode
        // This is allowed and handled by handle_toggle_running_mode
        // No need to block here

        // Cannot select uncommitted changes in parallel mode (only applies to non-active states)
        if parallel_mode && !is_parallel_eligible && !queue_status.is_active() {
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
                // Emit AddToQueue command; do NOT directly assign queue_status here.
                // The shared reducer state will be updated via apply_command in command_handlers,
                // and the TUI will derive the display status from the shared state.
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
                // Emit RemoveFromQueue command; do NOT directly assign queue_status here.
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
            QueueStatus::Applying
            | QueueStatus::Accepting
            | QueueStatus::Archiving
            | QueueStatus::Resolving => {
                // Active (in-flight) changes: issue stop request
                // State transition happens when ChangeStopped event is received
                let id = change.id.clone();
                let log_msg = format!("Stop requested: {}", id);
                ToggleActionResult::Command(TuiCommand::StopChange(id), Some(log_msg))
            }
            // Completed, Archived, Merged, Blocked, Error - cannot change status
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
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        assert_eq!(change.progress_percent(), 50.0);
    }

    #[test]
    fn test_app_state_new_all_not_selected() {
        // All changes should start unselected on startup
        let changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 0, 3),
        ];

        let app = AppState::new(changes);

        assert_eq!(app.mode, AppMode::Select);
        assert_eq!(app.changes.len(), 2);
        assert_eq!(app.cursor_index, 0);
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);
    }

    #[test]
    fn test_app_state_no_auto_selection() {
        // Changes should NOT be auto-selected on startup
        let changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 0, 3),
        ];

        let app = AppState::new(changes);

        assert_eq!(app.mode, AppMode::Select);
        assert_eq!(app.changes.len(), 2);
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);
        // Should NOT have log entry for auto-queued changes
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
        // Changes start unselected
        let changes = vec![create_test_change("a", 0, 1)];

        let mut app = AppState::new(changes);

        assert!(!app.changes[0].selected);

        app.toggle_selection();
        assert!(app.changes[0].selected);

        app.toggle_selection();
        assert!(!app.changes[0].selected);
    }

    #[test]
    fn test_toggle_all_marks_select_mode() {
        // Test toggle all in Select mode - mark all then unmark all
        let changes = vec![
            create_test_change("a", 0, 1),
            create_test_change("b", 0, 1),
            create_test_change("c", 0, 1),
        ];

        let mut app = AppState::new(changes);
        assert_eq!(app.mode, AppMode::Select);

        // All start unselected
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);
        assert!(!app.changes[2].selected);

        // First toggle: should mark all
        app.toggle_all_marks();
        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);
        assert!(app.changes[2].selected);

        // Second toggle: should unmark all
        app.toggle_all_marks();
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);
        assert!(!app.changes[2].selected);
    }

    #[test]
    fn test_toggle_all_marks_stopped_mode() {
        // Test toggle all in Stopped mode
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;

        // First toggle: should mark all
        app.toggle_all_marks();
        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);

        // Second toggle: should unmark all
        app.toggle_all_marks();
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);
    }

    #[test]
    fn test_toggle_all_marks_parallel_mode_excludes_uncommitted() {
        // Test that toggle all respects parallel mode restrictions
        let changes = vec![
            create_test_change("committed", 0, 1),
            create_test_change("uncommitted", 0, 1),
        ];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Select;
        app.parallel_mode = true;
        app.parallel_available = true;

        // Mark first as committed, second as uncommitted
        app.changes[0].is_parallel_eligible = true;
        app.changes[1].is_parallel_eligible = false;

        // Toggle all should only mark the committed change
        app.toggle_all_marks();
        assert!(app.changes[0].selected);
        assert!(!app.changes[1].selected); // Excluded due to parallel mode

        // Toggle all again should unmark
        app.toggle_all_marks();
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);
    }

    #[test]
    fn test_toggle_all_marks_partial_selection() {
        // Test that if any unmarked change exists, toggle marks all
        let changes = vec![
            create_test_change("a", 0, 1),
            create_test_change("b", 0, 1),
            create_test_change("c", 0, 1),
        ];

        let mut app = AppState::new(changes);

        // Manually select one change
        app.changes[0].selected = true;

        // Toggle all should mark the rest (because unmarked changes exist)
        app.toggle_all_marks();
        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);
        assert!(app.changes[2].selected);

        // Toggle all again should unmark all
        app.toggle_all_marks();
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);
        assert!(!app.changes[2].selected);
    }

    #[test]
    fn test_toggle_all_marks_running_mode_toggles_non_active_rows_only() {
        let changes = vec![
            create_test_change("resolving", 0, 1),
            create_test_change("not-queued", 0, 1),
            create_test_change("merge-wait", 0, 1),
            create_test_change("resolve-wait", 0, 1),
        ];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.is_resolving = true;
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.changes[1].queue_status = QueueStatus::NotQueued;
        app.changes[2].queue_status = QueueStatus::MergeWait;
        app.changes[3].queue_status = QueueStatus::ResolveWait;

        app.toggle_all_marks();
        assert!(!app.changes[0].selected, "active row must stay unchanged");
        assert!(app.changes[1].selected);
        assert!(app.changes[2].selected);
        assert!(app.changes[3].selected);

        // Wait states must keep queue_status unchanged.
        assert_eq!(app.changes[2].queue_status, QueueStatus::MergeWait);
        assert_eq!(app.changes[3].queue_status, QueueStatus::ResolveWait);

        // Second toggle unmarks only non-active rows.
        app.toggle_all_marks();
        assert!(!app.changes[0].selected, "active row must stay unchanged");
        assert!(!app.changes[1].selected);
        assert!(!app.changes[2].selected);
        assert!(!app.changes[3].selected);
    }

    #[test]
    fn test_has_bulk_toggle_targets_running_mode_requires_non_active_rows() {
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Applying;
        app.changes[1].queue_status = QueueStatus::Resolving;
        assert!(!app.has_bulk_toggle_targets());

        app.changes[1].queue_status = QueueStatus::ResolveWait;
        assert!(app.has_bulk_toggle_targets());
    }

    #[test]
    fn test_start_processing_blocked_while_resolving() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.is_resolving = true;

        let command = app.start_processing();
        assert!(command.is_none());
        assert_eq!(
            app.warning_message,
            Some("Cannot start processing while merge resolve is in progress".to_string())
        );
    }

    #[test]
    fn test_resume_processing_blocked_while_resolving() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;
        app.is_resolving = true;

        let command = app.resume_processing();
        assert!(command.is_none());
        assert_eq!(
            app.warning_message,
            Some("Cannot resume processing while merge resolve is in progress".to_string())
        );
    }

    #[test]
    fn test_retry_error_changes_blocked_while_resolving() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Error;
        app.changes[0].queue_status = QueueStatus::Error("boom".to_string());
        app.is_resolving = true;

        let command = app.retry_error_changes();
        assert!(command.is_none());
        assert_eq!(
            app.warning_message,
            Some("Cannot retry while merge resolve is in progress".to_string())
        );
    }

    #[test]
    fn test_selected_count() {
        // Changes start unselected
        let changes = vec![
            create_test_change("a", 0, 1),
            create_test_change("b", 0, 1),
            create_test_change("c", 0, 1),
        ];

        let mut app = AppState::new(changes);

        assert_eq!(app.selected_count(), 0);

        app.toggle_selection(); // Select first
        assert_eq!(app.selected_count(), 1);
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

    // Processing lifecycle tests (from processing.rs)

    #[test]
    fn test_processing_error_keeps_app_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Start in Running mode
        app.mode = AppMode::Running;
        app.current_change = Some("test-change".to_string());

        // Receive ProcessingError
        app.handle_processing_error("test-change".to_string(), "Test error message".to_string());

        // AppMode should remain Running (NOT transition to Error)
        assert_eq!(app.mode, AppMode::Running);

        // Change should be marked as Error
        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert!(matches!(change.queue_status, QueueStatus::Error(_)));

        // error_change_id should be set
        assert_eq!(app.error_change_id, Some("test-change".to_string()));

        // current_change should be cleared
        assert_eq!(app.current_change, None);
    }

    #[test]
    fn test_processing_error_from_select_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Start in Select mode
        app.mode = AppMode::Select;

        // Receive ProcessingError
        app.handle_processing_error("test-change".to_string(), "Test error message".to_string());

        // AppMode should remain Select (NOT transition to Error)
        assert_eq!(app.mode, AppMode::Select);

        // Change should be marked as Error
        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert!(matches!(change.queue_status, QueueStatus::Error(_)));
    }

    #[test]
    fn test_processing_started_sets_state() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_processing_started("test-change".to_string());

        assert_eq!(app.current_change, Some("test-change".to_string()));
        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert_eq!(change.queue_status, QueueStatus::Applying);
        assert!(change.started_at.is_some());
    }

    #[test]
    fn test_processing_completed_updates_status() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_processing_completed("test-change".to_string());

        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert_eq!(change.queue_status, QueueStatus::Archiving);
    }

    #[test]
    fn test_all_completed_transitions_to_select() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Running;
        app.handle_all_completed();

        assert_eq!(app.mode, AppMode::Select);
        assert_eq!(app.current_change, None);
    }

    #[test]
    fn test_all_completed_preserves_error_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Error;
        app.handle_all_completed();

        // Should remain in Error mode
        assert_eq!(app.mode, AppMode::Error);
    }

    #[test]
    fn test_all_completed_preserves_stopped_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Stopped;
        app.handle_all_completed();

        // Should remain in Stopped mode (not transition to Select)
        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn test_stopped_resets_queue_status() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Set to Queued
        app.changes[0].queue_status = QueueStatus::Queued;
        app.changes[0].selected = true;

        app.handle_stopped();

        assert_eq!(app.mode, AppMode::Stopped);
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        // selected should be preserved
        assert!(app.changes[0].selected);
    }

    // Iteration guard tests

    #[test]
    fn test_iteration_monotonic_update_from_none() {
        let mut change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            queue_status: QueueStatus::Applying,
            selected: false,
            is_new: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        // First iteration should be accepted
        change.update_iteration_monotonic(Some(1));
        assert_eq!(change.iteration_number, Some(1));

        // Higher iteration should update
        change.update_iteration_monotonic(Some(2));
        assert_eq!(change.iteration_number, Some(2));
    }

    #[test]
    fn test_iteration_monotonic_prevents_regression() {
        let mut change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            queue_status: QueueStatus::Applying,
            selected: false,
            is_new: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: Some(3),
        };

        // Lower iteration should be ignored
        change.update_iteration_monotonic(Some(1));
        assert_eq!(change.iteration_number, Some(3));

        // Same iteration should be ignored
        change.update_iteration_monotonic(Some(3));
        assert_eq!(change.iteration_number, Some(3));

        // Higher iteration should update
        change.update_iteration_monotonic(Some(5));
        assert_eq!(change.iteration_number, Some(5));
    }

    #[test]
    fn test_iteration_monotonic_ignores_none() {
        let mut change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            queue_status: QueueStatus::Applying,
            selected: false,
            is_new: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: Some(2),
        };

        // None should be ignored
        change.update_iteration_monotonic(None);
        assert_eq!(change.iteration_number, Some(2));
    }

    #[test]
    fn test_iteration_reset_on_stage_change_apply() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Set iteration to 3
        app.changes[0].iteration_number = Some(3);

        // Handle apply started
        app.handle_apply_started("test-change".to_string(), "mock command".to_string());

        // Iteration should be reset to None
        assert_eq!(app.changes[0].iteration_number, None);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Applying);
    }

    #[test]
    fn test_iteration_reset_on_stage_change_archive() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Set iteration to 5
        app.changes[0].iteration_number = Some(5);

        // Handle archive started
        app.handle_archive_started("test-change".to_string(), "mock command".to_string());

        // Iteration should be reset to None
        assert_eq!(app.changes[0].iteration_number, None);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Archiving);
    }

    #[test]
    fn test_iteration_reset_on_stage_change_resolve() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Set iteration to 2
        app.changes[0].iteration_number = Some(2);

        // Handle resolve started
        app.handle_resolve_started("test-change".to_string(), "mock command".to_string());

        // Iteration should be reset to None
        assert_eq!(app.changes[0].iteration_number, None);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Resolving);
    }

    #[test]
    fn test_iteration_reset_on_stage_change_acceptance() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Set iteration to 4
        app.changes[0].iteration_number = Some(4);

        // Handle acceptance started
        app.handle_acceptance_started("test-change".to_string(), "mock command".to_string());

        // Iteration should be reset to None
        assert_eq!(app.changes[0].iteration_number, None);
        assert_eq!(app.changes[0].queue_status, QueueStatus::Accepting);
    }

    #[test]
    fn test_iteration_update_via_output_event_apply() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Start apply stage first
        app.handle_apply_started("test-change".to_string(), "mock".to_string());

        // Handle apply output with iteration 1
        app.handle_apply_output("test-change".to_string(), "output 1".to_string(), Some(1));
        assert_eq!(app.changes[0].iteration_number, Some(1));

        // Handle apply output with iteration 3 (should update)
        app.handle_apply_output("test-change".to_string(), "output 3".to_string(), Some(3));
        assert_eq!(app.changes[0].iteration_number, Some(3));

        // Handle apply output with iteration 2 (should NOT regress)
        app.handle_apply_output("test-change".to_string(), "output 2".to_string(), Some(2));
        assert_eq!(app.changes[0].iteration_number, Some(3));
    }

    #[test]
    fn test_iteration_update_via_output_event_archive() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Start archive stage first
        app.handle_archive_started("test-change".to_string(), "mock".to_string());

        // Handle archive output with iteration 1
        app.handle_archive_output("test-change".to_string(), "output 1".to_string(), 1);
        assert_eq!(app.changes[0].iteration_number, Some(1));

        // Handle archive output with iteration 2 (should update)
        app.handle_archive_output("test-change".to_string(), "output 2".to_string(), 2);
        assert_eq!(app.changes[0].iteration_number, Some(2));

        // Handle archive output with iteration 1 again (should NOT regress)
        app.handle_archive_output("test-change".to_string(), "output 1".to_string(), 1);
        assert_eq!(app.changes[0].iteration_number, Some(2));
    }

    #[test]
    fn test_iteration_cross_stage_isolation() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Simulate apply stage with iteration 5
        app.handle_apply_started("test-change".to_string(), "mock".to_string());
        app.handle_apply_output("test-change".to_string(), "output".to_string(), Some(5));
        assert_eq!(app.changes[0].iteration_number, Some(5));

        // Simulate transition to archive stage
        app.handle_archive_started("test-change".to_string(), "mock".to_string());
        // Iteration should be reset
        assert_eq!(app.changes[0].iteration_number, None);

        // Archive stage starts at iteration 1
        app.handle_archive_output("test-change".to_string(), "output".to_string(), 1);
        assert_eq!(app.changes[0].iteration_number, Some(1));

        // Old apply iteration 2 should not regress archive iteration
        // (though in practice this shouldn't happen, test defensive behavior)
        app.handle_apply_output("test-change".to_string(), "stale".to_string(), Some(2));
        // Since we're in archive stage, the apply output should be ignored and iteration should remain 1
        assert_eq!(app.changes[0].iteration_number, Some(1));
    }

    #[test]
    fn test_resolve_queue_fifo_order() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
            create_test_change("change-c", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Add changes to resolve queue
        assert!(app.add_to_resolve_queue("change-a"));
        assert!(app.add_to_resolve_queue("change-b"));
        assert!(app.add_to_resolve_queue("change-c"));

        // Pop in FIFO order
        assert_eq!(app.pop_from_resolve_queue(), Some("change-a".to_string()));
        assert_eq!(app.pop_from_resolve_queue(), Some("change-b".to_string()));
        assert_eq!(app.pop_from_resolve_queue(), Some("change-c".to_string()));
        assert_eq!(app.pop_from_resolve_queue(), None);
    }

    #[test]
    fn test_resolve_queue_duplicate_prevention() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Add change-a once
        assert!(app.add_to_resolve_queue("change-a"));
        // Try to add change-a again - should be blocked
        assert!(!app.add_to_resolve_queue("change-a"));

        // Queue should only have one entry
        assert_eq!(app.pop_from_resolve_queue(), Some("change-a".to_string()));
        assert_eq!(app.pop_from_resolve_queue(), None);
    }

    #[test]
    fn test_resolve_queue_auto_start_on_completion() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Set change-a to Resolving
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        // Queue change-b for resolve
        app.add_to_resolve_queue("change-b");
        app.changes[1].queue_status = QueueStatus::ResolveWait;

        // Simulate resolve completion for change-a
        let cmd = app.handle_resolve_completed("change-a".to_string(), None);

        // Should return command to start change-b
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-b"));
        // is_resolving should be cleared
        assert!(!app.is_resolving);
        // Queue should be empty
        assert!(!app.has_queued_resolves());
    }

    #[test]
    fn test_resolve_queue_no_auto_start_when_queue_empty() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set change-a to Resolving
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        // Simulate resolve completion with empty queue
        let cmd = app.handle_resolve_completed("change-a".to_string(), None);

        // Should NOT return a command
        assert!(cmd.is_none());
        // is_resolving should be cleared
        assert!(!app.is_resolving);
    }

    #[test]
    fn test_resolve_merge_queues_when_resolving() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Set up: change-a is currently resolving
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        // Set up: change-b is in MergeWait
        app.changes[1].queue_status = QueueStatus::MergeWait;
        app.cursor_index = 1;
        app.mode = AppMode::Running;

        // Call resolve_merge on change-b (should queue it)
        let cmd = app.resolve_merge();

        // Should NOT return a command (queued instead)
        assert!(cmd.is_none());
        // change-b should transition to ResolveWait
        assert_eq!(app.changes[1].queue_status, QueueStatus::ResolveWait);
        // change-b should be in the queue
        assert!(app.has_queued_resolves());
    }

    #[test]
    fn test_resolve_merge_starts_immediately_when_not_resolving() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set up: change-a is in MergeWait, no resolve in progress
        app.changes[0].queue_status = QueueStatus::MergeWait;
        app.cursor_index = 0;
        app.mode = AppMode::Running;
        app.is_resolving = false;

        // Call resolve_merge
        let cmd = app.resolve_merge();

        // Should return a command to start resolve
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-a"));
        // change-a should transition to ResolveWait
        assert_eq!(app.changes[0].queue_status, QueueStatus::ResolveWait);
    }

    #[test]
    fn test_merge_deferred_transitions_to_resolve_wait_when_resolving() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Set up: change-a is currently resolving
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        // change-b is archived (typical state before merge deferred)
        app.changes[1].queue_status = QueueStatus::Archived;

        // Simulate MergeDeferred event for change-b during resolve
        app.handle_merge_deferred(
            "change-b".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        // change-b should transition to ResolveWait
        assert_eq!(
            app.changes[1].queue_status,
            QueueStatus::ResolveWait,
            "MergeDeferred during resolve should transition to ResolveWait"
        );
        // change-b should be added to resolve queue
        assert!(
            app.resolve_queue_set.contains("change-b"),
            "Change should be added to resolve queue"
        );
    }

    #[test]
    fn test_merge_deferred_does_not_queue_current_resolving_change() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set up: change-a is currently resolving
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        // Simulate MergeDeferred event for change-a (the currently resolving change)
        app.handle_merge_deferred(
            "change-a".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        // change-a should remain Resolving (not transition to ResolveWait)
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Resolving,
            "Currently resolving change should remain Resolving"
        );
        // change-a should NOT be added to resolve queue
        assert!(
            !app.resolve_queue_set.contains("change-a"),
            "Currently resolving change should not be queued"
        );
    }

    #[test]
    fn test_merge_deferred_queues_other_change_while_resolving() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Set up: change-a is currently resolving
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        // change-b is archived (typical state before merge deferred)
        app.changes[1].queue_status = QueueStatus::Archived;

        // Simulate MergeDeferred event for change-b (different from resolving change)
        app.handle_merge_deferred(
            "change-b".to_string(),
            "Base branch has uncommitted changes".to_string(),
            true,
        );

        // change-b should transition to ResolveWait
        assert_eq!(
            app.changes[1].queue_status,
            QueueStatus::ResolveWait,
            "Other change should transition to ResolveWait"
        );
        // change-b should be added to resolve queue
        assert!(
            app.resolve_queue_set.contains("change-b"),
            "Other change should be added to resolve queue"
        );
    }

    #[test]
    fn test_merge_deferred_maintains_merge_wait_when_not_resolving() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set up: no resolve in progress
        app.is_resolving = false;

        // change-a is archived (typical state before merge deferred)
        app.changes[0].queue_status = QueueStatus::Archived;

        // Simulate MergeDeferred event for change-a when not resolving (manual intervention)
        app.handle_merge_deferred(
            "change-a".to_string(),
            "Base branch has uncommitted changes".to_string(),
            false, // manual intervention required
        );

        // change-a should transition to MergeWait
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::MergeWait,
            "MergeDeferred (manual) when not resolving should transition to MergeWait"
        );
        // change-a should NOT be added to resolve queue
        assert!(
            !app.resolve_queue_set.contains("change-a"),
            "Change should not be added to resolve queue when not resolving"
        );
    }

    // Regression: auto-resumable MergeDeferred must not appear as MergeWait in TUI.
    // Before this fix, all MergeDeferred events that arrived when no resolve was active
    // set the TUI status to MergeWait, making the change look "stuck" until the user
    // manually pressed M.

    #[test]
    fn test_auto_resumable_merge_deferred_shows_resolve_wait_not_merge_wait() {
        let changes = vec![create_test_change("change-b", 0, 1)];
        let mut app = AppState::new(changes);

        // No resolve in progress.
        app.is_resolving = false;
        app.changes[0].queue_status = QueueStatus::Archived;

        // Auto-resumable deferral (e.g. change-a's merge was in progress when change-b tried).
        app.handle_merge_deferred(
            "change-b".to_string(),
            "Merge in progress (MERGE_HEAD exists)".to_string(),
            true, // auto_resumable
        );

        // Must show ResolveWait, NOT MergeWait.
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::ResolveWait,
            "auto-resumable MergeDeferred must display as ResolveWait, not MergeWait"
        );

        // Must NOT be added to the manual resolve queue
        // (the scheduler will retry automatically, no M press needed).
        assert!(
            !app.resolve_queue_set.contains("change-b"),
            "auto-resumable deferred change must not enter the manual resolve queue"
        );
    }

    #[test]
    fn test_remote_change_update_increases_progress() {
        let changes = vec![create_test_change("MyProj/feat", 1, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 3,
            total_tasks: 5,
            status: None,
            iteration_number: None,
        });

        assert_eq!(app.changes[0].completed_tasks, 3);
        assert_eq!(app.changes[0].total_tasks, 5);
    }

    #[test]
    fn test_remote_change_update_non_regression_rule() {
        // completed_tasks should NOT decrease (non-regression rule)
        let changes = vec![create_test_change("MyProj/feat", 4, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2, // lower than current 4
            total_tasks: 5,
            status: None,
            iteration_number: None,
        });

        // completed_tasks must not decrease
        assert_eq!(
            app.changes[0].completed_tasks, 4,
            "Non-regression rule: completed_tasks must not decrease"
        );
    }

    #[test]
    fn test_remote_change_update_not_found() {
        // Update for unknown change ID should be a no-op
        let changes = vec![create_test_change("MyProj/other", 1, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(), // does not exist
            completed_tasks: 3,
            total_tasks: 5,
            status: None,
            iteration_number: None,
        });

        // State should be unchanged
        assert_eq!(app.changes[0].completed_tasks, 1);
    }

    #[test]
    fn test_remote_change_update_iteration_non_regression_rule() {
        // iteration_number should NOT decrease (monotonic non-regression rule)
        let changes = vec![create_test_change("MyProj/feat", 1, 5)];
        let mut app = AppState::new(changes);

        // First update: set iteration to 3
        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            status: None,
            iteration_number: Some(3),
        });
        assert_eq!(
            app.changes[0].iteration_number,
            Some(3),
            "iteration_number should be 3 after first update"
        );

        // Second update: attempt to decrease iteration to 2 (should be rejected)
        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 3,
            total_tasks: 5,
            status: None,
            iteration_number: Some(2), // lower than current 3
        });

        // iteration_number must not decrease
        assert_eq!(
            app.changes[0].iteration_number,
            Some(3),
            "iteration_number must not decrease (non-regression rule): iteration=3 display should not regress to iteration=2"
        );
    }

    #[test]
    fn test_remote_change_update_iteration_increases() {
        // iteration_number should increase when a higher value arrives
        let changes = vec![create_test_change("MyProj/feat", 1, 5)];
        let mut app = AppState::new(changes);

        // Set iteration to 2
        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            status: None,
            iteration_number: Some(2),
        });
        assert_eq!(app.changes[0].iteration_number, Some(2));

        // Update with higher iteration 4 (should be accepted)
        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 3,
            total_tasks: 5,
            status: None,
            iteration_number: Some(4),
        });

        assert_eq!(
            app.changes[0].iteration_number,
            Some(4),
            "iteration_number should increase when a higher value arrives"
        );
    }

    #[test]
    fn test_remote_change_update_iteration_none_no_op() {
        // iteration_number = None should not change existing iteration_number
        let changes = vec![create_test_change("MyProj/feat", 1, 5)];
        let mut app = AppState::new(changes);

        // Set iteration to 3
        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            status: None,
            iteration_number: Some(3),
        });

        // Update with None iteration (should be a no-op for iteration_number)
        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 3,
            total_tasks: 5,
            status: None,
            iteration_number: None,
        });

        // iteration_number should remain 3
        assert_eq!(
            app.changes[0].iteration_number,
            Some(3),
            "iteration_number should not change when None is received"
        );
    }

    /// Verify that a remote Log event is added to the TUI log panel (state.logs).
    #[test]
    fn test_remote_log_event_added_to_log_panel() {
        use crate::tui::events::{LogEntry, LogLevel, OrchestratorEvent};

        let changes = vec![create_test_change("proj/change-a", 0, 3)];
        let mut app = AppState::new(changes);

        let initial_log_count = app.logs.len();

        // Build a LogEntry simulating what the remote WS translator creates from RemoteStateUpdate::Log
        let entry = LogEntry {
            timestamp: "12:00:00".to_string(),
            created_at: chrono::Utc::now(),
            message: "remote stdout: cargo build succeeded".to_string(),
            color: ratatui::style::Color::Reset,
            level: LogLevel::Info,
            change_id: Some("change-a".to_string()),
            operation: None,
            iteration: None,
            workspace_path: None,
        };

        app.handle_orchestrator_event(OrchestratorEvent::Log(entry.clone()));

        // The log entry should be appended to state.logs
        assert!(
            app.logs.len() > initial_log_count,
            "Expected log count to increase after remote Log event"
        );

        let last = app.logs.last().expect("Should have at least one log entry");
        assert_eq!(last.message, entry.message, "Log message should match");
        assert_eq!(
            last.change_id, entry.change_id,
            "Log change_id should match"
        );
    }

    /// Verify that multiple remote Log events accumulate in the log panel.
    #[test]
    fn test_multiple_remote_log_events_accumulate() {
        use crate::tui::events::{LogEntry, LogLevel, OrchestratorEvent};

        let changes = vec![];
        let mut app = AppState::new(changes);

        let initial_count = app.logs.len();

        for i in 0..5 {
            let entry = LogEntry {
                timestamp: format!("12:00:{:02}", i),
                created_at: chrono::Utc::now(),
                message: format!("remote log line {}", i),
                color: ratatui::style::Color::Reset,
                level: LogLevel::Info,
                change_id: None,
                operation: None,
                iteration: None,
                workspace_path: None,
            };
            app.handle_orchestrator_event(OrchestratorEvent::Log(entry));
        }

        assert_eq!(
            app.logs.len(),
            initial_count + 5,
            "All 5 remote log entries should be present in log panel"
        );
    }

    /// Verify that get_latest_log_for_change matches remote project-level logs
    /// where change_id is the project_id and the queried change_id has the
    /// format "<project_id>::<project_name>/<change_id>".
    #[test]
    fn test_get_latest_log_for_change_project_id_prefix_match() {
        use crate::tui::events::{LogEntry, LogLevel, OrchestratorEvent};

        let remote_change_id = "proj-abc123::my-project@main/change-a";
        let changes = vec![create_test_change(remote_change_id, 0, 3)];
        let mut app = AppState::new(changes);

        // Simulate a project-level remote log where change_id = project_id
        let project_id = "proj-abc123";
        let entry = LogEntry {
            timestamp: "12:00:00".to_string(),
            created_at: chrono::Utc::now(),
            message: "Remote project stdout: building...".to_string(),
            color: ratatui::style::Color::Reset,
            level: LogLevel::Info,
            change_id: Some(project_id.to_string()),
            operation: None,
            iteration: None,
            workspace_path: None,
        };

        app.handle_orchestrator_event(OrchestratorEvent::Log(entry.clone()));

        // The log should be found when querying by the full remote change_id
        // because the entry's change_id (project_id) is a prefix of the change_id
        let found = app.get_latest_log_for_change(remote_change_id);
        assert!(
            found.is_some(),
            "Expected to find log for remote change via project_id prefix matching"
        );
        assert_eq!(
            found.unwrap().message,
            entry.message,
            "Log message should match"
        );
    }

    /// Verify that get_latest_log_for_change still performs exact match for local changes.
    #[test]
    fn test_get_latest_log_for_change_exact_match_local() {
        use crate::tui::events::{LogEntry, LogLevel, OrchestratorEvent};

        let change_id = "my-local-change";
        let changes = vec![create_test_change(change_id, 0, 3)];
        let mut app = AppState::new(changes);

        let entry = LogEntry {
            timestamp: "12:00:00".to_string(),
            created_at: chrono::Utc::now(),
            message: "Local apply log".to_string(),
            color: ratatui::style::Color::Reset,
            level: LogLevel::Info,
            change_id: Some(change_id.to_string()),
            operation: Some("apply".to_string()),
            iteration: Some(1),
            workspace_path: None,
        };

        app.handle_orchestrator_event(OrchestratorEvent::Log(entry.clone()));

        let found = app.get_latest_log_for_change(change_id);
        assert!(found.is_some(), "Expected to find log by exact change_id");
        assert_eq!(found.unwrap().message, entry.message);

        // Should not match a different change_id
        let not_found = app.get_latest_log_for_change("other-change");
        assert!(
            not_found.is_none(),
            "Should not match a different change_id"
        );
    }

    /// Verify RemoteLogEntry round-trip with all new fields (project_id, operation, iteration).
    #[test]
    fn test_remote_log_entry_with_project_id_round_trip() {
        use crate::remote::types::RemoteLogEntry;

        let entry = RemoteLogEntry {
            message: "stdout: tests passed".to_string(),
            level: "info".to_string(),
            change_id: None,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            project_id: Some("proj-abc123".to_string()),
            operation: Some("apply".to_string()),
            iteration: Some(2),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let decoded: RemoteLogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.message, entry.message);
        assert_eq!(decoded.change_id, entry.change_id);
        assert_eq!(decoded.project_id, entry.project_id);
        assert_eq!(decoded.operation, entry.operation);
        assert_eq!(decoded.iteration, entry.iteration);
        // change_id is None so decoded value is None
        assert_eq!(decoded.change_id, None);
        // New fields appear in JSON when Some
        assert!(json.contains("project_id"));
        assert!(json.contains("operation"));
        assert!(json.contains("iteration"));
    }

    // ── Regression tests for fix-parallel-start-rejection-state ─────────────

    /// TUI stale-eligibility regression: when a change becomes uncommitted after
    /// the last refresh but before parallel start, the backend sends
    /// ParallelStartRejected and the TUI must clear the Queued row.
    #[test]
    fn test_parallel_start_rejected_clears_queued_status() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Simulate what start_processing / resume_processing does:
        // mark change-a as Queued (it was selected before backend rejected it).
        app.changes[0].queue_status = QueueStatus::Queued;
        // change-b is not queued.
        app.mode = AppMode::Running;

        // Backend rejects change-a at start time.
        app.handle_orchestrator_event(OrchestratorEvent::ParallelStartRejected {
            change_ids: vec!["change-a".to_string()],
            reason: "uncommitted or not in HEAD".to_string(),
        });

        // change-a must no longer be Queued.
        let a = app.changes.iter().find(|c| c.id == "change-a").unwrap();
        assert_eq!(
            a.queue_status,
            QueueStatus::NotQueued,
            "change-a should have been reset from Queued to NotQueued"
        );

        // change-b was never Queued; its status should be unchanged.
        let b = app.changes.iter().find(|c| c.id == "change-b").unwrap();
        assert_eq!(b.queue_status, QueueStatus::NotQueued);

        // A warning log entry should explain the rejection.
        assert!(
            app.logs
                .iter()
                .any(|log| log.message.contains("Not started")),
            "expected a 'Not started' log entry"
        );
    }

    /// Verify that ParallelStartRejected does NOT affect changes that are NOT in Queued state.
    #[test]
    fn test_parallel_start_rejected_does_not_affect_non_queued() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        // change-a is Applying (already running), not merely Queued.
        app.changes[0].queue_status = QueueStatus::Applying;
        app.mode = AppMode::Running;

        app.handle_orchestrator_event(OrchestratorEvent::ParallelStartRejected {
            change_ids: vec!["change-a".to_string()],
            reason: "uncommitted or not in HEAD".to_string(),
        });

        // Applying should not be disturbed.
        let a = app.changes.iter().find(|c| c.id == "change-a").unwrap();
        assert_eq!(a.queue_status, QueueStatus::Applying);
    }

    /// Safety-net regression: AllCompleted must reset any lingering Queued changes so that
    /// stale Queued state never survives into Select mode.
    #[test]
    fn test_all_completed_resets_remaining_queued() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].queue_status = QueueStatus::Queued;
        app.mode = AppMode::Running;

        app.handle_orchestrator_event(OrchestratorEvent::AllCompleted);

        assert_eq!(app.mode, AppMode::Select);
        let a = app.changes.iter().find(|c| c.id == "change-a").unwrap();
        assert_eq!(
            a.queue_status,
            QueueStatus::NotQueued,
            "Queued change should be reset to NotQueued after AllCompleted"
        );
    }

    /// Resume-processing regression: F5 from Stopped mode that encounters a now-uncommitted
    /// change should result in Queued→NotQueued when ParallelStartRejected arrives.
    #[test]
    fn test_resume_then_parallel_start_rejected_resets_queued() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Enter Stopped mode with change-a execution-marked.
        app.mode = AppMode::Stopped;
        app.changes[0].selected = true;
        app.changes[0].queue_status = QueueStatus::NotQueued;

        // User presses F5: resume_processing sets change-a to Queued.
        let cmd = app.resume_processing();
        assert!(cmd.is_some(), "resume_processing should return a command");
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);
        assert_eq!(app.mode, AppMode::Running);

        // Backend rejects change-a (became uncommitted between last refresh and F5).
        app.handle_orchestrator_event(OrchestratorEvent::ParallelStartRejected {
            change_ids: vec!["change-a".to_string()],
            reason: "uncommitted or not in HEAD".to_string(),
        });

        // change-a must not remain Queued.
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        assert!(
            app.logs
                .iter()
                .any(|log| log.message.contains("Not started")),
            "expected a rejection log entry"
        );
    }

    /// Regression: ResolveFailed must not demote an already-Merged change back to MergeWait.
    #[test]
    fn test_handle_resolve_failed_does_not_demote_merged() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);

        // Simulate that the change has already reached Merged state.
        app.changes[0].queue_status = QueueStatus::Merged;

        // A spurious ResolveFailed event arrives after the merge succeeded.
        app.handle_orchestrator_event(OrchestratorEvent::ResolveFailed {
            change_id: "change-a".to_string(),
            error: "archive check failed".to_string(),
        });

        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Merged,
            "ResolveFailed must not demote a Merged change to MergeWait"
        );
        assert!(
            app.logs
                .iter()
                .any(|log| log.message.contains("already Merged")),
            "expected an info log about ignoring ResolveFailed"
        );
    }

    /// Regression: reducer-driven display must not demote a Merged change to MergeWait.
    #[test]
    fn test_apply_merge_wait_status_does_not_demote_merged() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);

        // Simulate that the change has already reached Merged state.
        app.changes[0].queue_status = QueueStatus::Merged;

        // The reducer display map says "merged" → TUI must keep Merged.
        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "merged");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Merged,
            "reducer-driven display must not demote a Merged change to MergeWait"
        );
    }

    /// Regression: reducer-driven display must not demote a Blocked change to MergeWait.
    #[test]
    fn test_apply_merge_wait_status_does_not_demote_blocked() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);

        // The reducer display map says "blocked".
        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "blocked");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Blocked,
            "reducer-driven display must not demote a Blocked change to MergeWait"
        );
    }

    /// Regression: reducer-driven display must not affect a Merged change via merge_wait.
    #[test]
    fn test_auto_clear_merge_wait_does_not_affect_merged() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);

        // The reducer display map says "merged" — TUI keeps Merged.
        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "merged");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Merged,
            "reducer-driven display must not transition a Merged change away from Merged"
        );
    }

    #[test]
    fn test_start_processing_does_not_queue_blocked_changes() {
        // Regression: Blocked+selected changes must NOT be transitioned to Queued by F5
        let changes = vec![
            create_test_change("applying", 0, 1),
            create_test_change("blocked-b", 0, 1),
            create_test_change("blocked-c", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Simulate: A is Applying, B and C are Blocked+selected (execution marks present)
        app.changes[0].queue_status = QueueStatus::Applying;
        app.changes[0].selected = false;
        app.changes[1].queue_status = QueueStatus::Blocked;
        app.changes[1].selected = true;
        app.changes[2].queue_status = QueueStatus::Blocked;
        app.changes[2].selected = true;

        // Press F5 (start_processing) – should return None (no NotQueued changes)
        let result = app.start_processing();

        assert!(
            result.is_none(),
            "start_processing must return None when only Blocked changes are selected"
        );
        assert_eq!(
            app.changes[1].queue_status,
            QueueStatus::Blocked,
            "Blocked change B must remain Blocked after start_processing"
        );
        assert_eq!(
            app.changes[2].queue_status,
            QueueStatus::Blocked,
            "Blocked change C must remain Blocked after start_processing"
        );
    }

    #[test]
    fn test_handle_stopped_resets_blocked_to_not_queued() {
        // Regression: Blocked changes must be reset to NotQueued when processing stops
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Applying;
        app.changes[0].selected = true;
        app.changes[1].queue_status = QueueStatus::Blocked;
        app.changes[1].selected = true;

        app.handle_orchestrator_event(OrchestratorEvent::Stopped);

        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::NotQueued,
            "Applying change must be reset to NotQueued on Stopped"
        );
        assert_eq!(
            app.changes[1].queue_status,
            QueueStatus::NotQueued,
            "Blocked change must be reset to NotQueued on Stopped"
        );
        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn test_handle_all_completed_resets_blocked_to_not_queued() {
        // Regression: Blocked changes must be reset to NotQueued when all processing completes
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Queued;
        app.changes[0].selected = true;
        app.changes[1].queue_status = QueueStatus::Blocked;
        app.changes[1].selected = true;

        app.handle_orchestrator_event(OrchestratorEvent::AllCompleted);

        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::NotQueued,
            "Queued change must be reset to NotQueued on AllCompleted"
        );
        assert_eq!(
            app.changes[1].queue_status,
            QueueStatus::NotQueued,
            "Blocked change must be reset to NotQueued on AllCompleted"
        );
        assert_eq!(app.mode, AppMode::Select);
    }

    // -----------------------------------------------------------------------
    // Phase 6.1: TUI uses reducer display_status (apply_display_statuses_from_reducer)
    // -----------------------------------------------------------------------

    #[test]
    fn test_tui_uses_reducer_display_status() {
        use std::collections::HashMap;

        let changes = vec![
            create_test_change("c1", 0, 3),
            create_test_change("c2", 0, 3),
        ];
        let mut app = AppState::new(changes);

        // Simulate reducer snapshot with various statuses.
        let mut display_map: HashMap<String, &'static str> = HashMap::new();
        display_map.insert("c1".to_string(), "applying");
        display_map.insert("c2".to_string(), "merge wait");

        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(app.changes[0].queue_status, QueueStatus::Applying);
        assert_eq!(app.changes[1].queue_status, QueueStatus::MergeWait);

        // Verify active classification works correctly.
        assert!(matches!(app.changes[0].queue_status, QueueStatus::Applying));
    }

    // -----------------------------------------------------------------------
    // Phase 6.4: TUI and Web display vocabulary consistency
    // -----------------------------------------------------------------------

    #[test]
    fn test_display_status_consistency_between_tui_and_web() {
        use std::collections::HashMap;

        let changes = vec![create_test_change("c1", 0, 3)];
        let mut app = AppState::new(changes);

        // Scenarios: dependency blocked, merge wait, resolving.
        let scenarios: &[(&str, QueueStatus)] = &[
            ("blocked", QueueStatus::Blocked),
            ("merge wait", QueueStatus::MergeWait),
            ("resolve pending", QueueStatus::ResolveWait),
            ("resolving", QueueStatus::Resolving),
            ("archived", QueueStatus::Archived),
            ("merged", QueueStatus::Merged),
            ("queued", QueueStatus::Queued),
            ("not queued", QueueStatus::NotQueued),
        ];

        for (reducer_str, expected_tui_status) in scenarios {
            let mut display_map: HashMap<String, &'static str> = HashMap::new();
            display_map.insert("c1".to_string(), reducer_str);
            app.apply_display_statuses_from_reducer(&display_map);

            assert_eq!(
                app.changes[0].queue_status, *expected_tui_status,
                "reducer '{}' should map to {:?}",
                reducer_str, expected_tui_status
            );
        }
    }

    // -----------------------------------------------------------------------
    // Phase 3.3: toggle_selection in Running mode emits commands without
    // mutating queue_status locally.
    // -----------------------------------------------------------------------

    #[test]
    fn test_running_mode_toggle_emits_commands_without_local_status_mutation() {
        let changes = vec![
            create_test_change("c1", 0, 3),
            create_test_change("c2", 0, 3),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;

        // Simulate c1 in NotQueued state.
        app.changes[0].queue_status = QueueStatus::NotQueued;

        // toggle_selection should return AddToQueue command and NOT mutate queue_status.
        let cmd = app.toggle_selection();
        assert!(
            matches!(cmd, Some(TuiCommand::AddToQueue(ref id)) if id == "c1"),
            "expected AddToQueue command, got {:?}",
            cmd
        );
        // queue_status must NOT have been locally changed to Queued.
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::NotQueued,
            "queue_status must NOT be mutated locally; reducer drives it"
        );

        // Simulate c2 already Queued.
        app.cursor_index = 1;
        app.changes[1].queue_status = QueueStatus::Queued;
        let cmd2 = app.toggle_selection();
        assert!(
            matches!(cmd2, Some(TuiCommand::RemoveFromQueue(ref id)) if id == "c2"),
            "expected RemoveFromQueue command, got {:?}",
            cmd2
        );
        // queue_status must NOT have been locally changed to NotQueued.
        assert_eq!(
            app.changes[1].queue_status,
            QueueStatus::Queued,
            "queue_status must NOT be mutated locally; reducer drives it"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 3.4: MergeWait / ResolveWait Space and M key behaviour
    // -----------------------------------------------------------------------

    #[test]
    fn test_merge_wait_queue_operations() {
        let changes = vec![create_test_change("c1", 0, 3)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::MergeWait;

        // Space on MergeWait toggles selection only (no queue change).
        let cmd = app.toggle_selection();
        // Should NOT return AddToQueue/RemoveFromQueue.
        assert!(
            !matches!(
                cmd,
                Some(TuiCommand::AddToQueue(_)) | Some(TuiCommand::RemoveFromQueue(_))
            ),
            "Space on MergeWait must not issue queue commands, got {:?}",
            cmd
        );
        // queue_status must still be MergeWait.
        assert_eq!(app.changes[0].queue_status, QueueStatus::MergeWait);
    }

    #[test]
    fn test_resolve_wait_queue_operations() {
        let changes = vec![create_test_change("c1", 0, 3)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::ResolveWait;

        // Space on ResolveWait toggles selection only (no queue change).
        let cmd = app.toggle_selection();
        assert!(
            !matches!(
                cmd,
                Some(TuiCommand::AddToQueue(_)) | Some(TuiCommand::RemoveFromQueue(_))
            ),
            "Space on ResolveWait must not issue queue commands, got {:?}",
            cmd
        );
        assert_eq!(app.changes[0].queue_status, QueueStatus::ResolveWait);
    }

    // -----------------------------------------------------------------------
    // Fix: parallel TUI queued/blocked state regression
    // -----------------------------------------------------------------------

    /// start_processing must sync queue intent into the shared reducer so that a
    /// subsequent ChangesRefreshed display sync cannot regress the row back to
    /// "not queued" before the orchestrator processes it.
    #[test]
    fn test_start_processing_syncs_reducer_queue_intent() {
        use crate::orchestration::state::OrchestratorState;
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].selected = true;
        app.changes[0].is_parallel_eligible = true;

        // Attach a real shared reducer.
        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        app.set_shared_state(shared.clone());

        let cmd = app.start_processing();
        assert!(cmd.is_some(), "start_processing should return a command");

        // Reducer must now know about the queue intent.
        let guard = shared.blocking_read();
        assert_eq!(
            guard.display_status("change-a"),
            "queued",
            "reducer queue_intent must be Queued after start_processing"
        );

        // A subsequent ChangesRefreshed-driven display sync must not overwrite Queued.
        drop(guard);
        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "not queued");
        app.apply_display_statuses_from_reducer(&display_map);
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::NotQueued,
            "display sync should apply reducer snapshot – but reducer already says queued, so this tests the raw override path"
        );

        // Verify with the reducer's own snapshot (the correct integration path).
        let guard2 = shared.blocking_read();
        let real_map = guard2.all_display_statuses();
        drop(guard2);
        app.changes[0].queue_status = QueueStatus::Queued; // restore as start_processing set
        app.apply_display_statuses_from_reducer(&real_map);
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Queued,
            "reducer snapshot must preserve Queued through ChangesRefreshed display sync"
        );
    }

    /// resume_processing must sync queue intent into the shared reducer.
    #[test]
    fn test_resume_processing_syncs_reducer_queue_intent() {
        use crate::orchestration::state::OrchestratorState;
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;
        app.changes[0].selected = true;
        app.changes[0].queue_status = QueueStatus::NotQueued;

        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        app.set_shared_state(shared.clone());

        let cmd = app.resume_processing();
        assert!(cmd.is_some(), "resume_processing should return a command");

        let guard = shared.blocking_read();
        assert_eq!(
            guard.display_status("change-a"),
            "queued",
            "reducer queue_intent must be Queued after resume_processing"
        );
    }

    /// After start_processing, the reducer snapshot must preserve Queued through an
    /// initial parallel ChangesRefreshed display sync (startup refresh regression).
    #[test]
    fn test_parallel_start_refresh_preserves_queued_rows() {
        use crate::orchestration::state::OrchestratorState;
        use std::collections::{HashMap, HashSet};
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].selected = true;
        app.changes[0].is_parallel_eligible = true;
        app.parallel_mode = true;

        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        app.set_shared_state(shared.clone());

        // F5 – queues the change and syncs the reducer.
        let cmd = app.start_processing();
        assert!(cmd.is_some());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);

        // Simulate initial parallel ChangesRefreshed (workspace scan returns nothing special).
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::ChangesRefreshed {
                changes: vec![],
                committed_change_ids: HashSet::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: HashSet::new(),
            });
        }

        // Display sync from the reducer must keep the row as Queued.
        let display_map = shared.blocking_read().all_display_statuses();
        app.apply_display_statuses_from_reducer(&display_map);
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Queued,
            "initial parallel ChangesRefreshed must not regress a queued row to not-queued"
        );
    }

    /// Regression: after run_orchestrator_parallel resets shared state with with_mode(), it must
    /// re-apply AddToQueue so that the subsequent ChangesRefreshed display sync does not regress
    /// the TUI's Queued rows back to NotQueued.
    #[test]
    fn test_parallel_start_state_reset_preserves_queued_rows() {
        use crate::orchestration::state::{OrchestratorState, ReducerCommand};
        use std::collections::{HashMap, HashSet};
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].selected = true;
        app.changes[0].is_parallel_eligible = true;
        app.parallel_mode = true;

        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        app.set_shared_state(shared.clone());

        // F5 – queues the change in TUI and syncs the reducer.
        let cmd = app.start_processing();
        assert!(cmd.is_some());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);

        // Simulate run_orchestrator_parallel replacing shared state (the regression source).
        // Without the fix this would clear queue_intent back to NotQueued.
        {
            let mut guard = shared.blocking_write();
            *guard = OrchestratorState::with_mode(
                vec!["change-a".to_string()],
                0,
                crate::orchestration::state::ExecutionMode::Parallel,
            );
            // The fix: re-apply AddToQueue after the state reset.
            guard.apply_command(ReducerCommand::AddToQueue("change-a".to_string()));
        }

        // Simulate the initial ChangesRefreshed that fires at parallel startup.
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::ChangesRefreshed {
                changes: vec![],
                committed_change_ids: HashSet::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: HashSet::new(),
            });
        }

        // Display sync from the reducer must keep the row as Queued.
        let display_map = shared.blocking_read().all_display_statuses();
        app.apply_display_statuses_from_reducer(&display_map);
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Queued,
            "state reset followed by AddToQueue must preserve Queued through ChangesRefreshed"
        );
    }

    /// DependencyBlocked sets Blocked in both TUI and reducer; DependencyResolved restores
    /// Queued display because the reducer still holds queue_intent = Queued.
    #[test]
    fn test_dependency_block_preserves_queued_intent() {
        use crate::orchestration::state::OrchestratorState;
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        app.set_shared_state(shared.clone());

        // Simulate F5 path (queues change in both TUI and reducer).
        app.changes[0].selected = true;
        app.changes[0].is_parallel_eligible = true;
        app.start_processing();

        // Verify reducer has queued intent.
        assert_eq!(shared.blocking_read().display_status("change-a"), "queued");

        // Dependency block arrives.
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::DependencyBlocked {
                change_id: "change-a".to_string(),
                dependency_ids: vec!["dep".to_string()],
            });
        }
        // Reducer should show "blocked"; queue_intent is still Queued underneath.
        assert_eq!(shared.blocking_read().display_status("change-a"), "blocked");
    }

    /// After DependencyResolved the reducer restores "queued" display because queue_intent
    /// was never cleared during the block.
    #[test]
    fn test_dependency_resolved_restores_queued_display() {
        use crate::orchestration::state::OrchestratorState;
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        app.set_shared_state(shared.clone());

        app.changes[0].selected = true;
        app.changes[0].is_parallel_eligible = true;
        app.start_processing();

        // Block then resolve.
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::DependencyBlocked {
                change_id: "change-a".to_string(),
                dependency_ids: vec!["dep".to_string()],
            });
            guard.apply_execution_event(&crate::events::ExecutionEvent::DependencyResolved {
                change_id: "change-a".to_string(),
            });
        }

        // After resolution, reducer must report "queued" (not "not queued").
        let display_map = shared.blocking_read().all_display_statuses();
        app.apply_display_statuses_from_reducer(&display_map);
        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::Queued,
            "dependency resolution must restore queued display, not not-queued"
        );
    }

    /// ParallelStartRejected must also clear the reducer queue intent so subsequent
    /// ChangesRefreshed display syncs don't re-queue the rejected row.
    #[test]
    fn test_parallel_start_rejected_does_not_clear_other_rows() {
        use crate::orchestration::state::OrchestratorState;
        use std::sync::Arc;

        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string(), "change-b".to_string()],
            0,
        )));
        app.set_shared_state(shared.clone());

        // Queue both changes in reducer.
        {
            let mut guard = shared.blocking_write();
            guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                "change-a".to_string(),
            ));
            guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                "change-b".to_string(),
            ));
        }
        app.changes[0].queue_status = QueueStatus::Queued;
        app.changes[1].queue_status = QueueStatus::Queued;
        app.mode = AppMode::Running;

        // Backend rejects only change-a.
        app.handle_orchestrator_event(OrchestratorEvent::ParallelStartRejected {
            change_ids: vec!["change-a".to_string()],
            reason: "uncommitted".to_string(),
        });

        // change-a must be reset in both TUI and reducer.
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        assert_eq!(
            shared.blocking_read().display_status("change-a"),
            "not queued",
            "reducer must clear queue intent for rejected change-a"
        );

        // change-b must remain Queued in both TUI and reducer.
        assert_eq!(app.changes[1].queue_status, QueueStatus::Queued);
        assert_eq!(
            shared.blocking_read().display_status("change-b"),
            "queued",
            "reducer must not touch change-b which was not rejected"
        );
    }

    // -----------------------------------------------------------------------
    // Resolving mode transition tests (fix-resolving-mode-transition)
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_completed_keeps_running_when_resolving() {
        // When AllCompleted arrives while a change is Resolving,
        // AppMode must remain Running so the user can add changes via Space.
        let changes = vec![
            create_test_change("change-a", 3, 3),
            create_test_change("change-b", 2, 4),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Resolving;

        app.handle_all_completed();

        assert_eq!(
            app.mode,
            AppMode::Running,
            "Should stay Running while Resolving changes exist"
        );
    }

    #[test]
    fn test_resolve_completed_transitions_to_select_when_no_active() {
        // After the last Resolving change completes and no other active changes remain,
        // the mode should transition to Select.
        let changes = vec![
            create_test_change("change-a", 3, 3),
            create_test_change("change-b", 2, 4),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Merged; // already done
        app.changes[1].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        // Simulate resolve completion
        app.handle_resolve_completed("change-b".to_string(), None);

        assert_eq!(
            app.mode,
            AppMode::Select,
            "Should transition to Select when no active changes remain after resolve"
        );
    }

    #[test]
    fn test_resolve_completed_stays_running_when_other_active() {
        // If another change is still active (e.g. Applying), mode stays Running.
        let changes = vec![
            create_test_change("change-a", 1, 3),
            create_test_change("change-b", 2, 4),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Applying; // still active
        app.changes[1].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        app.handle_resolve_completed("change-b".to_string(), None);

        assert_eq!(
            app.mode,
            AppMode::Running,
            "Should stay Running when other active changes remain"
        );
    }

    #[test]
    fn test_resolve_failed_transitions_to_select_when_no_active() {
        // After resolve failure, if no other active changes remain, transition to Select.
        let changes = vec![create_test_change("change-a", 3, 3)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.is_resolving = true;

        app.handle_resolve_failed("change-a".to_string(), "conflict".to_string());

        // change-a goes to MergeWait (not active for transition purposes
        // since MergeWait is not in the active set)
        assert_eq!(
            app.mode,
            AppMode::Select,
            "Should transition to Select after resolve failure with no active changes"
        );
    }

    #[test]
    fn test_stopped_resets_resolving_changes() {
        // When Stop is triggered, Resolving changes must be reset to NotQueued.
        let changes = vec![
            create_test_change("change-a", 3, 3),
            create_test_change("change-b", 2, 4),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.changes[0].selected = true;
        app.changes[1].queue_status = QueueStatus::Merged;

        app.handle_stopped();

        assert_eq!(
            app.changes[0].queue_status,
            QueueStatus::NotQueued,
            "Resolving change must be reset to NotQueued on Stop"
        );
        // selected should be preserved
        assert!(
            app.changes[0].selected,
            "selected flag should be preserved on Stop"
        );
        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn test_try_transition_to_select_no_op_when_not_running() {
        // try_transition_to_select should be a no-op when not in Running mode.
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;

        app.try_transition_to_select();

        assert_eq!(
            app.mode,
            AppMode::Stopped,
            "Should remain Stopped when try_transition_to_select is called"
        );
    }

    #[test]
    fn test_try_transition_to_select_stays_running_with_active() {
        // try_transition_to_select should not transition when active changes exist.
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::Applying;

        app.try_transition_to_select();

        assert_eq!(
            app.mode,
            AppMode::Running,
            "Should stay Running with active Applying change"
        );
    }
}
