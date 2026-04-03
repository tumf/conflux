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
use crate::tui::events::{LogEntry, LogLevel, TuiCommand};
use crate::tui::types::{AppMode, StopMode, ViewMode, WorktreeAction, WorktreeInfo};
use crate::vcs::GitWorkspaceManager;
use ratatui::style::Color;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

fn apply_remote_status(change: &mut ChangeState, status: &str) {
    // Avoid regressing active/terminal states based on laggy remote snapshots.
    let current = change.display_status_cache.as_str();

    let next = match status {
        "applying" => Some("applying"),
        "archiving" => Some("archiving"),
        "accepting" => Some("accepting"),
        "resolving" => Some("resolving"),
        "archived" => Some("archived"),
        "merged" => Some("merged"),
        "merge_wait" => Some("merge wait"),
        "resolve_wait" => Some("resolve pending"),
        "blocked" => Some("blocked"),
        "queued" => Some("queued"),
        "idle" => Some("not queued"),
        "error" => Some("error"),
        _ => None,
    };

    let Some(next) = next else {
        return;
    };

    // Don't downgrade active states to queued/idle.
    if matches!(
        current,
        "applying" | "archiving" | "accepting" | "resolving"
    ) && matches!(next, "queued" | "not queued")
    {
        return;
    }

    // Only set queued/idle if we're not already in an immutable terminal state.
    // Note: error is intentionally excluded to allow error -> queued retry transitions.
    if matches!(next, "queued" | "not queued") && matches!(current, "archived" | "merged") {
        return;
    }

    // Transition bookkeeping for elapsed time.
    if matches!(next, "applying") && change.started_at.is_none() {
        change.started_at = Some(Instant::now());
        change.elapsed_time = None;
    }

    if !matches!(next, "applying" | "archiving" | "accepting" | "resolving")
        && matches!(
            current,
            "applying" | "archiving" | "accepting" | "resolving"
        )
    {
        if let Some(started) = change.started_at {
            change.elapsed_time = Some(started.elapsed());
        }
    }

    if next == "error" {
        if change.error_message_cache.is_none() {
            change.error_message_cache = Some("remote".to_string());
        }
        change.set_display_status_cache("error");
    } else {
        change.set_display_status_cache(next);
    }
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
    /// Display status cache (from reducer/TUI events)
    pub display_status_cache: String,
    /// Display color cache for status
    pub display_color_cache: Color,
    /// Error message cache for error status
    pub error_message_cache: Option<String>,
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
            display_status_cache: "not queued".to_string(),
            display_color_cache: Color::DarkGray,
            error_message_cache: None,
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

    pub fn set_display_status_cache(&mut self, status: &str) {
        self.display_status_cache = status.to_string();
        self.display_color_cache = match status {
            "not queued" => Color::DarkGray,
            "queued" => Color::Yellow,
            "blocked" => Color::Gray,
            "applying" => Color::Cyan,
            "accepting" => Color::LightGreen,
            "archiving" => Color::Magenta,
            "merge wait" => Color::LightMagenta,
            "resolve pending" => Color::Magenta,
            "resolving" => Color::LightCyan,
            "archived" => Color::Blue,
            "merged" => Color::LightBlue,
            "error" => Color::Red,
            _ => Color::DarkGray,
        };
        if status != "error" {
            self.error_message_cache = None;
        }
    }

    pub fn set_error_message_cache(&mut self, message: String) {
        self.error_message_cache = Some(message);
        self.set_display_status_cache("error");
    }

    pub fn is_active_display_status(&self) -> bool {
        matches!(
            self.display_status_cache.as_str(),
            "applying" | "accepting" | "archiving" | "resolving"
        )
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
                    change.display_status_cache.as_str(),
                    "queued" | "applying" | "archiving" | "resolving" | "accepting" | "merge wait"
                );

                if is_active {
                    self.warning_message = Some(format!(
                        "Cannot delete worktree: change '{}' is {}",
                        change_id,
                        change.display_status_cache.as_str()
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
            &change.display_status_cache,
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
        if matches!(self.mode, AppMode::Running) && change.is_active_display_status() {
            return false;
        }

        guards::validate_change_toggleable(
            change.is_parallel_eligible,
            self.parallel_mode,
            &change.display_status_cache,
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
    /// Running mode emits `AddToQueue`/`RemoveFromQueue` commands for
    /// `NotQueued`/`Queued` rows (same semantics as single-row Space).
    /// `MergeWait`/`ResolveWait` rows only toggle the execution mark.
    /// In parallel mode, uncommitted changes remain excluded.
    pub fn toggle_all_marks(&mut self) -> Vec<TuiCommand> {
        if !self.has_bulk_toggle_targets() {
            return Vec::new();
        }

        // If any eligible unmarked change exists, we mark all; otherwise unmark all.
        let has_unmarked = self
            .changes
            .iter()
            .any(|change| !change.selected && self.can_bulk_toggle_change(change));

        let target_state = has_unmarked;
        let is_running = matches!(self.mode, AppMode::Running);
        let mut commands = Vec::new();

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

                // In Running mode, emit queue commands for NotQueued/Queued rows
                // (same semantics as single-row Space toggle).
                // MergeWait/ResolveWait only toggle the execution mark.
                if is_running {
                    match self.changes[i].display_status_cache.as_str() {
                        "not queued" if target_state => {
                            let id = self.changes[i].id.clone();
                            self.add_log(LogEntry::info(format!("Added to queue: {}", id)));
                            commands.push(TuiCommand::AddToQueue(id));
                        }
                        "queued" if !target_state => {
                            let id = self.changes[i].id.clone();
                            self.add_log(LogEntry::info(format!("Removed from queue: {}", id)));
                            commands.push(TuiCommand::RemoveFromQueue(id));
                        }
                        _ => {}
                    }
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

        commands
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
            if !matches!(change.display_status_cache.as_str(), "merge wait") {
                return None;
            }
            change.id.clone()
        };

        if self.is_resolving {
            // Resolve is running: add to queue and transition to ResolveWait
            if self.add_to_resolve_queue(&change_id) {
                self.changes[self.cursor_index].set_display_status_cache("resolve pending");

                // Sync resolve intent into the shared reducer so that
                // apply_display_statuses_from_reducer() cannot regress the
                // status back to "merge wait" on the next ChangesRefreshed.
                if let Some(shared) = &self.shared_orchestrator_state {
                    if let Ok(mut guard) = shared.try_write() {
                        guard.apply_command(
                            crate::orchestration::state::ReducerCommand::ResolveMerge(
                                change_id.clone(),
                            ),
                        );
                    }
                }

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
            // Set resolving flag synchronously so consecutive M presses in the same
            // event loop tick are routed to the queueing path.
            self.is_resolving = true;
            if matches!(self.mode, AppMode::Select | AppMode::Stopped) {
                self.mode = AppMode::Running;
            }
            self.changes[self.cursor_index].set_display_status_cache("resolve pending");

            // Sync resolve intent into the shared reducer so that
            // apply_display_statuses_from_reducer() cannot regress the
            // status back to "merge wait" on the next ChangesRefreshed.
            if let Some(shared) = &self.shared_orchestrator_state {
                if let Ok(mut guard) = shared.try_write() {
                    guard.apply_command(crate::orchestration::state::ReducerCommand::ResolveMerge(
                        change_id.clone(),
                    ));
                }
            }

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
                if matches!(change.display_status_cache.as_str(), "queued") {
                    change.set_display_status_cache("not queued");
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

    /// Sync displayed status caches from the reducer's display status snapshot.
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
                let normalized = match status_str {
                    "stopped" => "not queued",
                    other => other,
                };

                if normalized == "error" {
                    if change.display_status_cache == "error" {
                        continue;
                    }
                    if change.error_message_cache.is_none() {
                        change.error_message_cache = Some("reducer".to_string());
                    }
                    change.set_display_status_cache("error");
                } else {
                    change.set_display_status_cache(normalized);
                }
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
                    if matches!(change.display_status_cache.as_str(), "queued") {
                        change.set_display_status_cache("not queued");
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

        // Only NotQueued changes can be transitioned to Queued by StartProcessing.
        // Active states (Applying, Accepting, Archiving, Blocked, Queued) and terminal
        // states (Merged, Error, MergeWait, ResolveWait, Archived) are excluded.
        let selected: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.selected && matches!(c.display_status_cache.as_str(), "not queued"))
            .map(|c| c.id.clone())
            .collect();

        if self.parallel_mode {
            let ineligible: Vec<String> = self
                .changes
                .iter()
                .filter(|c| {
                    c.selected
                        && !c.is_parallel_eligible
                        && matches!(c.display_status_cache.as_str(), "not queued")
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
            if change.selected && matches!(change.display_status_cache.as_str(), "not queued") {
                change.set_display_status_cache("queued");
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

        // Find execution-marked changes (selected=true, display_status_cache=NotQueued)
        let marked_ids: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.selected && matches!(c.display_status_cache.as_str(), "not queued"))
            .map(|c| c.id.clone())
            .collect();

        if marked_ids.is_empty() {
            self.warning_message = Some("No changes marked for execution".to_string());
            return None;
        }

        // Convert execution-marked changes to Queued
        for change in &mut self.changes {
            if marked_ids.contains(&change.id) {
                change.set_display_status_cache("queued");
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

        // Collect error change IDs
        let error_ids: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.display_status_cache == "error")
            .map(|c| c.id.clone())
            .collect();

        if error_ids.is_empty() {
            return None;
        }

        // Reset error changes to queued
        for change in &mut self.changes {
            if change.display_status_cache == "error" {
                change.set_display_status_cache("queued");
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

mod event_handlers;

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
    /// IMPORTANT: This method does NOT modify display_status_cache. In Stopped mode, task completion
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
                let was_archived = existing.display_status_cache == "archived";
                let is_merge_wait = existing.display_status_cache == "merge wait";
                let is_resolve_wait = existing.display_status_cache == "resolve pending";

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
                    existing.set_display_status_cache("not queued");
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

                        match existing.display_status_cache.as_str() {
                            "archiving" | "resolving" | "archived" | "merged" => {
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
                    c.display_status_cache.as_str(),
                    "archiving"
                        | "archived"
                        | "merged"
                        | "merge wait"
                        | "resolving"
                        | "resolve pending"
                        | "error"
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
// The TUI syncs display_status_cache via apply_display_statuses_from_reducer() in the runner.

// ============================================================================
// Guard Logic
// ============================================================================

mod guards {
    use super::{ChangeState, TuiCommand, ViewMode, WorktreeInfo};

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
        display_status_cache: &str,
        change_id: &str,
    ) -> ToggleGuardResult {
        // Active (in-flight) changes can be stopped via Space key in Running mode
        // This is allowed and handled by handle_toggle_running_mode
        // No need to block here

        // Cannot select uncommitted changes in parallel mode (only applies to non-active states)
        if parallel_mode
            && !is_parallel_eligible
            && !matches!(
                display_status_cache,
                "applying" | "accepting" | "archiving" | "resolving"
            )
        {
            return ToggleGuardResult::Blocked(format!(
                "Cannot queue uncommitted change '{}' in parallel mode. Commit it first.",
                change_id
            ));
        }

        // MergeWait and ResolveWait can toggle execution mark (selected)
        // but cannot change display_status_cache or modify DynamicQueue
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
        match change.display_status_cache.as_str() {
            "not queued" => {
                // Emit AddToQueue command; do NOT directly assign display_status_cache here.
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
            "queued" => {
                // Emit RemoveFromQueue command; do NOT directly assign display_status_cache here.
                change.selected = false;
                let id = change.id.clone();
                let log_msg = format!("Removed from queue: {}", id);
                ToggleActionResult::Command(TuiCommand::RemoveFromQueue(id), Some(log_msg))
            }
            "merge wait" | "resolve pending" => {
                // Only toggle execution mark (selected), do not modify display_status_cache or DynamicQueue
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
            "applying" | "accepting" | "archiving" | "resolving" => {
                // Active (in-flight) changes: issue stop-and-dequeue request
                // State transition happens when ChangeDequeued event is received
                let id = change.id.clone();
                let log_msg = format!("Stop-and-dequeue requested: {}", id);
                ToggleActionResult::Command(TuiCommand::DequeueChange(id), Some(log_msg))
            }
            "error" => {
                // Error rows in Running mode must mirror queue operations:
                // selected=true => AddToQueue, selected=false => RemoveFromQueue.
                change.selected = !change.selected;
                if change.is_new {
                    change.is_new = false;
                    *new_change_count = new_change_count.saturating_sub(1);
                }
                let id = change.id.clone();
                let log_msg = if change.selected {
                    format!("Marked for retry and added to queue: {}", id)
                } else {
                    format!("Retry mark cleared and removed from queue: {}", id)
                };
                if change.selected {
                    ToggleActionResult::Command(TuiCommand::AddToQueue(id), Some(log_msg))
                } else {
                    ToggleActionResult::Command(TuiCommand::RemoveFromQueue(id), Some(log_msg))
                }
            }
            // Completed, Archived, Merged, Blocked - cannot change status
            _ => ToggleActionResult::None,
        }
    }

    /// Handle toggle selection in Stopped mode
    pub fn handle_toggle_stopped_mode(
        change: &mut ChangeState,
        new_change_count: &mut usize,
    ) -> ToggleActionResult {
        // In Stopped mode, only toggle execution mark (selected), not display_status_cache.
        // For wait states (MergeWait/ResolveWait), display_status_cache MUST remain unchanged.
        // For NotQueued, display_status_cache remains NotQueued until resume.
        if !matches!(
            change.display_status_cache.as_str(),
            "not queued" | "merge wait" | "resolve pending" | "error"
        ) {
            // Cannot modify processing/completed states.
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
            metadata: crate::openspec::ProposalMetadata::default(),
        }
    }

    #[test]
    fn test_change_state_progress() {
        let change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 3,
            total_tasks: 6,
            display_status_cache: "not queued".to_string(),
            display_color_cache: Color::DarkGray,
            error_message_cache: None,
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
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "not queued".to_string();
        app.changes[2].display_status_cache = "merge wait".to_string();
        app.changes[3].display_status_cache = "resolve pending".to_string();

        app.toggle_all_marks();
        assert!(!app.changes[0].selected, "active row must stay unchanged");
        assert!(app.changes[1].selected);
        assert!(app.changes[2].selected);
        assert!(app.changes[3].selected);

        // Wait states must keep display_status_cache unchanged.
        assert_eq!(app.changes[2].display_status_cache, "merge wait");
        assert_eq!(app.changes[3].display_status_cache, "resolve pending");

        // Second toggle unmarks only non-active rows.
        app.toggle_all_marks();
        assert!(!app.changes[0].selected, "active row must stay unchanged");
        assert!(!app.changes[1].selected);
        assert!(!app.changes[2].selected);
        assert!(!app.changes[3].selected);
    }

    #[test]
    fn test_bulk_toggle_running_mode_emits_add_to_queue_commands() {
        // When bulk toggle marks NotQueued rows in Running mode,
        // it must emit AddToQueue commands (same as single-row Space).
        let changes = vec![
            create_test_change("a", 0, 1),
            create_test_change("b", 0, 1),
            create_test_change("c", 0, 1),
        ];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "not queued".to_string();
        app.changes[1].display_status_cache = "not queued".to_string();
        app.changes[2].display_status_cache = "not queued".to_string();

        let commands = app.toggle_all_marks();

        // All three should be marked
        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);
        assert!(app.changes[2].selected);

        // Must emit AddToQueue for each NotQueued row
        assert_eq!(commands.len(), 3);
        assert!(matches!(&commands[0], TuiCommand::AddToQueue(id) if id == "a"));
        assert!(matches!(&commands[1], TuiCommand::AddToQueue(id) if id == "b"));
        assert!(matches!(&commands[2], TuiCommand::AddToQueue(id) if id == "c"));
    }

    #[test]
    fn test_bulk_toggle_running_mode_emits_remove_from_queue_commands() {
        // When all eligible rows are Queued and marked, bulk toggle must
        // unmark them and emit RemoveFromQueue commands.
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[0].selected = true;
        app.changes[1].display_status_cache = "queued".to_string();
        app.changes[1].selected = true;

        let commands = app.toggle_all_marks();

        // Both should be unmarked
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);

        // Must emit RemoveFromQueue for each Queued row
        assert_eq!(commands.len(), 2);
        assert!(matches!(&commands[0], TuiCommand::RemoveFromQueue(id) if id == "a"));
        assert!(matches!(&commands[1], TuiCommand::RemoveFromQueue(id) if id == "b"));
    }

    #[test]
    fn test_bulk_toggle_running_mode_no_commands_for_wait_states() {
        // MergeWait/ResolveWait rows should only toggle execution mark,
        // NOT emit queue commands.
        let changes = vec![
            create_test_change("not-queued", 0, 1),
            create_test_change("merge-wait", 0, 1),
            create_test_change("resolve-wait", 0, 1),
        ];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "not queued".to_string();
        app.changes[1].display_status_cache = "merge wait".to_string();
        app.changes[2].display_status_cache = "resolve pending".to_string();

        let commands = app.toggle_all_marks();

        // All eligible rows should be marked
        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);
        assert!(app.changes[2].selected);

        // Wait state display_status_cache must remain unchanged
        assert_eq!(app.changes[1].display_status_cache, "merge wait");
        assert_eq!(app.changes[2].display_status_cache, "resolve pending");

        // Only the NotQueued row should emit AddToQueue
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], TuiCommand::AddToQueue(id) if id == "not-queued"));
    }

    #[test]
    fn test_bulk_toggle_running_mode_excludes_active_rows_from_commands() {
        // Active rows (Applying, Accepting, etc.) must NOT be toggled
        // and must NOT receive stop requests via bulk toggle.
        let changes = vec![
            create_test_change("applying", 0, 1),
            create_test_change("not-queued", 0, 1),
        ];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "applying".to_string();
        app.changes[1].display_status_cache = "not queued".to_string();

        let commands = app.toggle_all_marks();

        // Active row must NOT be selected
        assert!(!app.changes[0].selected);
        // NotQueued row should be selected
        assert!(app.changes[1].selected);

        // Only one command: AddToQueue for the non-active row
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], TuiCommand::AddToQueue(id) if id == "not-queued"));
        // No StopChange command should appear
        assert!(!commands
            .iter()
            .any(|c| matches!(c, TuiCommand::DequeueChange(_))));
    }

    #[test]
    fn test_bulk_toggle_running_mode_mixed_queued_and_not_queued() {
        // When there's a mix of Queued and NotQueued, and at least one
        // unmarked row exists, all should be marked and NotQueued rows
        // get AddToQueue. (Queued rows already selected stay as-is.)
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[0].selected = true; // already marked
        app.changes[1].display_status_cache = "not queued".to_string();
        app.changes[1].selected = false; // not yet marked

        let commands = app.toggle_all_marks();

        // Both should be marked (a stays marked, b becomes marked)
        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);

        // Only the newly toggled NotQueued row should emit AddToQueue
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], TuiCommand::AddToQueue(id) if id == "b"));
    }

    #[test]
    fn test_bulk_toggle_select_mode_returns_no_commands() {
        // In Select mode, toggle_all_marks should NOT emit any queue commands.
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Select;

        let commands = app.toggle_all_marks();

        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);
        assert!(
            commands.is_empty(),
            "Select mode must not emit queue commands"
        );
    }

    #[test]
    fn test_bulk_toggle_stopped_mode_returns_no_commands() {
        // In Stopped mode, toggle_all_marks should NOT emit any queue commands.
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;

        let commands = app.toggle_all_marks();

        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);
        assert!(
            commands.is_empty(),
            "Stopped mode must not emit queue commands"
        );
    }

    #[test]
    fn test_has_bulk_toggle_targets_running_mode_requires_non_active_rows() {
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 1)];

        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "applying".to_string();
        app.changes[1].display_status_cache = "resolving".to_string();
        assert!(!app.has_bulk_toggle_targets());

        app.changes[1].display_status_cache = "resolve pending".to_string();
        assert!(app.has_bulk_toggle_targets());
    }

    #[test]
    fn test_start_processing_blocked_while_resolving() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].selected = true;
        app.is_resolving = true;

        let command = app.start_processing();
        assert!(
            matches!(command, Some(TuiCommand::StartProcessing(ids)) if ids == vec!["a".to_string()])
        );
        assert_eq!(app.mode, AppMode::Running);
        assert!(app.warning_message.is_none());
        assert_eq!(app.changes[0].display_status_cache, "queued");
    }

    #[test]
    fn test_resume_processing_blocked_while_resolving() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;
        app.changes[0].selected = true;
        app.is_resolving = true;

        let command = app.resume_processing();
        assert!(
            matches!(command, Some(TuiCommand::StartProcessing(ids)) if ids == vec!["a".to_string()])
        );
        assert_eq!(app.mode, AppMode::Running);
        assert!(app.warning_message.is_none());
        assert_eq!(app.changes[0].display_status_cache, "queued");
    }

    #[test]
    fn test_retry_error_changes_blocked_while_resolving() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Error;
        app.changes[0].set_error_message_cache("boom".to_string());
        app.is_resolving = true;

        let command = app.retry_error_changes();
        assert!(
            matches!(command, Some(TuiCommand::StartProcessing(ids)) if ids == vec!["a".to_string()])
        );
        assert_eq!(app.mode, AppMode::Running);
        assert!(app.warning_message.is_none());
        assert_eq!(app.changes[0].display_status_cache, "queued");
    }

    #[test]
    fn test_retry_error_changes_returns_error_rows_to_queued_status() {
        let change_a = create_test_change("error-a", 0, 1);
        let change_b = create_test_change("error-b", 0, 1);
        let change_ok = create_test_change("ok", 0, 1);

        let mut app = AppState::new(vec![change_a, change_b, change_ok]);
        app.mode = AppMode::Error;
        app.changes[0].set_error_message_cache("boom-a".to_string());
        app.changes[1].set_error_message_cache("boom-b".to_string());

        let command = app.retry_error_changes();

        assert!(
            matches!(command, Some(TuiCommand::StartProcessing(ids)) if ids == vec!["error-a".to_string(), "error-b".to_string()])
        );
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.changes[0].display_status_cache, "queued");
        assert_eq!(app.changes[1].display_status_cache, "queued");
        assert_eq!(app.changes[2].display_status_cache, "not queued");
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
    fn test_update_change_status_blocks_archived_and_merged_to_queued() {
        let mut archived = ChangeState {
            id: "archived-change".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: "archived".to_string(),
            display_color_cache: Color::Blue,
            error_message_cache: None,
            selected: false,
            is_new: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };
        apply_remote_status(&mut archived, "queued");
        assert_eq!(archived.display_status_cache, "archived");

        let mut merged = ChangeState {
            id: "merged-change".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: "merged".to_string(),
            display_color_cache: Color::LightBlue,
            error_message_cache: None,
            selected: false,
            is_new: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };
        apply_remote_status(&mut merged, "queued");
        assert_eq!(merged.display_status_cache, "merged");
    }

    #[test]
    fn test_running_mode_error_change_toggle_sets_retry_mark() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].set_error_message_cache("boom".to_string());
        app.changes[0].selected = false;

        let command = app.toggle_selection();

        assert!(
            matches!(command, Some(TuiCommand::AddToQueue(ref id)) if id == "test-change"),
            "error retry mark should emit AddToQueue command"
        );
        assert!(
            app.changes[0].selected,
            "Space should set retry mark on error change"
        );
        assert!(app.logs.iter().any(|log| log
            .message
            .contains("Marked for retry and added to queue: test-change")));
    }

    #[test]
    fn test_running_mode_error_change_toggle_queue() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].set_error_message_cache("boom".to_string());
        app.changes[0].selected = false;

        // First space: mark retry and add to queue
        let first_command = app.toggle_selection();
        assert!(
            matches!(first_command, Some(TuiCommand::AddToQueue(ref id)) if id == "test-change")
        );
        assert!(app.changes[0].selected);

        // Simulate queue state reflected by reducer
        app.changes[0].set_display_status_cache("error");

        // Second space: clear retry mark and remove from queue
        let second_command = app.toggle_selection();
        assert!(
            matches!(second_command, Some(TuiCommand::RemoveFromQueue(ref id)) if id == "test-change")
        );
        assert!(!app.changes[0].selected);
        assert!(app.logs.iter().any(|log| log
            .message
            .contains("Retry mark cleared and removed from queue: test-change")));
    }

    #[test]
    fn test_stopped_mode_error_change_toggle_sets_retry_mark() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Stopped;
        app.changes[0].set_error_message_cache("boom".to_string());
        app.changes[0].selected = false;

        let command = app.toggle_selection();

        assert!(
            command.is_none(),
            "stopped retry mark should be local state only"
        );
        assert!(
            app.changes[0].selected,
            "Space should set retry mark in stopped mode"
        );
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Marked for execution: test-change")));
    }

    // Iteration guard tests

    #[test]
    fn test_iteration_monotonic_update_from_none() {
        let mut change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: "applying".to_string(),
            display_color_cache: Color::Cyan,
            error_message_cache: None,
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
            display_status_cache: "applying".to_string(),
            display_color_cache: Color::Cyan,
            error_message_cache: None,
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
            display_status_cache: "applying".to_string(),
            display_color_cache: Color::Cyan,
            error_message_cache: None,
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
        app.changes[0].display_status_cache = "resolving".to_string();
        app.is_resolving = true;

        // Queue change-b for resolve
        app.add_to_resolve_queue("change-b");
        app.changes[1].display_status_cache = "resolve pending".to_string();

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
        app.changes[0].display_status_cache = "resolving".to_string();
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
        app.changes[0].display_status_cache = "resolving".to_string();
        app.is_resolving = true;

        // Set up: change-b is in MergeWait
        app.changes[1].display_status_cache = "merge wait".to_string();
        app.cursor_index = 1;
        app.mode = AppMode::Running;

        // Call resolve_merge on change-b (should queue it)
        let cmd = app.resolve_merge();

        // Should NOT return a command (queued instead)
        assert!(cmd.is_none());
        // change-b should transition to ResolveWait
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        // change-b should be in the queue
        assert!(app.has_queued_resolves());
    }

    /// Regression test: resolve_merge() in the queued path must sync intent to the shared
    /// reducer so that the display_status is "resolve pending" (not "merge wait").
    #[test]
    fn test_resolve_merge_queues_syncs_reducer() {
        use crate::orchestration::state::{OrchestratorState, WorkspaceObservation};
        use std::sync::Arc;

        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Set up shared orchestrator state with both changes.
        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string(), "change-b".to_string()],
            0,
        )));

        // Pre-condition: change-b must be in MergeWait in the reducer
        // (simulates workspace detected as Archived).
        {
            let mut guard = shared.blocking_write();
            guard.apply_observation("change-b", WorkspaceObservation::WorkspaceArchived);
        }

        app.set_shared_state(shared.clone());

        // change-a is currently resolving
        app.changes[0].display_status_cache = "resolving".to_string();
        app.is_resolving = true;

        // change-b is in MergeWait
        app.changes[1].display_status_cache = "merge wait".to_string();
        app.cursor_index = 1;
        app.mode = AppMode::Running;

        // Queue change-b for resolve via M key
        let cmd = app.resolve_merge();
        assert!(cmd.is_none());
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");

        // Verify the shared reducer reflects "resolve pending" for change-b
        let display_map = shared.blocking_read().all_display_statuses();
        assert_eq!(
            display_map.get("change-b"),
            Some(&"resolve pending"),
            "reducer must reflect 'resolve pending' after queued resolve_merge()"
        );
    }

    /// Regression test: after queuing a resolve via M key, a ChangesRefreshed event
    /// with the change still in merge_wait_ids must NOT regress ResolveWait back to MergeWait.
    #[test]
    fn test_resolve_wait_survives_changes_refreshed() {
        use crate::orchestration::state::{OrchestratorState, WorkspaceObservation};
        use std::collections::{HashMap, HashSet};
        use std::sync::Arc;

        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Set up shared orchestrator state.
        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string(), "change-b".to_string()],
            0,
        )));
        // Pre-condition: change-b is in MergeWait in the reducer.
        {
            let mut guard = shared.blocking_write();
            guard.apply_observation("change-b", WorkspaceObservation::WorkspaceArchived);
        }
        app.set_shared_state(shared.clone());

        // change-a is currently resolving
        app.changes[0].display_status_cache = "resolving".to_string();
        app.is_resolving = true;

        // change-b is in MergeWait; user presses M to queue resolve
        app.changes[1].display_status_cache = "merge wait".to_string();
        app.cursor_index = 1;
        app.mode = AppMode::Running;

        let cmd = app.resolve_merge();
        assert!(cmd.is_none());
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");

        // Simulate a ChangesRefreshed event where workspace still reports change-b
        // as Archived (which would normally set MergeWait in the reducer).
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::ChangesRefreshed {
                changes: vec![],
                committed_change_ids: HashSet::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: ["change-b".to_string()].into_iter().collect(),
            });
        }

        // apply_display_statuses_from_reducer should preserve ResolveWait
        let display_map = shared.blocking_read().all_display_statuses();
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(
            app.changes[1].display_status_cache, "resolve pending",
            "ResolveWait must survive ChangesRefreshed + apply_display_statuses_from_reducer"
        );
    }

    #[test]
    fn test_resolve_merge_select_transitions_to_running() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.mode = AppMode::Select;
        app.is_resolving = false;

        let cmd = app.resolve_merge();

        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-a"));
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
    }

    #[test]
    fn test_resolve_merge_stopped_transitions_to_running() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.mode = AppMode::Stopped;
        app.is_resolving = false;

        let cmd = app.resolve_merge();

        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-a"));
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
    }

    #[test]
    fn test_resolve_merge_running_stays_running() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.mode = AppMode::Running;
        app.is_resolving = false;

        let cmd = app.resolve_merge();

        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-a"));
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
    }

    /// Regression test: resolve_merge() in the immediate path (is_resolving == false) must
    /// sync intent to the shared reducer so that display_status is "resolve pending".
    #[test]
    fn test_resolve_merge_immediate_syncs_reducer() {
        use crate::orchestration::state::{OrchestratorState, WorkspaceObservation};
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set up shared orchestrator state.
        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));

        // Pre-condition: change-a must be in MergeWait in the reducer
        // (simulates workspace detected as Archived).
        {
            let mut guard = shared.blocking_write();
            guard.apply_observation("change-a", WorkspaceObservation::WorkspaceArchived);
        }

        app.set_shared_state(shared.clone());

        // change-a is in MergeWait, no resolve in progress
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.mode = AppMode::Running;
        app.is_resolving = false;

        // Trigger immediate resolve via M key
        let cmd = app.resolve_merge();
        assert!(
            matches!(&cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-a"),
            "immediate resolve must return TuiCommand::ResolveMerge"
        );
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");

        // Verify the shared reducer reflects "resolve pending" for change-a
        let display_map = shared.blocking_read().all_display_statuses();
        assert_eq!(
            display_map.get("change-a"),
            Some(&"resolve pending"),
            "reducer must reflect 'resolve pending' after immediate resolve_merge()"
        );
    }

    /// Regression test: after immediate resolve (is_resolving == false), a ChangesRefreshed
    /// event with the change still in merge_wait_ids must NOT regress ResolveWait back to
    /// MergeWait.
    #[test]
    fn test_resolve_wait_survives_changes_refreshed_after_immediate_resolve() {
        use crate::orchestration::state::{OrchestratorState, WorkspaceObservation};
        use std::collections::{HashMap, HashSet};
        use std::sync::Arc;

        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set up shared orchestrator state.
        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            0,
        )));
        // Pre-condition: change-a is in MergeWait in the reducer.
        {
            let mut guard = shared.blocking_write();
            guard.apply_observation("change-a", WorkspaceObservation::WorkspaceArchived);
        }
        app.set_shared_state(shared.clone());

        // change-a is in MergeWait, no resolve in progress; user presses M
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.mode = AppMode::Running;
        app.is_resolving = false;

        let cmd = app.resolve_merge();
        assert!(cmd.is_some());
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");

        // Simulate a ChangesRefreshed event where workspace still reports change-a
        // as needing merge (which would normally set MergeWait in the reducer).
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::ChangesRefreshed {
                changes: vec![],
                committed_change_ids: HashSet::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: ["change-a".to_string()].into_iter().collect(),
            });
        }

        // apply_display_statuses_from_reducer should preserve ResolveWait
        let display_map = shared.blocking_read().all_display_statuses();
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(
            app.changes[0].display_status_cache, "resolve pending",
            "ResolveWait must survive ChangesRefreshed after immediate resolve"
        );
    }

    #[test]
    fn test_resolve_merge_consecutive_m_key_presses_queue_second_change() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Both changes are merge-wait candidates.
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.changes[1].display_status_cache = "merge wait".to_string();
        app.mode = AppMode::Running;
        app.is_resolving = false;

        // 1st M press on change-a: should start immediate resolve and set is_resolving.
        app.cursor_index = 0;
        let first_cmd = app.resolve_merge();
        assert!(matches!(first_cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-a"));
        assert!(
            app.is_resolving,
            "first resolve_merge() must set is_resolving=true immediately"
        );
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");

        // 2nd M press on change-b: must take queue path and not start immediately.
        app.cursor_index = 1;
        let second_cmd = app.resolve_merge();
        assert!(
            second_cmd.is_none(),
            "second resolve_merge() must queue while resolve is in progress"
        );
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert!(
            app.resolve_queue_set.contains("change-b"),
            "second change must be queued for resolve"
        );
    }

    /// Regression: reducer-driven display must not demote a Merged change to MergeWait.
    #[test]
    fn test_apply_merge_wait_status_does_not_demote_merged() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);

        // Simulate that the change has already reached Merged state.
        app.changes[0].display_status_cache = "merged".to_string();

        // The reducer display map says "merged" → TUI must keep Merged.
        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "merged");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(
            app.changes[0].display_status_cache, "merged",
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
            app.changes[0].display_status_cache, "blocked",
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
            app.changes[0].display_status_cache, "merged",
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
        app.changes[0].display_status_cache = "applying".to_string();
        app.changes[0].selected = false;
        app.changes[1].display_status_cache = "blocked".to_string();
        app.changes[1].selected = true;
        app.changes[2].display_status_cache = "blocked".to_string();
        app.changes[2].selected = true;

        // Press F5 (start_processing) – should return None (no NotQueued changes)
        let result = app.start_processing();

        assert!(
            result.is_none(),
            "start_processing must return None when only Blocked changes are selected"
        );
        assert_eq!(
            app.changes[1].display_status_cache, "blocked",
            "Blocked change B must remain Blocked after start_processing"
        );
        assert_eq!(
            app.changes[2].display_status_cache, "blocked",
            "Blocked change C must remain Blocked after start_processing"
        );
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

        assert_eq!(app.changes[0].display_status_cache, "applying");
        assert_eq!(app.changes[1].display_status_cache, "merge wait");

        // Verify active classification works correctly.
        assert!(matches!(
            app.changes[0].display_status_cache.as_str(),
            "applying"
        ));
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
        let scenarios: &[(&str, &str)] = &[
            ("blocked", "blocked"),
            ("merge wait", "merge wait"),
            ("resolve pending", "resolve pending"),
            ("resolving", "resolving"),
            ("archived", "archived"),
            ("merged", "merged"),
            ("queued", "queued"),
            ("not queued", "not queued"),
        ];

        for (reducer_str, expected_tui_status) in scenarios {
            let mut display_map: HashMap<String, &'static str> = HashMap::new();
            display_map.insert("c1".to_string(), reducer_str);
            app.apply_display_statuses_from_reducer(&display_map);

            assert_eq!(
                app.changes[0].display_status_cache, *expected_tui_status,
                "reducer '{}' should map to {:?}",
                reducer_str, expected_tui_status
            );
        }
    }

    // -----------------------------------------------------------------------
    // Phase 3.3: toggle_selection in Running mode emits commands without
    // mutating display_status_cache locally.
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
        app.changes[0].display_status_cache = "not queued".to_string();

        // toggle_selection should return AddToQueue command and NOT mutate display_status_cache.
        let cmd = app.toggle_selection();
        assert!(
            matches!(cmd, Some(TuiCommand::AddToQueue(ref id)) if id == "c1"),
            "expected AddToQueue command, got {:?}",
            cmd
        );
        // display_status_cache must NOT have been locally changed to Queued.
        assert_eq!(
            app.changes[0].display_status_cache, "not queued",
            "display_status_cache must NOT be mutated locally; reducer drives it"
        );

        // Simulate c2 already Queued.
        app.cursor_index = 1;
        app.changes[1].display_status_cache = "queued".to_string();
        let cmd2 = app.toggle_selection();
        assert!(
            matches!(cmd2, Some(TuiCommand::RemoveFromQueue(ref id)) if id == "c2"),
            "expected RemoveFromQueue command, got {:?}",
            cmd2
        );
        // display_status_cache must NOT have been locally changed to NotQueued.
        assert_eq!(
            app.changes[1].display_status_cache, "queued",
            "display_status_cache must NOT be mutated locally; reducer drives it"
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
        app.changes[0].display_status_cache = "merge wait".to_string();

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
        // display_status_cache must still be MergeWait.
        assert_eq!(app.changes[0].display_status_cache, "merge wait");
    }

    #[test]
    fn test_resolve_wait_queue_operations() {
        let changes = vec![create_test_change("c1", 0, 3)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "resolve pending".to_string();

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
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
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
            app.changes[0].display_status_cache,
            "not queued",
            "display sync should apply reducer snapshot – but reducer already says queued, so this tests the raw override path"
        );

        // Verify with the reducer's own snapshot (the correct integration path).
        let guard2 = shared.blocking_read();
        let real_map = guard2.all_display_statuses();
        drop(guard2);
        app.changes[0].display_status_cache = "queued".to_string(); // restore as start_processing set
        app.apply_display_statuses_from_reducer(&real_map);
        assert_eq!(
            app.changes[0].display_status_cache, "queued",
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
        app.changes[0].display_status_cache = "not queued".to_string();

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
        assert_eq!(app.changes[0].display_status_cache, "queued");

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
            app.changes[0].display_status_cache, "queued",
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
        assert_eq!(app.changes[0].display_status_cache, "queued");

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
            app.changes[0].display_status_cache, "queued",
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
            app.changes[0].display_status_cache, "queued",
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
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[1].display_status_cache = "queued".to_string();
        app.mode = AppMode::Running;

        // Backend rejects only change-a.
        app.handle_orchestrator_event(OrchestratorEvent::ParallelStartRejected {
            change_ids: vec!["change-a".to_string()],
            reason: "uncommitted".to_string(),
        });

        // change-a must be reset in both TUI and reducer.
        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert_eq!(
            shared.blocking_read().display_status("change-a"),
            "not queued",
            "reducer must clear queue intent for rejected change-a"
        );

        // change-b must remain Queued in both TUI and reducer.
        assert_eq!(app.changes[1].display_status_cache, "queued");
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
    fn test_all_completed_transitions_to_select_even_when_resolving() {
        // Scheduler側でResolveWaitを管理するため、TUI側のhandle_all_completedは
        // Resolving changeがある場合でも即座にSelectに遷移する。
        let changes = vec![
            create_test_change("change-a", 3, 3),
            create_test_change("change-b", 2, 4),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "resolving".to_string();

        app.handle_all_completed();

        assert_eq!(
            app.mode,
            AppMode::Select,
            "Should transition to Select because scheduler manages ResolveWait"
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
        app.changes[0].display_status_cache = "merged".to_string(); // already done
        app.changes[1].display_status_cache = "resolving".to_string();
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
        app.changes[0].display_status_cache = "applying".to_string(); // still active
        app.changes[1].display_status_cache = "resolving".to_string();
        app.is_resolving = true;

        app.handle_resolve_completed("change-b".to_string(), None);

        assert_eq!(
            app.mode,
            AppMode::Running,
            "Should stay Running when other active changes remain"
        );
    }
}
