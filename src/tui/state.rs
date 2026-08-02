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
use crate::parallel::dedup::{DiagnosticDeduplicationKey, DiagnosticDeduplicationStore};
use crate::tui::config::TuiConfig;
use crate::tui::events::{LogEntry, LogLevel, TuiCommand};
use crate::tui::types::{AppMode, StopMode, ViewMode, WorktreeAction, WorktreeInfo};
use ratatui::style::Color;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};

use std::path::PathBuf;

use std::time::{Duration, Instant};
use tracing::{error, info, warn};

mod log_logic;
mod processing_logic;
mod selection_logic;
mod worktree_action_logic;
mod worktree_logic;

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

impl WarningPopup {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }
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
    /// Reducer-derived blocker kind for a `blocked` row.
    ///
    /// Cached from [`crate::orchestration::state::OrchestratorState::all_blocker_views`]
    /// rather than re-derived here, so the TUI can distinguish a dependency wait
    /// from an external prerequisite wait without inferring anything itself.
    pub blocker_kind_cache: crate::orchestration::state::BlockerKind,
    /// Reducer-derived operator-facing blocker detail for a blocked/stalled row.
    pub blocker_detail_cache: Option<String>,
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
    /// Scroll offset for the warning popup body (presentation-only state)
    pub warning_popup_scroll: u16,
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
    /// Map of change_id to worktree path for active worktrees (for progress fallback)
    pub worktree_paths: HashMap<String, PathBuf>,
    /// Process-local UI markers for manually requested worktree deletions.
    ///
    /// This is transient observability/input-suppression state only. It must not be
    /// persisted or used by orchestration reducers/schedulers as workflow-control input.
    deleting_worktree_paths: HashSet<PathBuf>,
    /// Reference to shared orchestration state (for unified state tracking)
    /// TUI can query this for pending/archived status, apply counts, etc.
    pub shared_orchestrator_state:
        Option<std::sync::Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>>,
    /// Shared single-resolver reservation ledger.
    ///
    /// The TUI does not own resolve ordering: the same process-local ledger backs
    /// `/api/v2`, so an `M` keypress and a remote `resolve_merge` compete for one
    /// active resolver and share one FIFO queue.
    resolve_reservations: std::sync::Arc<crate::orchestration::run_control::ResolveReservations>,
    /// Whether the log panel is visible in Changes view
    pub logs_panel_enabled: bool,
    /// Whether the Logs panel is limited to the proposal under the Changes cursor.
    ///
    /// Presentation-only, non-persistent TUI state. It never mutates
    /// `AppState::logs` and must not be used as scheduler dispatch, resume
    /// routing, acceptance, or archive input.
    pub selected_proposal_log_filter: bool,
    /// Client-local TUI preferences such as keybindings.
    ///
    /// This is presentation/input mapping state only and must not be used for
    /// resume routing, acceptance gating, archive routing, or scheduling.
    pub tui_config: TuiConfig,
    /// Latest reducer-derived display status snapshot observed by the TUI.
    ///
    /// This is presentation-only state used to order display synchronization evidence:
    /// reducer-owned lifecycle intent is stronger than refresh-derived display hints.
    /// It must not be used as scheduler dispatch, resume routing, or workflow-control input.
    reducer_display_status_snapshot: HashMap<String, &'static str>,
    /// Runtime-only observability dedupe for TUI diagnostics.
    ///
    /// This state is not workflow-control state.
    diagnostic_dedup: DiagnosticDeduplicationStore<DiagnosticDeduplicationKey>,
    /// Process-local execution marks shared with the operator command service.
    ///
    /// `ChangeState::selected` stays the rendering projection; this store is the
    /// frontend-independent copy other adapters read. It is never persisted, so a
    /// restarted process starts with every mark `false`.
    execution_marks: std::sync::Arc<crate::orchestration::operator_command::ExecutionMarkStore>,
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
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
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
            "stalled" => Color::LightYellow,
            "applying" => Color::Cyan,
            "accepting" => Color::LightGreen,
            "archiving" => Color::Magenta,
            "merge wait" => Color::LightMagenta,
            "resolve pending" => Color::Magenta,
            "resolving" => Color::LightCyan,
            "archived" => Color::Blue,
            "merged" => Color::LightBlue,
            "rejected" => Color::LightRed,
            "error" => Color::Red,
            _ => Color::DarkGray,
        };
        if status != "error" {
            self.error_message_cache = None;
        }
        // Leaving a blocked/stalled row drops its blocker view, so a stale
        // badge can never outlive the hold that produced it.
        if !matches!(status, "blocked" | "stalled") {
            self.blocker_kind_cache = crate::orchestration::state::BlockerKind::None;
            self.blocker_detail_cache = None;
        }
    }

    /// Status text for the row badge.
    ///
    /// A blocked row appends its reducer-derived blocker kind so an operator can
    /// tell a dependency wait from an external prerequisite wait at a glance;
    /// every other status keeps its plain word.
    pub fn status_badge(&self) -> String {
        match (
            self.display_status_cache.as_str(),
            self.blocker_kind_cache.as_str(),
        ) {
            ("blocked", Some(kind)) => format!("blocked:{kind}"),
            (status, _) => status.to_string(),
        }
    }

    /// Adopt a reducer-derived blocker view verbatim.
    pub fn set_blocker_view(&mut self, view: Option<&crate::orchestration::state::BlockerView>) {
        match view {
            Some(view) => {
                self.blocker_kind_cache = view.kind;
                self.blocker_detail_cache = view.detail.clone();
            }
            None => {
                self.blocker_kind_cache = crate::orchestration::state::BlockerKind::None;
                self.blocker_detail_cache = None;
            }
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
            warning_popup_scroll: 0,
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
            worktree_paths: HashMap::new(),
            deleting_worktree_paths: HashSet::new(),
            shared_orchestrator_state: None,
            resolve_reservations: std::sync::Arc::new(
                crate::orchestration::run_control::ResolveReservations::new(),
            ),
            logs_panel_enabled: true, // Default: logs panel visible
            selected_proposal_log_filter: false, // Default: show logs for every proposal
            tui_config: TuiConfig::default(),
            reducer_display_status_snapshot: HashMap::new(),
            diagnostic_dedup: DiagnosticDeduplicationStore::new(),
            execution_marks: std::sync::Arc::new(
                crate::orchestration::operator_command::ExecutionMarkStore::new(),
            ),
        }
    }

    /// Shared handle to the process-local execution marks.
    ///
    /// The operator command service and any future frontend adapter read marks
    /// through this handle instead of reaching into TUI rendering state.
    pub fn execution_marks(
        &self,
    ) -> std::sync::Arc<crate::orchestration::operator_command::ExecutionMarkStore> {
        self.execution_marks.clone()
    }

    /// Adopt an externally owned execution-mark store.
    ///
    /// Production wires this the other way round — the run-control service is
    /// built over `execution_marks()` — but a test that builds the service first
    /// needs the app to join the store the service already reads, or the
    /// "authoritative marked target set" would exist in two copies.
    pub fn set_execution_marks(
        &mut self,
        marks: std::sync::Arc<crate::orchestration::operator_command::ExecutionMarkStore>,
    ) {
        self.execution_marks = marks;
    }

    /// Shared handle to the process-local resolve reservation ledger.
    ///
    /// The run-control service takes the same handle, which is what makes the
    /// single-resolver rule hold across the TUI and `/api/v2` instead of once per
    /// frontend.
    pub fn resolve_reservations(
        &self,
    ) -> std::sync::Arc<crate::orchestration::run_control::ResolveReservations> {
        self.resolve_reservations.clone()
    }

    /// Adopt an externally owned reservation ledger.
    pub fn set_resolve_reservations(
        &mut self,
        reservations: std::sync::Arc<crate::orchestration::run_control::ResolveReservations>,
    ) {
        self.resolve_reservations = reservations;
    }

    /// True when a merge resolution currently owns the single resolver slot.
    pub fn is_resolving(&self) -> bool {
        self.resolve_reservations.is_active()
    }

    /// Record that `change_id` owns the resolver (driven by `ResolveStarted`).
    pub fn set_resolving(&self, change_id: &str) {
        self.resolve_reservations.mark_active(change_id);
    }

    /// Clear the active resolver without promoting a waiting change.
    pub fn clear_resolving(&self) {
        if let Some(active) = self.resolve_reservations.active() {
            self.resolve_reservations.cancel(&active);
        }
    }

    /// Publish the current TUI mark projection into the shared store.
    ///
    /// Called after every operator interaction that can change marks so other
    /// frontends observe the same process-local intent.
    pub fn publish_execution_marks(&self) {
        self.execution_marks.replace(
            self.changes
                .iter()
                .filter(|change| change.selected)
                .map(|change| change.id.clone()),
        );
    }

    pub fn set_tui_config(&mut self, tui_config: TuiConfig) {
        self.tui_config = tui_config;
    }

    pub fn start_key_label(&self) -> String {
        self.tui_config.start_key_label()
    }

    /// Show a warning popup and reset popup-local presentation state.
    pub fn show_warning_popup(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.warning_popup = Some(WarningPopup::new(title, message));
        self.warning_popup_scroll = 0;
    }

    /// Clear any warning popup and reset popup-local presentation state.
    pub fn clear_warning_popup(&mut self) {
        self.warning_popup = None;
        self.warning_popup_scroll = 0;
    }

    /// Scroll the warning popup body by a signed amount while keeping the offset non-negative.
    pub fn scroll_warning_popup(&mut self, delta: i16) {
        if delta.is_negative() {
            self.warning_popup_scroll = self
                .warning_popup_scroll
                .saturating_sub(delta.unsigned_abs());
        } else {
            self.warning_popup_scroll = self.warning_popup_scroll.saturating_add(delta as u16);
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
        let previous_filter_target = self.captured_selected_proposal_log_filter_target();
        self.cursor_index = if self.cursor_index == 0 {
            self.changes.len() - 1
        } else {
            self.cursor_index - 1
        };
        self.list_state.select(Some(self.cursor_index));
        self.sync_selected_proposal_log_filter_after_cursor_move(previous_filter_target.as_deref());
    }

    /// Move cursor down
    pub fn cursor_down(&mut self) {
        if self.changes.is_empty() {
            return;
        }
        let previous_filter_target = self.captured_selected_proposal_log_filter_target();
        self.cursor_index = (self.cursor_index + 1) % self.changes.len();
        self.list_state.select(Some(self.cursor_index));
        self.sync_selected_proposal_log_filter_after_cursor_move(previous_filter_target.as_deref());
    }

    /// Move worktree cursor up
    pub fn worktree_cursor_up(&mut self) {
        let Some(next_index) = worktree_logic::previous_worktree_cursor_index(
            self.worktree_cursor_index,
            self.worktrees.len(),
        ) else {
            return;
        };

        self.worktree_cursor_index = next_index;
        self.worktree_list_state
            .select(Some(self.worktree_cursor_index));
    }

    /// Move worktree cursor down
    pub fn worktree_cursor_down(&mut self) {
        let Some(next_index) = worktree_logic::next_worktree_cursor_index(
            self.worktree_cursor_index,
            self.worktrees.len(),
        ) else {
            return;
        };

        self.worktree_cursor_index = next_index;
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

    /// Mark a worktree as having an accepted delete request in progress.
    pub fn mark_worktree_deleting(&mut self, path: impl Into<PathBuf>) {
        self.deleting_worktree_paths.insert(path.into());
    }

    /// Clear the transient delete-in-progress marker for a worktree.
    pub fn clear_worktree_deleting(&mut self, path: &PathBuf) {
        self.deleting_worktree_paths.remove(path);
    }

    /// Return whether a worktree path is currently marked deleting.
    pub fn is_worktree_deleting(&self, path: &PathBuf) -> bool {
        self.deleting_worktree_paths.contains(path)
    }

    /// Return a display label for the first visible deleting worktree, if any.
    pub fn deleting_worktree_status_label(&self) -> Option<String> {
        self.worktrees
            .iter()
            .find(|worktree| self.is_worktree_deleting(&worktree.path))
            .map(|worktree| worktree.display_label())
            .or_else(|| {
                self.deleting_worktree_paths
                    .iter()
                    .next()
                    .map(|path| path.display().to_string())
            })
    }

    fn selected_worktree_is_deleting(&self) -> bool {
        self.get_selected_worktree()
            .is_some_and(|worktree| self.is_worktree_deleting(&worktree.path))
    }

    fn block_selected_deleting_worktree_action(&mut self) -> bool {
        if self.selected_worktree_is_deleting() {
            self.warning_message = Some("Worktree is already being deleted".to_string());
            true
        } else {
            false
        }
    }

    /// Return true if the selected worktree is deleting and the caller should suppress its action.
    pub fn suppress_if_selected_worktree_deleting(&mut self) -> bool {
        self.block_selected_deleting_worktree_action()
    }

    /// Request worktree delete with validation
    ///
    /// Returns Some(TuiCommand) if deletion should proceed, None if it should be blocked
    pub fn request_worktree_delete_from_list(&mut self) -> Option<TuiCommand> {
        if self.block_selected_deleting_worktree_action() {
            return None;
        }

        match worktree_action_logic::validate_delete_request(
            &self.worktrees,
            self.worktree_cursor_index,
            &self.changes,
        ) {
            Ok((path, branch)) => {
                worktree_action_logic::apply_delete_confirmation_state(
                    path,
                    branch,
                    &mut self.mode,
                    &mut self.pending_worktree_action,
                    &mut self.pending_worktree_branch,
                    &mut self.previous_mode,
                );
                None
            }
            Err(msg) => {
                self.warning_message = Some(msg);
                None
            }
        }
    }

    /// Confirm and execute pending worktree action.
    pub fn confirm_worktree_action_delete(&mut self) -> Option<TuiCommand> {
        self.confirm_worktree_action_delete_with_options(false)
    }

    /// Confirm and execute pending worktree deletion with explicit teardown behavior.
    pub fn confirm_worktree_action_delete_with_options(
        &mut self,
        skip_teardown: bool,
    ) -> Option<TuiCommand> {
        if let Some((path, WorktreeAction::Delete)) = self.pending_worktree_action.take() {
            // Get the branch name that was stored when the delete was requested
            let branch_name = self.pending_worktree_branch.take();
            let path_buf = PathBuf::from(&path);
            self.mark_worktree_deleting(path_buf.clone());
            let teardown_note = if skip_teardown {
                " with skip-teardown"
            } else {
                ""
            };
            self.add_log(LogEntry::info(format!(
                "Deleting worktree{}: {}",
                teardown_note,
                path_buf.display()
            )));

            // Restore previous mode
            if let Some(mode) = self.previous_mode.take() {
                self.mode = mode;
            } else {
                self.mode = AppMode::Select;
            }

            Some(TuiCommand::DeleteWorktreeByPath(
                path_buf,
                branch_name,
                skip_teardown,
            ))
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
        worktree_action_logic::request_merge_worktree_branch(self)
    }

    /// Toggle selection of the current change
    ///
    /// In Select mode:
    /// - Changes can be toggled between selected/unselected
    ///
    /// In Running/Completed mode:
    /// - Changes can be added to or removed from the queue
    pub fn toggle_selection(&mut self) -> Option<TuiCommand> {
        selection_logic::toggle_selection(self)
    }

    fn can_bulk_toggle_change(&self, change: &ChangeState) -> bool {
        selection_logic::can_bulk_toggle_change(self.mode.clone(), self.parallel_mode, change)
    }

    /// Returns true when at least one change can be targeted by bulk toggle.
    pub fn has_bulk_toggle_targets(&self) -> bool {
        selection_logic::is_bulk_toggle_mode(&self.mode)
            && self
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
    ///
    /// Excluded rows and an empty target set are always reported so the
    /// operation never looks like it silently stopped halfway.
    pub fn toggle_all_marks(&mut self) -> Vec<TuiCommand> {
        selection_logic::toggle_all_marks(self)
    }

    /// Trigger merge resolution for the change under the cursor when applicable.
    ///
    /// This is presentation only. Whether the change becomes the active resolver
    /// or waits in FIFO order, whether the reducer still accepts the intent, and
    /// whether the scheduler is started or woken are all decided by the shared
    /// run-control service when the emitted command is handled, so an `M`
    /// keypress and a remote `resolve_merge` cannot reach different conclusions.
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
        let change = &self.changes[self.cursor_index];
        if !matches!(change.display_status_cache.as_str(), "merge wait") {
            return None;
        }

        Some(TuiCommand::ResolveMerge(change.id.clone()))
    }

    /// Add a change to the resolve queue (with duplicate prevention).
    ///
    /// Returns true if the change was added, false if it was already reserved.
    pub fn add_to_resolve_queue(&mut self, change_id: &str) -> bool {
        self.resolve_reservations.reserve(change_id).is_some()
    }

    /// Remove one specific change from the resolve queue.
    ///
    /// Used when reducer-owned retry intent for that change is gone (for example a
    /// manual `MergeDeferred(auto_resumable=false)`), so stale queue membership
    /// cannot outlive it. FIFO order of unrelated entries is preserved.
    ///
    /// Returns true if the change was queued and has been removed.
    pub fn remove_from_resolve_queue(&mut self, change_id: &str) -> bool {
        self.resolve_reservations.cancel(change_id)
    }

    /// Release the active resolver and take the next change from the queue.
    ///
    /// Returns the promoted change ID if one was waiting, otherwise None.
    pub fn pop_from_resolve_queue(&mut self) -> Option<String> {
        self.resolve_reservations.finish_active()
    }

    /// Changes currently waiting behind the active resolver, in FIFO order.
    pub fn queued_resolves(&self) -> Vec<String> {
        self.resolve_reservations.waiting()
    }

    /// Check if there are queued resolves waiting.
    pub fn has_queued_resolves(&self) -> bool {
        self.resolve_reservations.has_waiting()
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
        self.reducer_display_status_snapshot = display_map.clone();

        for change in &mut self.changes {
            if change.display_status_cache == "rejected"
                && !matches!(display_map.get(&change.id).copied(), Some("rejected"))
            {
                // rejected rows are display-only and remain immutable until marker removal.
                change.selected = false;
                continue;
            }

            if matches!(display_map.get(&change.id).copied(), Some("queued")) {
                change.selected = true;
            }

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
                    if normalized == "rejected" {
                        change.selected = false;
                    }
                }
            }
        }
    }

    /// Sync blocker views from the reducer.
    ///
    /// Kept separate from the display-status sync so a row's `blocked` versus
    /// `stalled` word and its blocker kind always come from the same reducer
    /// snapshot instead of being inferred independently here.
    pub fn apply_blocker_views_from_reducer(
        &mut self,
        blocker_views: &HashMap<String, crate::orchestration::state::BlockerView>,
    ) {
        for change in &mut self.changes {
            change.set_blocker_view(blocker_views.get(&change.id));
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
        self.reset_analysis_log_dedupe();
    }

    pub(crate) fn reset_analysis_log_dedupe(&mut self) {
        self.diagnostic_dedup.reset_matching(|key| {
            matches!(key, DiagnosticDeduplicationKey::TuiAnalysisStarted { .. })
        });
    }

    /// Project this frontend's lifecycle mode onto the shared operator vocabulary.
    ///
    /// Modal popups are presentation state, so the mode underneath them is what
    /// the shared lifecycle matrix must see; a popup must never silently widen or
    /// narrow what an operator is allowed to do.
    pub fn operator_mode(&self) -> crate::orchestration::operator_command::OperatorMode {
        use crate::orchestration::operator_command::OperatorMode;
        let effective = match &self.mode {
            AppMode::ConfirmWorktreeDelete
            | AppMode::QrPopup
            | AppMode::ConfirmForceKill { .. } => {
                self.previous_mode.clone().unwrap_or(AppMode::Select)
            }
            other => other.clone(),
        };
        match effective {
            AppMode::Running => OperatorMode::Running,
            AppMode::Stopping => OperatorMode::Stopping,
            AppMode::Stopped => OperatorMode::Stopped,
            AppMode::Error => OperatorMode::Error,
            _ => OperatorMode::Select,
        }
    }

    /// Project an accepted run dispatch onto TUI presentation state.
    ///
    /// The shared service already decided the targets and applied the reducer
    /// intent; this only refreshes the row cache and the run-scoped UI state so
    /// the screen matches what was actually dispatched.
    pub fn begin_run(&mut self, change_ids: &[String]) {
        for change in &mut self.changes {
            if change_ids.iter().any(|id| id == &change.id) {
                change.set_display_status_cache("queued");
                change.selected = true;
            }
        }
        self.reset_for_run();
        self.mode = AppMode::Running;
        self.publish_execution_marks();
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
        if log_logic::apply_log_buffer_limit(self.logs.len(), MAX_LOG_ENTRIES) {
            self.logs.remove(0);
        }

        // Auto-scroll to bottom if enabled, otherwise freeze view position
        self.log_scroll_offset = log_logic::next_log_offset_on_append(
            self.log_auto_scroll,
            self.log_scroll_offset,
            self.logs.len(),
        );
    }

    /// Scroll logs up by a page (show older entries)
    pub fn scroll_logs_up(&mut self, page_size: usize) {
        self.log_scroll_offset =
            log_logic::scroll_logs_up(self.log_scroll_offset, self.logs.len(), page_size);
        // Disable auto-scroll when user scrolls up
        self.log_auto_scroll = false;
    }

    /// Scroll logs down by a page (show newer entries)
    pub fn scroll_logs_down(&mut self, page_size: usize) {
        self.log_scroll_offset = log_logic::scroll_logs_down(self.log_scroll_offset, page_size);
        // Re-enable auto-scroll when at bottom
        if self.log_scroll_offset == 0 {
            self.log_auto_scroll = true;
        }
    }

    /// Jump to the oldest log entry (top of history)
    pub fn scroll_logs_to_top(&mut self) {
        self.log_scroll_offset = log_logic::scroll_logs_to_top(self.logs.len());
        self.log_auto_scroll = false;
    }

    /// Jump to the newest log entry (bottom) and re-enable auto-scroll
    pub fn scroll_logs_to_bottom(&mut self) {
        self.log_scroll_offset = 0;
        self.log_auto_scroll = true;
    }

    /// Toggle log panel visibility
    pub fn toggle_logs_panel(&mut self) {
        self.logs_panel_enabled = log_logic::toggle_logs_panel(self.logs_panel_enabled);
    }

    /// Proposal ID the selected-proposal log filter currently targets.
    ///
    /// Derived from the Changes cursor so no second copy of the target can drift
    /// out of sync. Returns `None` when the list is empty or the cursor is out of
    /// range.
    pub fn selected_proposal_log_filter_target(&self) -> Option<&str> {
        self.changes
            .get(self.cursor_index)
            .map(|change| change.id.as_str())
    }

    /// Toggle the presentation-only selected-proposal log filter.
    ///
    /// Filtering changes which entries are visible, so the entry-based scroll
    /// offset is no longer meaningful; return to the newest visible output with
    /// auto-scroll enabled. `AppState::logs` is never modified.
    pub fn toggle_selected_proposal_log_filter(&mut self) {
        self.selected_proposal_log_filter =
            log_logic::toggle_selected_proposal_log_filter(self.selected_proposal_log_filter);
        self.scroll_logs_to_bottom();
    }

    /// Whether a buffered entry is visible under the current filter state.
    pub fn log_entry_visible_for_selected_proposal_filter(&self, entry: &LogEntry) -> bool {
        log_logic::log_entry_matches_selected_proposal(
            self.selected_proposal_log_filter,
            entry.change_id.as_deref(),
            self.selected_proposal_log_filter_target(),
        )
    }

    /// Snapshot the filter target before a cursor move, only when it can matter.
    fn captured_selected_proposal_log_filter_target(&self) -> Option<String> {
        if self.selected_proposal_log_filter {
            self.selected_proposal_log_filter_target()
                .map(str::to_string)
        } else {
            None
        }
    }

    /// Reset the Logs panel position when a cursor move retargets an active filter.
    fn sync_selected_proposal_log_filter_after_cursor_move(
        &mut self,
        previous_target: Option<&str>,
    ) {
        if log_logic::should_reset_log_position_for_target_change(
            self.selected_proposal_log_filter,
            previous_target,
            self.selected_proposal_log_filter_target(),
        ) {
            self.scroll_logs_to_bottom();
        }
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
    fn update_changes_with_rejected(
        &mut self,
        fetched_changes: Vec<Change>,
        rejected_changes: Vec<Change>,
    ) {
        processing_logic::update_changes_with_rejected(self, fetched_changes, rejected_changes);
    }

    #[cfg(test)]
    fn update_changes_with_rejected_for_test(
        &mut self,
        fetched_changes: Vec<Change>,
        rejected_changes: Vec<Change>,
    ) {
        self.update_changes_with_rejected(fetched_changes, rejected_changes);
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
    use crate::orchestration::operator_command::{classify_mark_route, MarkRoute, OperatorMode};

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

    /// Why a change cannot be toggled for selection.
    ///
    /// Single-row toggle renders this as a warning message, while bulk toggle
    /// groups rows by reason to explain what was excluded.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ToggleBlockReason {
        /// Parallel mode cannot queue a change that is not committed yet.
        ParallelUncommitted,
        /// Rejected proposals are read-only.
        Rejected,
    }

    /// Classifies why a change is not toggleable, or `None` when it is.
    ///
    /// This is the single source of truth shared by the single-row guard and
    /// the bulk toggle classification, so both paths stay consistent.
    pub fn classify_toggle_block(
        is_parallel_eligible: bool,
        parallel_mode: bool,
        display_status_cache: &str,
    ) -> Option<ToggleBlockReason> {
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
            return Some(ToggleBlockReason::ParallelUncommitted);
        }

        if display_status_cache == "rejected" {
            return Some(ToggleBlockReason::Rejected);
        }

        // MergeWait and ResolveWait can toggle execution mark (selected)
        // but cannot change display_status_cache or modify DynamicQueue
        // This is handled by the mode-specific handlers
        None
    }

    /// Validates that a change can be toggled for selection
    pub fn validate_change_toggleable(
        is_parallel_eligible: bool,
        parallel_mode: bool,
        display_status_cache: &str,
        change_id: &str,
    ) -> ToggleGuardResult {
        match classify_toggle_block(is_parallel_eligible, parallel_mode, display_status_cache) {
            Some(ToggleBlockReason::ParallelUncommitted) => ToggleGuardResult::Blocked(format!(
                "Cannot queue uncommitted change '{}' in parallel mode. Commit it first.",
                change_id
            )),
            Some(ToggleBlockReason::Rejected) => ToggleGuardResult::Blocked(format!(
                "Change '{}' is rejected and read-only",
                change_id
            )),
            None => ToggleGuardResult::Allowed,
        }
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

    /// Clear the NEW flag once the operator has interacted with a change.
    fn consume_new_flag(change: &mut ChangeState, new_change_count: &mut usize) {
        if change.is_new {
            change.is_new = false;
            *new_change_count = new_change_count.saturating_sub(1);
        }
    }

    /// Toggle the execution mark only, without touching queue or display state.
    fn toggle_mark_only(
        change: &mut ChangeState,
        new_change_count: &mut usize,
        log: bool,
    ) -> ToggleActionResult {
        change.selected = !change.selected;
        consume_new_flag(change, new_change_count);
        if !log {
            return ToggleActionResult::StateOnly(None);
        }
        let log_msg = if change.selected {
            format!("Marked for execution: {}", change.id)
        } else {
            format!("Unmarked: {}", change.id)
        };
        ToggleActionResult::StateOnly(Some(log_msg))
    }

    /// Handle toggle selection in Select mode
    pub fn handle_toggle_select_mode(
        change: &mut ChangeState,
        new_change_count: &mut usize,
    ) -> ToggleActionResult {
        match classify_mark_route(OperatorMode::Select, &change.display_status_cache) {
            MarkRoute::MarkOnly => toggle_mark_only(change, new_change_count, false),
            _ => ToggleActionResult::None,
        }
    }

    /// Handle toggle selection in Running mode
    ///
    /// Routing comes from the shared operator command matrix; this function only
    /// projects the decision onto TUI state and log messages.
    pub fn handle_toggle_running_mode(
        change: &mut ChangeState,
        new_change_count: &mut usize,
    ) -> ToggleActionResult {
        match classify_mark_route(OperatorMode::Running, &change.display_status_cache) {
            MarkRoute::QueueIntent => {
                // Queue intent decides the new mark: unqueued rows are added,
                // queued rows are removed, and error rows mirror the same pair.
                let is_error = change.display_status_cache == "error";
                let add = match change.display_status_cache.as_str() {
                    "not queued" => true,
                    "queued" => false,
                    _ => !change.selected,
                };
                // Do NOT assign display_status_cache here: the shared service applies
                // the reducer transition and the TUI derives display status from it.
                change.selected = add;
                consume_new_flag(change, new_change_count);
                let id = change.id.clone();
                let log_msg = match (is_error, add) {
                    (true, true) => format!("Marked for retry and added to queue: {}", id),
                    (true, false) => format!("Retry mark cleared and removed from queue: {}", id),
                    (false, true) => format!("Added to queue: {}", id),
                    (false, false) => format!("Removed from queue: {}", id),
                };
                if add {
                    ToggleActionResult::Command(TuiCommand::AddToQueue(id), Some(log_msg))
                } else {
                    ToggleActionResult::Command(TuiCommand::RemoveFromQueue(id), Some(log_msg))
                }
            }
            // MergeWait/ResolveWait rows accept mark-only mutation.
            MarkRoute::MarkOnly => toggle_mark_only(change, new_change_count, true),
            // Active rows are stopped with the force-kill control, not with Space;
            // final rows and retry-owned rows cannot change here.
            MarkRoute::Immutable | MarkRoute::RetryRequired => ToggleActionResult::None,
        }
    }

    /// Handle toggle selection in Stopped mode
    ///
    /// In Stopped mode only the execution mark changes. For wait states
    /// (MergeWait/ResolveWait) and NotQueued, `display_status_cache` MUST remain
    /// unchanged until the run resumes.
    pub fn handle_toggle_stopped_mode(
        change: &mut ChangeState,
        new_change_count: &mut usize,
    ) -> ToggleActionResult {
        match classify_mark_route(OperatorMode::Stopped, &change.display_status_cache) {
            MarkRoute::MarkOnly => toggle_mark_only(change, new_change_count, true),
            _ => ToggleActionResult::None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::events::OrchestratorEvent;

    #[test]
    fn tui_config_defaults_and_configured_labels_are_available_in_app_state() {
        let mut app = AppState::new(vec![]);
        assert_eq!(app.start_key_label(), "F5/!");

        let custom = TuiConfig::parse_jsonc(
            r#"{"keybindings":{"start":["F5","!"]}}"#,
            std::path::Path::new("/tmp/tui.jsonc"),
        )
        .unwrap();
        app.set_tui_config(custom);

        assert_eq!(app.start_key_label(), "F5/!");
    }

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

    fn visible_filtered_messages(app: &AppState) -> Vec<String> {
        app.logs
            .iter()
            .filter(|entry| app.log_entry_visible_for_selected_proposal_filter(entry))
            .map(|entry| entry.message.clone())
            .collect()
    }

    fn app_with_mixed_proposal_logs() -> AppState {
        let mut app = AppState::new(vec![
            create_test_change("alpha", 0, 1),
            create_test_change("beta", 0, 1),
        ]);
        app.logs.clear();
        app.add_log(LogEntry::info("alpha apply").with_change_id("alpha"));
        app.add_log(LogEntry::info("beta apply").with_change_id("beta"));
        app.add_log(LogEntry::info("global orchestration"));
        app
    }

    #[test]
    fn selected_proposal_log_filter_defaults_off_and_shows_every_entry() {
        let app = app_with_mixed_proposal_logs();

        assert!(!app.selected_proposal_log_filter);
        assert_eq!(
            visible_filtered_messages(&app),
            vec!["alpha apply", "beta apply", "global orchestration"]
        );
    }

    #[test]
    fn selected_proposal_log_filter_shows_only_cursor_proposal_entries() {
        let mut app = app_with_mixed_proposal_logs();

        app.toggle_selected_proposal_log_filter();

        assert!(app.selected_proposal_log_filter);
        assert_eq!(app.selected_proposal_log_filter_target(), Some("alpha"));
        assert_eq!(visible_filtered_messages(&app), vec!["alpha apply"]);
    }

    #[test]
    fn selected_proposal_log_filter_follows_cursor_and_resets_to_newest() {
        let mut app = app_with_mixed_proposal_logs();
        app.toggle_selected_proposal_log_filter();
        app.log_scroll_offset = 2;
        app.log_auto_scroll = false;

        app.cursor_down();

        assert_eq!(app.selected_proposal_log_filter_target(), Some("beta"));
        assert_eq!(visible_filtered_messages(&app), vec!["beta apply"]);
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);
    }

    #[test]
    fn cursor_move_keeps_log_position_when_filter_is_off() {
        let mut app = app_with_mixed_proposal_logs();
        app.log_scroll_offset = 2;
        app.log_auto_scroll = false;

        app.cursor_down();

        assert_eq!(app.log_scroll_offset, 2);
        assert!(!app.log_auto_scroll);
    }

    #[test]
    fn toggling_selected_proposal_log_filter_never_mutates_the_log_buffer() {
        let mut app = app_with_mixed_proposal_logs();
        let before: Vec<String> = app.logs.iter().map(|e| e.message.clone()).collect();

        app.toggle_selected_proposal_log_filter();
        app.toggle_selected_proposal_log_filter();

        let after: Vec<String> = app.logs.iter().map(|e| e.message.clone()).collect();
        assert_eq!(before, after);
        assert!(!app.selected_proposal_log_filter);
        assert_eq!(
            visible_filtered_messages(&app),
            vec!["alpha apply", "beta apply", "global orchestration"]
        );
    }

    #[test]
    fn toggling_selected_proposal_log_filter_returns_to_newest_output() {
        let mut app = app_with_mixed_proposal_logs();
        app.log_scroll_offset = 2;
        app.log_auto_scroll = false;

        app.toggle_selected_proposal_log_filter();

        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);
    }

    #[test]
    fn selected_proposal_log_filter_hides_everything_without_a_cursor_proposal() {
        let mut app = AppState::new(vec![]);
        app.logs.clear();
        app.add_log(LogEntry::info("alpha apply").with_change_id("alpha"));
        app.add_log(LogEntry::info("global orchestration"));

        app.toggle_selected_proposal_log_filter();

        assert_eq!(app.selected_proposal_log_filter_target(), None);
        assert!(visible_filtered_messages(&app).is_empty());
        assert_eq!(app.logs.len(), 2);
    }

    #[test]
    fn selected_proposal_log_filter_uses_handler_attached_metadata_not_message_text() {
        let mut app = AppState::new(vec![
            create_test_change("alpha", 0, 1),
            create_test_change("beta", 0, 1),
        ]);
        app.logs.clear();

        app.handle_processing_started("alpha".to_string());
        app.handle_processing_error("beta".to_string(), "boom".to_string());
        app.handle_analysis_started(1, "attempt-a".to_string());

        app.toggle_selected_proposal_log_filter();

        // The beta error message contains the substring "beta", but only the
        // structured metadata decides visibility.
        assert_eq!(visible_filtered_messages(&app), vec!["Processing: alpha"]);
        assert_eq!(app.logs.len(), 3);
    }

    #[test]
    fn selected_proposal_log_filter_excludes_remote_project_only_entries() {
        let mut app = AppState::new(vec![create_test_change("proj-1::demo/alpha", 0, 1)]);
        app.logs.clear();
        app.add_log(LogEntry::info("project scoped output").with_change_id("proj-1"));
        app.add_log(LogEntry::info("proposal output").with_change_id("proj-1::demo/alpha"));

        app.toggle_selected_proposal_log_filter();

        assert_eq!(visible_filtered_messages(&app), vec!["proposal output"]);
    }

    fn create_test_worktree(path: &str, branch: &str, is_main: bool) -> WorktreeInfo {
        WorktreeInfo {
            path: PathBuf::from(path),
            head: "abc123".to_string(),
            branch: branch.to_string(),
            is_detached: false,
            is_main,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        }
    }

    #[test]
    fn stale_archive_started_does_not_regress_merged_row() {
        let change = create_test_change("alpha", 1, 1);
        let mut app = AppState::new(vec![change]);
        app.changes[0].set_display_status_cache("merged");

        app.handle_orchestrator_event(OrchestratorEvent::ArchiveStarted {
            change_id: "alpha".to_string(),
            command: "archive alpha".to_string(),
        });

        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert_eq!(app.changes[0].display_color_cache, Color::LightBlue);
    }

    #[test]
    fn worktree_delete_progress_marker_marks_and_clears_path() {
        let mut app = AppState::new(vec![]);
        let path = PathBuf::from("/tmp/worktree-a");

        assert!(!app.is_worktree_deleting(&path));
        app.mark_worktree_deleting(path.clone());
        assert!(app.is_worktree_deleting(&path));
        app.clear_worktree_deleting(&path);
        assert!(!app.is_worktree_deleting(&path));
    }

    #[test]
    fn worktree_delete_confirmation_marks_before_emitting_delete_command() {
        let mut app = AppState::new(vec![]);
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a", false)];
        app.request_worktree_delete_from_list();

        let command = app.confirm_worktree_action_delete();

        assert!(app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")));
        assert!(matches!(
            command,
            Some(TuiCommand::DeleteWorktreeByPath(path, Some(branch), false))
                if path.as_path() == PathBuf::from("/tmp/worktree-a").as_path() && branch == "feature-a"
        ));
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Deleting worktree: /tmp/worktree-a")));
    }

    #[test]
    fn worktree_skip_teardown_confirmation_marks_before_emitting_delete_command() {
        let mut app = AppState::new(vec![]);
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a", false)];
        app.request_worktree_delete_from_list();

        let command = app.confirm_worktree_action_delete_with_options(true);

        assert!(app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")));
        assert!(matches!(
            command,
            Some(TuiCommand::DeleteWorktreeByPath(path, Some(branch), true))
                if path.as_path() == PathBuf::from("/tmp/worktree-a").as_path() && branch == "feature-a"
        ));
        assert!(app.logs.iter().any(|entry| {
            entry
                .message
                .contains("Deleting worktree with skip-teardown: /tmp/worktree-a")
        }));
    }

    #[test]
    fn worktree_delete_request_is_suppressed_while_marker_is_active() {
        let mut app = AppState::new(vec![]);
        let path = PathBuf::from("/tmp/worktree-a");
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a", false)];
        app.mark_worktree_deleting(path);

        app.request_worktree_delete_from_list();

        assert!(app.pending_worktree_action.is_none());
        assert_eq!(
            app.warning_message.as_deref(),
            Some("Worktree is already being deleted")
        );
    }

    #[test]
    fn merge_request_is_suppressed_while_selected_worktree_is_deleting() {
        let mut app = AppState::new(vec![]);
        app.view_mode = ViewMode::Worktrees;
        let path = PathBuf::from("/tmp/worktree-a");
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a", false)];
        app.mark_worktree_deleting(path);

        let command = app.request_merge_worktree_branch();

        assert!(command.is_none());
        assert_eq!(
            app.warning_message.as_deref(),
            Some("Worktree is already being deleted")
        );
    }

    #[test]
    fn warning_popup_lifecycle_resets_scroll_offset() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.show_warning_popup("first", "message");
        app.scroll_warning_popup(3);
        assert_eq!(app.warning_popup_scroll, 3);

        app.show_warning_popup("second", "message");
        assert_eq!(app.warning_popup_scroll, 0);

        app.scroll_warning_popup(2);
        app.clear_warning_popup();
        assert_eq!(app.warning_popup_scroll, 0);
        assert!(app.warning_popup.is_none());
    }

    #[test]
    fn warning_popup_scroll_saturates_at_zero() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.show_warning_popup("warning", "message");

        app.scroll_warning_popup(-5);

        assert_eq!(app.warning_popup_scroll, 0);
    }

    #[test]
    fn test_change_state_progress() {
        let change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 3,
            total_tasks: 6,
            display_status_cache: "not queued".to_string(),
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
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
        app.set_resolving("__active__");
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

    /// Rows for a bulk toggle case: (id, display status, parallel eligible, pre-selected).
    type BulkToggleRow = (&'static str, &'static str, bool, bool);

    /// Applies one bulk toggle case and returns the resulting `selected` flags.
    fn run_bulk_toggle_case(
        mode: AppMode,
        parallel_mode: bool,
        rows: &[BulkToggleRow],
    ) -> (AppState, Vec<TuiCommand>) {
        let changes = rows
            .iter()
            .map(|(id, _, _, _)| create_test_change(id, 0, 1))
            .collect();
        let mut app = AppState::new(changes);
        app.mode = mode;
        app.parallel_mode = parallel_mode;
        app.parallel_available = parallel_mode;
        for (index, (_, status, parallel_eligible, selected)) in rows.iter().enumerate() {
            app.changes[index].display_status_cache = status.to_string();
            app.changes[index].is_parallel_eligible = *parallel_eligible;
            app.changes[index].selected = *selected;
        }

        let commands = app.toggle_all_marks();
        (app, commands)
    }

    /// One table-driven bulk toggle regression case.
    struct BulkToggleCase {
        name: &'static str,
        mode: AppMode,
        parallel_mode: bool,
        rows: Vec<BulkToggleRow>,
        /// Expected `selected` flag for each row after the toggle.
        expected: Vec<bool>,
    }

    #[test]
    fn test_toggle_all_marks_mixed_eligible_and_ineligible_leaves_no_partial_eligible_rows() {
        let cases = vec![
            BulkToggleCase {
                name: "select mode marks every eligible row alongside a rejected row",
                mode: AppMode::Select,
                parallel_mode: false,
                rows: vec![
                    ("eligible-marked", "not queued", true, true),
                    ("eligible-unmarked", "not queued", true, false),
                    ("rejected", "rejected", true, false),
                ],
                expected: vec![true, true, false],
            },
            BulkToggleCase {
                name: "select mode unmarks every eligible row when all are marked",
                mode: AppMode::Select,
                parallel_mode: false,
                rows: vec![
                    ("eligible-a", "not queued", true, true),
                    ("eligible-b", "not queued", true, true),
                    ("rejected", "rejected", true, false),
                ],
                expected: vec![false, false, false],
            },
            BulkToggleCase {
                name: "stopped mode marks every eligible row alongside a rejected row",
                mode: AppMode::Stopped,
                parallel_mode: false,
                rows: vec![
                    ("eligible-marked", "merge wait", true, true),
                    ("eligible-unmarked", "not queued", true, false),
                    ("rejected", "rejected", true, false),
                ],
                expected: vec![true, true, false],
            },
            BulkToggleCase {
                name: "running mode marks every eligible row and skips active rows",
                mode: AppMode::Running,
                parallel_mode: false,
                rows: vec![
                    ("active", "applying", true, false),
                    ("eligible-marked", "merge wait", true, true),
                    ("eligible-unmarked", "not queued", true, false),
                    ("rejected", "rejected", true, false),
                ],
                expected: vec![false, true, true, false],
            },
            BulkToggleCase {
                name: "parallel mode marks every committed row and skips uncommitted rows",
                mode: AppMode::Select,
                parallel_mode: true,
                rows: vec![
                    ("committed-marked", "not queued", true, true),
                    ("committed-unmarked", "not queued", true, false),
                    ("uncommitted", "not queued", false, false),
                ],
                expected: vec![true, true, false],
            },
        ];

        for case in cases {
            let (app, _) = run_bulk_toggle_case(case.mode, case.parallel_mode, &case.rows);

            let actual: Vec<bool> = app.changes.iter().map(|change| change.selected).collect();
            assert_eq!(actual, case.expected, "case failed: {}", case.name);
            assert!(
                app.logs
                    .iter()
                    .any(|entry| entry.message.contains("Toggled all")),
                "case must report a bulk toggle result: {}",
                case.name
            );
        }
    }

    #[test]
    fn test_toggle_all_marks_reports_changed_and_excluded_counts_with_reasons() {
        let (app, commands) = run_bulk_toggle_case(
            AppMode::Running,
            false,
            &[
                ("active", "applying", true, false),
                ("rejected", "rejected", true, false),
                ("eligible", "not queued", true, false),
            ],
        );

        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], TuiCommand::AddToQueue(id) if id == "eligible"));

        let warning = app
            .warning_message
            .as_ref()
            .expect("exclusions must be surfaced to the user");
        assert!(
            warning.contains("1 marked change(s)") && warning.contains("2 excluded"),
            "warning must report changed and excluded counts: {}",
            warning
        );
        assert!(
            warning.contains("in progress") && warning.contains("rejected"),
            "warning must report actionable exclusion reasons: {}",
            warning
        );
        assert!(
            app.logs.iter().any(|entry| entry.message == *warning),
            "bulk toggle result must also be logged"
        );
    }

    #[test]
    fn test_toggle_all_marks_without_exclusions_does_not_warn() {
        let (app, _) = run_bulk_toggle_case(
            AppMode::Select,
            false,
            &[
                ("a", "not queued", true, false),
                ("b", "not queued", true, false),
            ],
        );

        assert!(app.warning_message.is_none());
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message == "Toggled all: 2 marked change(s)"));
    }

    #[test]
    fn test_toggle_all_marks_with_zero_eligible_targets_reports_reason() {
        let (app, commands) = run_bulk_toggle_case(
            AppMode::Running,
            false,
            &[
                ("active", "applying", true, false),
                ("rejected", "rejected", true, true),
            ],
        );

        assert!(commands.is_empty());
        assert!(!app.changes[0].selected, "ineligible rows must not change");
        assert!(app.changes[1].selected, "ineligible rows must not change");

        let warning = app
            .warning_message
            .as_ref()
            .expect("zero eligible targets must not be silent");
        assert!(
            warning.contains("no eligible changes") && warning.contains("2 excluded"),
            "warning must explain why nothing happened: {}",
            warning
        );
        assert!(
            warning.contains("in progress") && warning.contains("rejected"),
            "warning must report actionable exclusion reasons: {}",
            warning
        );
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.level == LogLevel::Warn && entry.message == *warning));
    }

    #[test]
    fn test_toggle_all_marks_with_no_changes_reports_reason() {
        let mut app = AppState::new(Vec::new());
        app.mode = AppMode::Select;

        let commands = app.toggle_all_marks();

        assert!(commands.is_empty());
        assert!(app
            .warning_message
            .as_ref()
            .is_some_and(|msg| msg.contains("no changes to toggle")));
    }

    #[test]
    fn test_toggle_all_marks_in_unsupported_mode_reports_reason() {
        let (app, commands) =
            run_bulk_toggle_case(AppMode::Error, false, &[("a", "error", true, false)]);

        assert!(commands.is_empty());
        assert!(!app.changes[0].selected);
        assert!(app
            .warning_message
            .as_ref()
            .is_some_and(|msg| msg.contains("Select, Running, or Stopped mode")));
    }

    #[test]
    fn test_toggle_all_marks_running_partial_selection_emits_command_for_every_eligible_row() {
        let (app, commands) = run_bulk_toggle_case(
            AppMode::Running,
            false,
            &[
                ("already-queued", "queued", true, true),
                ("not-queued-a", "not queued", true, false),
                ("not-queued-b", "not queued", true, false),
                ("active", "resolving", true, false),
            ],
        );

        // Every eligible row ends marked; the active row is untouched.
        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);
        assert!(app.changes[2].selected);
        assert!(!app.changes[3].selected);

        let queued_ids: Vec<String> = commands
            .iter()
            .filter_map(|cmd| match cmd {
                TuiCommand::AddToQueue(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(queued_ids, vec!["not-queued-a", "not-queued-b"]);
        assert_eq!(
            commands.len(),
            queued_ids.len(),
            "active rows must not produce stop or dequeue commands"
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
    fn running_not_queued_toggle_survives_reducer_sync_and_changes_refreshed() {
        let changes = vec![create_test_change("dynamic-change", 0, 1)];
        let mut app = AppState::new(changes.clone());
        app.mode = AppMode::Running;

        let command = app.toggle_selection();
        assert!(matches!(command, Some(TuiCommand::AddToQueue(ref id)) if id == "dynamic-change"));
        assert!(app.changes[0].selected);

        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "dynamic-change".to_string(),
            "queued",
        )]));
        app.handle_orchestrator_event(OrchestratorEvent::ChangesRefreshed {
            changes,
            committed_change_ids: HashSet::new(),
            rejected_changes: Vec::new(),
            uncommitted_file_change_ids: HashSet::new(),
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::new(),
            worktree_not_ahead_ids: HashSet::new(),
            merge_wait_ids: HashSet::new(),
        });

        assert!(
            app.changes[0].selected,
            "execution mark must survive refresh"
        );
        assert_eq!(app.changes[0].display_status_cache, "queued");
    }

    #[test]
    fn reducer_sync_marks_queued_rows_selected_for_running_unqueue() {
        let changes = vec![create_test_change("queued-change", 0, 1)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;

        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "queued-change".to_string(),
            "queued",
        )]));

        assert!(
            app.changes[0].selected,
            "queued reducer intent should show as marked"
        );
        let command = app.toggle_selection();
        assert!(
            matches!(command, Some(TuiCommand::RemoveFromQueue(ref id)) if id == "queued-change")
        );
        assert!(!app.changes[0].selected);
    }

    #[test]
    fn active_status_survives_reducer_sync_and_changes_refreshed() {
        let changes = vec![create_test_change("active-change", 0, 1)];
        let mut app = AppState::new(changes.clone());
        app.mode = AppMode::Running;
        app.changes[0].set_display_status_cache("queued");
        app.changes[0].selected = true;

        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "active-change".to_string(),
            "applying",
        )]));
        app.handle_orchestrator_event(OrchestratorEvent::ChangesRefreshed {
            changes,
            committed_change_ids: HashSet::new(),
            rejected_changes: Vec::new(),
            uncommitted_file_change_ids: HashSet::new(),
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::new(),
            worktree_not_ahead_ids: HashSet::new(),
            merge_wait_ids: HashSet::new(),
        });

        assert_eq!(app.changes[0].display_status_cache, "applying");
        assert!(app.changes[0].selected);
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

    #[test]
    fn test_apply_display_statuses_from_reducer_updates_error_row_to_merged() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].set_error_message_cache("previous failure".to_string());
        app.changes[0].selected = true;
        assert_eq!(app.changes[0].display_status_cache, "error");

        let display_map = HashMap::from([("test-change".to_string(), "merged")]);
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert!(
            app.changes[0].selected,
            "only rejected rows should be forcibly deselected during reducer sync"
        );
    }

    // Iteration guard tests

    #[test]
    fn test_iteration_monotonic_update_from_none() {
        let mut change = ChangeState {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: "applying".to_string(),
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
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
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
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
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
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
    fn test_resolve_queue_auto_start_on_completion() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);

        // Set change-a to Resolving
        app.changes[0].display_status_cache = "resolving".to_string();
        app.set_resolving("change-a");

        // Queue change-b for resolve
        app.add_to_resolve_queue("change-b");
        app.changes[1].display_status_cache = "resolve pending".to_string();

        // Simulate resolve completion for change-a
        let cmd = app.handle_resolve_completed("change-a".to_string(), None);

        // Should return command to start change-b
        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-b"));
        // is_resolving should be cleared
        assert!(!app.is_resolving());
        // Queue should be empty
        assert!(!app.has_queued_resolves());
    }

    #[test]
    fn test_resolve_queue_no_auto_start_when_queue_empty() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set change-a to Resolving
        app.changes[0].display_status_cache = "resolving".to_string();
        app.set_resolving("change-a");

        // Simulate resolve completion with empty queue
        let cmd = app.handle_resolve_completed("change-a".to_string(), None);

        // Should NOT return a command
        assert!(cmd.is_none());
        // is_resolving should be cleared
        assert!(!app.is_resolving());
    }

    #[test]
    fn test_resolve_completed_does_not_log_duplicate_after_merge_completed() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].display_status_cache = "merged".to_string();

        let cmd = app.handle_resolve_completed("change-a".to_string(), None);

        assert!(cmd.is_none());
        assert!(!app
            .logs
            .iter()
            .any(|log| log.message == "Merge resolved for 'change-a'"));
    }

    // ── `M` is presentation only ────────────────────────────────────────────
    //
    // The reservation, FIFO ordering, duplicate rejection, reducer transition,
    // and scheduler dispatch all belong to the shared run-control service now —
    // that is what makes an `M` keypress and a remote `resolve_merge` reach one
    // conclusion instead of two. What `AppState` still owns is which row the
    // cursor addresses and whether the key offers a command at all, so that is
    // all these tests pin.

    #[test]
    fn m_emits_a_resolve_command_without_deciding_its_outcome() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.cursor_index = 0;
        app.mode = AppMode::Running;

        let cmd = app.resolve_merge();

        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-a"));
        assert!(
            !app.is_resolving(),
            "the key path must not pre-empt the shared single-resolver rule"
        );
        assert!(
            !app.resolve_reservations().is_reserved("change-a"),
            "reserving here would turn the service's own reservation into a duplicate"
        );
        assert_eq!(
            app.changes[0].display_status_cache, "merge wait",
            "the row advances when the service accepts, not when the key is pressed"
        );
    }

    #[test]
    fn m_is_ignored_for_a_row_that_is_not_waiting_on_a_merge() {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.changes[0].display_status_cache = "queued".to_string();
        app.cursor_index = 0;
        app.mode = AppMode::Running;

        assert!(app.resolve_merge().is_none());
    }

    #[test]
    fn m_offers_the_same_command_in_every_mode_that_allows_it() {
        for mode in [AppMode::Select, AppMode::Stopped, AppMode::Running] {
            let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
            app.changes[0].display_status_cache = "merge wait".to_string();
            app.cursor_index = 0;
            app.mode = mode.clone();

            assert!(
                matches!(
                    app.resolve_merge(),
                    Some(TuiCommand::ResolveMerge(ref id)) if id == "change-a"
                ),
                "{mode:?} must offer resolve"
            );
        }
    }

    #[test]
    fn consecutive_m_presses_emit_one_command_per_row() {
        let mut app = AppState::new(vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ]);
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.changes[1].display_status_cache = "merge wait".to_string();
        app.mode = AppMode::Running;

        app.cursor_index = 0;
        let first = app.resolve_merge();
        app.cursor_index = 1;
        let second = app.resolve_merge();

        assert!(matches!(first, Some(TuiCommand::ResolveMerge(id)) if id == "change-a"));
        assert!(
            matches!(second, Some(TuiCommand::ResolveMerge(id)) if id == "change-b"),
            "the second press still reaches the service, which is what decides it queues"
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
        app.set_resolving("__active__");

        // change-b is in MergeWait; user presses M to queue resolve
        app.changes[1].display_status_cache = "merge wait".to_string();
        app.cursor_index = 1;
        app.mode = AppMode::Running;

        // What the shared run-control service does when it accepts the resolve:
        // record the reducer intent, then let the adapter advance the row.
        shared
            .blocking_write()
            .apply_command(crate::orchestration::state::ReducerCommand::ResolveMerge(
                "change-b".to_string(),
            ));
        app.changes[1].set_display_status_cache("resolve pending");

        // Simulate a ChangesRefreshed event where workspace still reports change-b
        // as Archived (which would normally set MergeWait in the reducer).
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::ChangesRefreshed {
                changes: vec![],
                committed_change_ids: HashSet::new(),
                rejected_changes: Vec::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: ["change-b".to_string()].into_iter().collect(),
            });
        }

        // The actual TUI path applies reducer display first, then local ChangesRefreshed handling.
        let display_map = shared.blocking_read().all_display_statuses();
        app.apply_display_statuses_from_reducer(&display_map);
        app.handle_orchestrator_event(OrchestratorEvent::ChangesRefreshed {
            changes: vec![
                create_test_change("change-a", 0, 1),
                create_test_change("change-b", 0, 1),
            ],
            rejected_changes: Vec::new(),
            committed_change_ids: HashSet::new(),
            uncommitted_file_change_ids: HashSet::new(),
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::new(),
            worktree_not_ahead_ids: HashSet::new(),
            merge_wait_ids: ["change-b".to_string()].into_iter().collect(),
        });

        assert_eq!(
            app.changes[1].display_status_cache, "resolve pending",
            "ResolveWait must survive reducer sync followed by ChangesRefreshed handling"
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
        app.clear_resolving();

        // What the shared run-control service does when it accepts the resolve.
        shared
            .blocking_write()
            .apply_command(crate::orchestration::state::ReducerCommand::ResolveMerge(
                "change-a".to_string(),
            ));
        app.changes[0].set_display_status_cache("resolve pending");

        // Simulate a ChangesRefreshed event where workspace still reports change-a
        // as needing merge (which would normally set MergeWait in the reducer).
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::ChangesRefreshed {
                changes: vec![],
                committed_change_ids: HashSet::new(),
                rejected_changes: Vec::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: ["change-a".to_string()].into_iter().collect(),
            });
        }

        // The actual TUI path applies reducer display first, then local ChangesRefreshed handling.
        let display_map = shared.blocking_read().all_display_statuses();
        app.apply_display_statuses_from_reducer(&display_map);
        app.handle_orchestrator_event(OrchestratorEvent::ChangesRefreshed {
            changes: vec![create_test_change("change-a", 0, 1)],
            committed_change_ids: HashSet::new(),
            rejected_changes: Vec::new(),
            uncommitted_file_change_ids: HashSet::new(),
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::new(),
            worktree_not_ahead_ids: HashSet::new(),
            merge_wait_ids: ["change-a".to_string()].into_iter().collect(),
        });

        assert_eq!(
            app.changes[0].display_status_cache, "resolve pending",
            "ResolveWait must survive reducer sync followed by ChangesRefreshed handling after immediate resolve"
        );
    }

    #[test]
    fn test_handle_orchestrator_event_merge_completed_returns_queued_resolve() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.set_resolving("__active__");
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "resolve pending".to_string();
        app.add_to_resolve_queue("change-b");

        let cmd = app.handle_orchestrator_event(OrchestratorEvent::MergeCompleted {
            change_id: "change-a".to_string(),
            revision: "abc123".to_string(),
        });

        assert!(matches!(cmd, Some(TuiCommand::ResolveMerge(id)) if id == "change-b"));
        assert!(!app.is_resolving());
        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
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

    #[test]
    fn test_apply_display_statuses_keeps_rejected_row_read_only() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);

        app.changes[0].display_status_cache = "rejected".to_string();
        app.changes[0].selected = true;

        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "not queued");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(
            app.changes[0].display_status_cache, "rejected",
            "rejected row must stay immutable during reducer display sync"
        );
        assert!(
            !app.changes[0].selected,
            "rejected row must remain unselected"
        );
    }

    #[test]
    fn test_update_changes_reactivates_rejected_row_when_marker_removed() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes.clone());
        app.changes[0].display_status_cache = "rejected".to_string();
        app.changes[0].selected = true;

        app.update_changes_with_rejected_for_test(changes, Vec::new());

        assert_eq!(
            app.changes[0].display_status_cache, "not queued",
            "active refresh without marker should reactivate rejected row"
        );
        assert!(
            !app.changes[0].selected,
            "reactivated row must remain unselected until explicit user action"
        );
    }

    #[test]
    fn test_update_changes_new_rejected_row_is_not_new_and_not_counted() {
        let mut app = AppState::new(vec![]);

        let rejected = create_test_change("change-rejected", 0, 1);
        app.update_changes_with_rejected_for_test(vec![], vec![rejected]);

        let row = app
            .changes
            .iter()
            .find(|c| c.id == "change-rejected")
            .expect("rejected row should be added");
        assert_eq!(row.display_status_cache, "rejected");
        assert!(
            !row.is_new,
            "newly surfaced rejected row must not carry NEW badge"
        );
        assert_eq!(
            app.new_change_count, 0,
            "rejected rows must not increment NEW counter"
        );
    }

    #[test]
    fn test_toggle_selection_blocks_rejected_row() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].display_status_cache = "rejected".to_string();
        app.mode = AppMode::Select;

        let cmd = app.toggle_selection();

        assert!(cmd.is_none(), "rejected row must not emit toggle commands");
        assert!(
            !app.changes[0].selected,
            "rejected row must remain unselected"
        );
        assert!(
            app.warning_message
                .as_deref()
                .is_some_and(|m| m.contains("read-only")),
            "rejected toggle should explain read-only guard"
        );
    }

    #[test]
    fn test_toggle_all_marks_ignores_rejected_rows() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Select;
        app.changes[0].display_status_cache = "rejected".to_string();
        app.changes[1].display_status_cache = "not queued".to_string();

        let _ = app.toggle_all_marks();

        assert!(
            !app.changes[0].selected,
            "bulk mark (@) must not mark rejected rows"
        );
        assert!(
            app.changes[1].selected,
            "eligible rows should still be marked"
        );
    }

    #[test]
    fn test_apply_display_statuses_rejected_clears_selection() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].selected = true;

        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "rejected");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(app.changes[0].display_status_cache, "rejected");
        assert!(
            !app.changes[0].selected,
            "rejected terminal row must not keep execution mark"
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
    // Force-kill: Space on active change must NOT trigger stop
    // -----------------------------------------------------------------------

    #[test]
    fn test_running_mode_space_on_active_change_does_not_stop() {
        let changes = vec![create_test_change("c1", 0, 3)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "applying".to_string();

        let cmd = app.toggle_selection();
        assert!(
            cmd.is_none(),
            "Space on active change must NOT issue any command, got {:?}",
            cmd
        );
    }

    #[test]
    fn test_running_mode_space_on_accepting_does_not_stop() {
        let changes = vec![create_test_change("c1", 0, 3)];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "accepting".to_string();

        let cmd = app.toggle_selection();
        assert!(
            cmd.is_none(),
            "Space on accepting change must NOT issue any command, got {:?}",
            cmd
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

    #[test]
    fn test_apply_display_statuses_from_reducer_shows_reject_pending() {
        let changes = vec![create_test_change("reject-b", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].display_status_cache = "queued".to_string();

        let mut display_map = std::collections::HashMap::new();
        display_map.insert("reject-b".to_string(), "reject pending");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(app.changes[0].display_status_cache, "reject pending");
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

        // The start path queues the change in the reducer and the row cache.
        shared.blocking_write().apply_command(
            crate::orchestration::state::ReducerCommand::AddToQueue("change-a".to_string()),
        );
        app.begin_run(&["change-a".to_string()]);
        assert_eq!(app.changes[0].display_status_cache, "queued");

        // Simulate initial parallel ChangesRefreshed (workspace scan returns nothing special).
        {
            let mut guard = shared.blocking_write();
            guard.apply_execution_event(&crate::events::ExecutionEvent::ChangesRefreshed {
                changes: vec![],
                committed_change_ids: HashSet::new(),
                rejected_changes: Vec::new(),
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

        // The start path queues the change in the reducer and the row cache.
        shared
            .blocking_write()
            .apply_command(ReducerCommand::AddToQueue("change-a".to_string()));
        app.begin_run(&["change-a".to_string()]);
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
                rejected_changes: Vec::new(),
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

        // Simulate the start path (queues change in both TUI and reducer).
        app.changes[0].selected = true;
        app.changes[0].is_parallel_eligible = true;
        shared.blocking_write().apply_command(
            crate::orchestration::state::ReducerCommand::AddToQueue("change-a".to_string()),
        );
        app.begin_run(&["change-a".to_string()]);

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
        shared.blocking_write().apply_command(
            crate::orchestration::state::ReducerCommand::AddToQueue("change-a".to_string()),
        );
        app.begin_run(&["change-a".to_string()]);

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

    /// Duplicate DependencyBlocked/DependencyResolved events should not spam user-visible TUI logs
    /// when the display state has not changed.
    #[test]
    fn test_duplicate_dependency_events_are_tui_log_noops() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_dependency_blocked("change-a".to_string());
        app.handle_dependency_blocked("change-a".to_string());
        assert_eq!(app.changes[0].display_status_cache, "blocked");
        assert_eq!(
            app.logs
                .iter()
                .filter(|log| log.message.contains("blocked by dependencies"))
                .count(),
            1,
            "duplicate blocked event should not append a duplicate log"
        );

        app.handle_dependency_resolved("change-a".to_string());
        app.handle_dependency_resolved("change-a".to_string());
        assert_eq!(app.changes[0].display_status_cache, "queued");
        assert_eq!(
            app.logs
                .iter()
                .filter(|log| log.message.contains("dependencies resolved"))
                .count(),
            1,
            "duplicate resolved event should not append a duplicate log"
        );

        app.handle_dependency_blocked("change-a".to_string());
        assert_eq!(
            app.logs
                .iter()
                .filter(|log| log.message.contains("blocked by dependencies"))
                .count(),
            2,
            "real resolved -> blocked transition should still append a new log"
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
        app.set_resolving("__active__");

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
        app.set_resolving("__active__");

        app.handle_resolve_completed("change-b".to_string(), None);

        assert_eq!(
            app.mode,
            AppMode::Running,
            "Should stay Running when other active changes remain"
        );
    }
}
