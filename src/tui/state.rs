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
use crate::orchestration::operator_command::ParallelEligibility;
use crate::parallel::dedup::{DiagnosticDeduplicationKey, DiagnosticDeduplicationStore};
use crate::tui::config::TuiConfig;
use crate::tui::events::{LogEntry, LogLevel, TuiCommand};
use crate::tui::types::{
    AppExecutionMode, DeleteIntent, ModalState, StopMode, ViewMode, WorkspaceDirtyState,
    WorktreeInfo,
};
use ratatui::style::Color;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};

use std::path::PathBuf;

use std::time::{Duration, Instant};
use tracing::{error, info, warn};

#[cfg(test)]
mod execution_mark_tests;
pub(crate) mod log_logic;
pub(crate) mod modal_logic;
mod processing_logic;
mod selection_logic;
#[cfg(test)]
mod workspace_dirty_observability_tests;
mod worktree_action_logic;
mod worktree_logic;

pub(crate) use selection_logic::ACTIVE_APPLY_LIMIT_EXPLANATION;

// ============================================================================
// Constants
// ============================================================================

/// Auto-refresh interval in seconds
pub const AUTO_REFRESH_INTERVAL_SECS: u64 = 5;

/// Maximum number of log entries to keep
pub const MAX_LOG_ENTRIES: usize = 1000;

/// Stated when an `error` row has no retained diagnostic at all.
///
/// Explicit by design: the alternative — presenting an unrelated ordinary log as
/// the failure reason — would misreport why the change failed.
pub const ERROR_DETAILS_UNAVAILABLE: &str = "Error details unavailable";

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

/// Outcome of the most recent copy attempt made from the Error Details popup.
///
/// Kept on the popup rather than in the log panel so the operator reads the
/// result where the action happened, without the popup closing under them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyFeedback {
    /// The clipboard accepted the text.
    Copied,
    /// The clipboard refused, carrying the reason it gave.
    Failed(String),
}

impl CopyFeedback {
    /// Operator-facing one-line message for this outcome.
    pub fn message(&self) -> String {
        match self {
            CopyFeedback::Copied => "Copied to clipboard".to_string(),
            CopyFeedback::Failed(reason) => {
                format!("Copy failed: {reason}. Select the text manually to copy it.")
            }
        }
    }
}

/// Error Details popup content and popup-local presentation state.
///
/// Process-local observability state. It holds the retained final diagnostic for
/// one change so the operator can read and copy it after the matching log entry
/// has been evicted from the bounded buffer. It is never persisted and never
/// used as scheduler dispatch, retry routing, acceptance, archive, or merge input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorDetailsPopup {
    /// Change the diagnostic belongs to.
    pub change_id: String,
    /// Complete, untruncated final diagnostic.
    pub error: String,
    /// Popup-local scroll offset; it never moves the Changes list or Logs panel.
    pub scroll: u16,
    /// Result of the last copy attempt, if one has been made.
    pub copy_feedback: Option<CopyFeedback>,
}

impl ErrorDetailsPopup {
    /// The exact plain text `c` places on the clipboard.
    ///
    /// Stable by contract: `Change: <id>` and `Error: <diagnostic>` on two
    /// lines, so a pasted report is the same shape every time.
    pub fn clipboard_text(&self) -> String {
        format!("Change: {}\nError: {}", self.change_id, self.error)
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
    /// Whether this change is eligible for parallel execution, and why not.
    ///
    /// The reason is carried rather than collapsed into a boolean because the
    /// two ineligible cases look identical to admission but not to an operator:
    /// only dirty proposal content may be presented as uncommitted state.
    pub parallel_eligibility: ParallelEligibility,
    /// Whether a worktree exists for this change
    pub has_worktree: bool,
    /// When processing started for this change
    pub started_at: Option<Instant>,
    /// Elapsed time when processing finished (for display after completion)
    pub elapsed_time: Option<Duration>,
    /// Current iteration number (for apply/archive/acceptance operations)
    pub iteration_number: Option<u32>,
    /// Ephemeral Apply-lane operation label: `"apply"` or `"commit"`.
    ///
    /// Purely rendering state. `display_status_cache` stays `"applying"` for the
    /// whole finalization sequence, and nothing routes on this field.
    pub apply_operation_cache: String,
    /// Whether the active run still owns this change's exhausted Apply ceiling.
    ///
    /// Synchronized from the one shared query
    /// ([`crate::orchestration::operator_command::active_apply_iteration_limit`])
    /// rather than derived from the diagnostic text or the iteration number. The
    /// cache exists because row `Space` flips a mark optimistically *before* the
    /// command reaches the service; without it the UI would briefly claim intent
    /// the service is about to refuse. It is presentation state only — the
    /// service guard stands on its own with no TUI attached.
    pub apply_iteration_limit_active: bool,
}

/// Main application state for the TUI
pub struct AppState {
    /// Current view mode (Changes or Worktrees)
    pub view_mode: ViewMode,
    /// Current orchestration execution mode.
    ///
    /// This axis is never replaced by a popup: overlays live in [`AppState::modal`].
    pub execution_mode: AppExecutionMode,
    /// Active modal interaction layered over `execution_mode`, if any.
    ///
    /// Process-local presentation/input-ownership state. It is never persisted and
    /// must not be used as scheduler dispatch, resume routing, acceptance, archive,
    /// merge, or next-action input.
    pub modal: Option<ModalState>,
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
    /// Error Details popup for a change-level failure, if one is open.
    ///
    /// Lives on its own axis rather than in [`AppState::modal`] so opening it can
    /// never replace a pending confirmation, and so it can claim its keys ahead
    /// of interaction modals while still yielding to the warning popup.
    pub error_details_popup: Option<ErrorDetailsPopup>,
    /// Clipboard boundary used by the Error Details popup.
    ///
    /// Injectable so tests assert what would have been copied without mutating
    /// the developer's clipboard.
    clipboard: std::sync::Arc<dyn crate::tui::clipboard::Clipboard>,
    /// Current spinner animation frame
    pub spinner_frame: usize,
    /// Process-local sequence number of `logs[0]`.
    ///
    /// Buffer trimming shifts indices, so the Logs anchor cannot use them as
    /// identity. This counter gives every retained entry a stable process-local
    /// ID (`log_seq_base + index`) without adding a field to `LogEntry`.
    log_seq_base: u64,
    /// Top visible Logs display line, in source coordinates.
    ///
    /// `None` means "follow the newest line" — the auto-scroll position.
    /// Ephemeral in-process presentation state only; it is discarded on restart
    /// and never used as workflow-control input.
    pub(crate) log_anchor: Option<log_logic::LogViewAnchor>,
    /// Last Logs-panel geometry the renderer observed.
    pub(crate) log_viewport: log_logic::LogViewport,
    /// Whether to auto-scroll logs to bottom on new entries
    pub log_auto_scroll: bool,
    /// Current stop mode
    pub stop_mode: StopMode,
    /// Whether Ready is currently backed by a live persistent-scheduler idle
    /// episode rather than pre-run selection.
    ///
    /// Process-local presentation state, exactly like `execution_mode`: it
    /// defaults to false, is discarded on restart, and never authorizes a
    /// command. It only makes the live-scheduler controls discoverable —
    /// [`crate::orchestration::run_control::RunControlService`] revalidates
    /// scheduler liveness itself before executing any of them.
    pub persistent_scheduler_idle: bool,
    /// VCS backend being used (git)
    pub vcs_backend: String,
    /// Max concurrent workspaces for worktree execution
    pub max_concurrent: usize,
    /// When orchestration started (for overall elapsed time)
    pub orchestration_started_at: Option<Instant>,
    /// Total elapsed time when orchestration finished
    pub orchestration_elapsed: Option<Duration>,
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
    /// Shared process-wide worktree runtime facts.
    ///
    /// One store, shared by every frontend, so a keypress and a remote command
    /// read the same concurrency, backend, and per-change eligibility.
    parallel_runtime: std::sync::Arc<crate::orchestration::operator_command::ParallelRuntime>,
    /// Latest successful workspace dirty observation from the local refresh task.
    ///
    /// Private on purpose: the only way to move it is
    /// [`AppState::adopt_workspace_dirty_observation`], which consumes a typed
    /// event that the refresh task publishes only after a completed `git status`
    /// read. Nothing else can turn a failed or unfinished check into a
    /// `Clean` claim.
    ///
    /// Presentation-only, process-local state. It is never persisted and never
    /// used as scheduler dispatch, resume routing, acceptance, archive, or
    /// next-action input.
    workspace_dirty: WorkspaceDirtyState,
    /// Target-scoped mark writes an operator interaction requested but that the
    /// shared service has not applied yet.
    ///
    /// Key handling is where the operator expresses intent, and it must not be
    /// the place the shared store is written: the write has to take the same
    /// mutation guard event reconciliation takes, and that guard is async. The
    /// interaction records `(change_id, marked)` pairs here instead, the run loop
    /// drains them through the shared service, and the rows are then mirrored
    /// back from the store. Nothing outside one process lifetime sees this.
    pending_mark_writes: Vec<(String, bool)>,
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
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
            apply_operation_cache: "apply".to_string(),
            apply_iteration_limit_active: false,
        }
    }

    /// Operation label the Apply lane renders for this row.
    pub fn apply_operation(&self) -> &str {
        if self.apply_operation_cache.is_empty() {
            return "apply";
        }
        &self.apply_operation_cache
    }

    /// Whether this change may take part in parallel execution.
    ///
    /// Every ineligible reason answers `false` here: this is the admission
    /// guard, and narrowing it is never what distinguishing reasons is for.
    pub fn is_parallel_eligible(&self) -> bool {
        self.parallel_eligibility.is_eligible()
    }

    /// Whether uncommitted or untracked proposal files were actually observed.
    ///
    /// The `UNCOMMITTED` badge and the commit instruction key off this rather
    /// than off [`ChangeState::is_parallel_eligible`], so a clean proposal that
    /// is merely absent from `HEAD` is never described as dirty.
    pub fn has_uncommitted_proposal_files(&self) -> bool {
        self.parallel_eligibility.has_uncommitted_proposal_files()
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
            "preparing" => Color::Green,
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
            execution_mode: AppExecutionMode::Select,
            modal: None,
            changes: change_states,
            cursor_index: 0,
            list_state,
            worktrees: Vec::new(),
            worktree_cursor_index: 0,
            worktree_list_state: ListState::default(),
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
            error_details_popup: None,
            clipboard: crate::tui::clipboard::default_clipboard(),
            spinner_frame: 0,
            log_seq_base: 0,
            log_anchor: None,
            log_viewport: log_logic::LogViewport::default(),
            log_auto_scroll: true,
            stop_mode: StopMode::None,
            persistent_scheduler_idle: false,
            vcs_backend: "git".to_string(),
            max_concurrent: 4, // Default value, can be overridden from config
            orchestration_started_at: None,
            orchestration_elapsed: None,
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
            parallel_runtime: std::sync::Arc::new(
                crate::orchestration::operator_command::ParallelRuntime::new(),
            ),
            workspace_dirty: WorkspaceDirtyState::default(),
            pending_mark_writes: Vec::new(),
        }
    }

    /// Latest workspace dirty observation, for the header badge.
    ///
    /// Read-only by design: rendering is the only consumer, and there is no
    /// setter beyond the typed observation adoption path.
    pub fn workspace_dirty(&self) -> WorkspaceDirtyState {
        self.workspace_dirty
    }

    /// Shared handle to the process-wide parallel runtime facts.
    pub fn parallel_runtime(
        &self,
    ) -> std::sync::Arc<crate::orchestration::operator_command::ParallelRuntime> {
        self.parallel_runtime.clone()
    }

    /// Adopt an externally owned parallel runtime store.
    ///
    /// Test-only for the same reason as [`AppState::set_execution_marks`]:
    /// production wires this the other way round — the run-control service and
    /// the web state are built over `parallel_runtime()` — but a test that
    /// builds the shared services first needs the app to join the store they
    /// already read, or the toggle would exist once per frontend.
    #[cfg(test)]
    pub fn set_parallel_runtime(
        &mut self,
        parallel: std::sync::Arc<crate::orchestration::operator_command::ParallelRuntime>,
    ) {
        self.parallel_runtime = parallel;
        self.publish_parallel_runtime();
    }

    /// Publish the TUI's observations into the shared parallel runtime store.
    ///
    /// Concurrency, backend, and per-change eligibility are all observations
    /// only this frontend makes; publishing them is what lets the shared start
    /// guard and a remote client use them without re-deriving them.
    pub fn publish_parallel_runtime(&self) {
        self.parallel_runtime
            .set_max_concurrent(self.max_concurrent);
        self.parallel_runtime
            .set_vcs_backend(self.vcs_backend.clone());
        self.parallel_runtime.set_parallel_ineligible(
            self.changes
                .iter()
                .filter(|change| !change.is_parallel_eligible())
                .map(|change| (change.id.clone(), change.parallel_eligibility)),
        );
    }

    /// Clear mark and queue presentation for every parallel-ineligible row.
    ///
    /// Returns the affected change IDs so the caller can report them; an
    /// operator whose marks silently disappeared cannot tell that from a bug.
    fn clear_parallel_ineligible_intent(&mut self) -> Vec<String> {
        use crate::orchestration::operator_command::{
            parallel_cleanup_targets, ParallelCleanupRow,
        };

        let rows: Vec<ParallelCleanupRow<'_>> = self
            .changes
            .iter()
            .map(|change| ParallelCleanupRow {
                change_id: &change.id,
                parallel_eligible: change.is_parallel_eligible(),
                marked: change.selected,
                queued: change.display_status_cache == "queued",
            })
            .collect();
        let cleared = parallel_cleanup_targets(&rows);

        for change in &mut self.changes {
            if !cleared.contains(&change.id) {
                continue;
            }
            change.selected = false;
            if change.display_status_cache == "queued" {
                change.set_display_status_cache("not queued");
            }
            // Target-scoped, never a whole-store replace: an ineligible row loses
            // its own mark, and a mark another frontend set on an unrelated row
            // cannot be swept away by this frontend's cached row set.
            self.execution_marks.set(&change.id, false);
        }
        cleared
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
    /// Test-only by construction: production wires this the other way round — the
    /// run-control service is built over `execution_marks()` — but a test that
    /// builds the service first needs the app to join the store the service
    /// already reads, or the "authoritative marked target set" would exist in two
    /// copies.
    #[cfg(test)]
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
    ///
    /// Test-only for the same reason as [`AppState::set_execution_marks`]:
    /// production builds the run-control service over `resolve_reservations()`.
    #[cfg(test)]
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

    /// Mirror row marks from the shared store.
    ///
    /// One direction only, and this is the direction: the store decides, the row
    /// renders. A system event can revoke a mark while this frontend still holds
    /// a stale row, so a row that disagreed with the store used to be a second
    /// authority — the one that let the TUI show `[ ]` while `/api/v2` still
    /// reported `execution_marked: true`.
    pub fn sync_execution_marks_from_store(&mut self) {
        for change in &mut self.changes {
            change.selected = self.execution_marks.is_marked(&change.id);
        }
    }

    /// Record a target-scoped mark write for the run loop to apply.
    ///
    /// The interaction has already run the shared admission classification for
    /// the row; what is deferred is only the *write*, because it must take the
    /// same async mutation guard event reconciliation takes.
    pub(crate) fn request_mark_write(&mut self, change_id: &str, marked: bool) {
        if let Some(pending) = self
            .pending_mark_writes
            .iter_mut()
            .find(|(id, _)| id == change_id)
        {
            pending.1 = marked;
            return;
        }
        self.pending_mark_writes
            .push((change_id.to_string(), marked));
    }

    /// Take the mark writes recorded since the last drain, in request order.
    pub fn take_pending_mark_writes(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.pending_mark_writes)
    }

    /// Apply the pending mark writes straight to the store.
    ///
    /// Test-only: production drains them through `OperatorCommandService`, which
    /// is what makes the write take the shared mutation guard. A test with no
    /// service still needs the store to reach the state the run loop would have
    /// produced before the next event arrives.
    #[cfg(test)]
    pub fn flush_pending_mark_writes(&mut self) {
        for (change_id, marked) in self.take_pending_mark_writes() {
            self.execution_marks.set(&change_id, marked);
        }
    }

    /// Publish the current TUI row projection into the shared store.
    ///
    /// Test-only. Production never writes the store from a whole row set: a
    /// cached row that a concurrent event already invalidated would resurrect
    /// the mark that event revoked, and would overwrite marks this frontend
    /// never observed. Operator interactions use target-scoped writes through
    /// the shared service instead; tests use this to arrange a starting state.
    #[cfg(test)]
    pub fn publish_execution_marks(&self) {
        self.execution_marks.replace(
            self.changes
                .iter()
                .filter(|change| change.selected)
                .map(|change| change.id.clone()),
        );
    }

    /// Adopt the shared active Apply-iteration-limit eligibility set.
    ///
    /// Returns true when the projection actually changed. The run loop uses that
    /// as the scheduler-liveness transition signal: on the frame where the owning
    /// task exits, `limited` arrives empty, the rows are refreshed, and exactly
    /// one authoritative `/api/v2` revision is published — without waiting for
    /// unrelated repository activity to move the snapshot.
    pub fn sync_active_apply_iteration_limits(&mut self, limited: &HashSet<String>) -> bool {
        let mut changed = false;
        for change in &mut self.changes {
            let active = limited.contains(&change.id);
            if change.apply_iteration_limit_active != active {
                change.apply_iteration_limit_active = active;
                changed = true;
            }
        }
        changed
    }

    /// True when any visible row is gated by an active-run Apply ceiling.
    pub fn has_active_apply_iteration_limit(&self) -> bool {
        self.changes
            .iter()
            .any(|change| change.apply_iteration_limit_active)
    }

    /// True when at least one row carries retryable evidence the service admits.
    ///
    /// Route classification is the shared one, so guidance offers the retry key
    /// only when some target really exists for it.
    pub fn has_admissible_retry_target(&self) -> bool {
        self.changes.iter().any(|change| {
            !change.apply_iteration_limit_active
                && crate::orchestration::operator_command::classify_retry_route(
                    &change.display_status_cache,
                    change.blocker_kind_cache,
                )
                .is_some()
        })
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

    /// Open the Error Details popup for the change under the Changes cursor.
    ///
    /// Only an `error` row can open it: every other row keeps whatever `Enter`
    /// already did. Returns whether the popup was opened, so the caller can fall
    /// through to the pre-existing behavior when it was not.
    ///
    /// Opening is presentation only — no workflow-control state changes.
    pub fn open_error_details_popup(&mut self) -> bool {
        if self.view_mode != ViewMode::Changes {
            return false;
        }

        let Some(change) = self.changes.get(self.cursor_index) else {
            return false;
        };
        if change.display_status_cache != "error" {
            return false;
        }

        self.error_details_popup = Some(ErrorDetailsPopup {
            change_id: change.id.clone(),
            error: change
                .error_message_cache
                .clone()
                .unwrap_or_else(|| ERROR_DETAILS_UNAVAILABLE.to_string()),
            scroll: 0,
            copy_feedback: None,
        });
        true
    }

    /// Close the Error Details popup and drop its popup-local state.
    ///
    /// Closing is presentation only: it never transitions the change, the queue,
    /// or the run.
    pub fn close_error_details_popup(&mut self) {
        self.error_details_popup = None;
    }

    /// Scroll the Error Details popup body by a signed amount.
    ///
    /// The offset stays non-negative and the Changes cursor and Logs-panel
    /// position are untouched, so popup scrolling can never move the views
    /// underneath it.
    pub fn scroll_error_details_popup(&mut self, delta: i16) {
        let Some(popup) = self.error_details_popup.as_mut() else {
            return;
        };
        popup.scroll = if delta.is_negative() {
            popup.scroll.saturating_sub(delta.unsigned_abs())
        } else {
            popup.scroll.saturating_add(delta as u16)
        };
    }

    /// Copy the open Error Details popup's diagnostic to the OS clipboard.
    ///
    /// The popup stays open and keeps its complete diagnostic either way; the
    /// outcome is reported inside the popup so the operator never loses the text
    /// they were trying to copy.
    pub fn copy_error_details(&mut self) {
        let Some(popup) = self.error_details_popup.as_ref() else {
            return;
        };

        let text = popup.clipboard_text();
        let feedback = match self.clipboard.set_text(&text) {
            Ok(()) => CopyFeedback::Copied,
            Err(reason) => CopyFeedback::Failed(reason),
        };

        if let Some(popup) = self.error_details_popup.as_mut() {
            popup.copy_feedback = Some(feedback);
        }
    }

    /// Replace the clipboard boundary with a test double.
    ///
    /// Test-only: production always keeps
    /// [`crate::tui::clipboard::default_clipboard`], so this is the seam that
    /// lets automated tests assert what would have been copied without writing
    /// to the developer's real clipboard.
    #[cfg(test)]
    pub fn set_clipboard(
        &mut self,
        clipboard: std::sync::Arc<dyn crate::tui::clipboard::Clipboard>,
    ) {
        self.clipboard = clipboard;
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

    /// Show QR popup (only when web_url is set).
    ///
    /// The popup is layered over the current execution mode; it never captures or
    /// replaces it, so a background transition arriving while the popup is open is
    /// exactly what the operator sees once it closes.
    pub fn show_qr_popup(&mut self) {
        if self.web_url.is_some() {
            self.modal = Some(ModalState::QrPopup);
        }
    }

    /// Hide the QR popup, exposing the latest execution mode.
    pub fn hide_qr_popup(&mut self) {
        if matches!(self.modal, Some(ModalState::QrPopup)) {
            self.modal = None;
        }
    }

    /// True when any overlay — warning popup or modal — owns input.
    ///
    /// The warning popup is an independent diagnostic overlay with its own
    /// scrolling contract; both must suppress ordinary view commands.
    pub fn has_overlay(&self) -> bool {
        self.warning_popup.is_some() || self.modal.is_some()
    }

    /// Fresh observation the active modal's validity is judged against.
    fn modal_validity_context(&self) -> modal_logic::ModalValidityContext<'_> {
        modal_logic::ModalValidityContext {
            execution_mode: self.execution_mode,
            web_url: self.web_url.as_deref(),
            worktrees: &self.worktrees,
            changes: &self.changes,
            deleting_worktree_paths: &self.deleting_worktree_paths,
        }
    }

    /// Re-evaluate the active modal against current state.
    ///
    /// Clearing the modal clears its identity payload with it, because the payload
    /// lives inside the variant. Execution state is never touched here: an overlay
    /// losing its target must not rewrite the lifecycle underneath it.
    ///
    /// Returns the reason when a modal was invalidated.
    pub(crate) fn revalidate_modal(&mut self) -> Option<modal_logic::ModalInvalidation> {
        let modal = self.modal.as_ref()?;
        let invalidation = modal_logic::evaluate(modal, &self.modal_validity_context()).err()?;
        self.modal = None;
        Some(invalidation)
    }

    /// Clear the active modal and report the invalidation reason to the operator.
    fn invalidate_modal_with_report(
        &mut self,
        invalidation: modal_logic::ModalInvalidation,
        action: &str,
    ) {
        self.modal = None;
        let message = format!("{} canceled: {}", action, invalidation.reason());
        self.warning_message = Some(message.clone());
        self.add_log(LogEntry::warn(message));
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
                // Opening a confirmation is advisory. The identity that will be
                // revalidated at confirmation time is captured here, inside the
                // modal variant, so it cannot drift away from the modal itself.
                self.modal = Some(ModalState::ConfirmWorktreeDelete { path, branch });
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
    ///
    /// The confirmation is re-checked against a fresh worktree observation before
    /// anything is mutated: a stale identity refuses the command and leaves the
    /// worktree untouched rather than deleting whatever now occupies the path.
    ///
    /// Neither `Y` (`skip_teardown = false`) nor `S` (`skip_teardown = true`)
    /// grants permission to discard uncommitted work. A known-dirty target
    /// refuses in the shared service and comes back as an escalation into
    /// [`Self::open_dirty_discard_confirmation`].
    pub fn confirm_worktree_action_delete_with_options(
        &mut self,
        skip_teardown: bool,
    ) -> Option<TuiCommand> {
        let Some(ModalState::ConfirmWorktreeDelete { path, branch }) = self.modal.clone() else {
            return None;
        };

        if let Err(invalidation) =
            modal_logic::evaluate_worktree_delete(&path, &branch, &self.modal_validity_context())
        {
            self.invalidate_modal_with_report(invalidation, "Worktree delete");
            return None;
        }

        self.modal = None;
        self.mark_worktree_deleting(path.clone());
        let teardown_note = if skip_teardown {
            " with skip-teardown"
        } else {
            ""
        };
        self.add_log(LogEntry::info(format!(
            "Deleting worktree{}: {}",
            teardown_note,
            path.display()
        )));

        Some(TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
            path,
            branch,
            skip_teardown,
        )))
    }

    /// Escalate the shared service's fresh known-dirty refusal into a second,
    /// explicitly destructive confirmation.
    ///
    /// The target comes from the service's own observation, taken under its
    /// mutation guard. Nothing here is read from `worktrees`, which carries no
    /// dirty state and would be a stale answer if it did.
    pub fn open_dirty_discard_confirmation(
        &mut self,
        target: &crate::worktree_ops::service::DirtyTarget,
        skip_teardown: bool,
    ) {
        self.modal = Some(ModalState::ConfirmDirtyDiscard {
            path: target.path.clone(),
            identity: target.identity.clone(),
            branch: target.branch.clone(),
            head: target.head.clone(),
            skip_teardown,
        });
        self.add_log(LogEntry::warn(format!(
            "Worktree has uncommitted changes: {} (press X to discard them and delete)",
            target.path.display()
        )));
    }

    /// Grant known-dirty discard for the pending destructive confirmation.
    ///
    /// This is reachable only from the uppercase `X` keypress. It re-checks the
    /// confirmation against the latest observation first, so a target that became
    /// active, was re-branded, moved its HEAD, or entered another delete between
    /// the escalation and the keypress refuses instead of being discarded.
    pub fn confirm_dirty_discard(&mut self) -> Option<TuiCommand> {
        let Some(ModalState::ConfirmDirtyDiscard {
            path,
            identity,
            branch,
            head,
            skip_teardown,
        }) = self.modal.clone()
        else {
            return None;
        };

        if let Err(invalidation) =
            modal_logic::evaluate_discard(&path, &branch, &head, &self.modal_validity_context())
        {
            self.invalidate_modal_with_report(invalidation, "Dirty worktree discard");
            return None;
        }

        self.modal = None;
        self.mark_worktree_deleting(path.clone());
        let teardown_note = if skip_teardown {
            " without teardown"
        } else {
            ""
        };
        self.add_log(LogEntry::warn(format!(
            "Discarding uncommitted changes and deleting worktree{}: {}",
            teardown_note,
            path.display()
        )));

        Some(TuiCommand::DeleteWorktree(DeleteIntent {
            path,
            branch,
            identity: Some(identity),
            head: Some(head),
            skip_teardown,
            allow_known_dirty: true,
            allow_commits_ahead: false,
        }))
    }

    /// Escalate the shared service's fresh commits-ahead refusal into a
    /// dedicated destructive confirmation.
    ///
    /// Same rule as [`Self::open_dirty_discard_confirmation`]: every field is the
    /// service's own observation. The worktree list does carry a
    /// `has_commits_ahead` flag, but it is a projection with no identity, no
    /// HEAD, and no dirty state, and it is not what the branch will be deleted
    /// at.
    pub fn open_ahead_discard_confirmation(
        &mut self,
        target: &crate::worktree_ops::service::AheadTarget,
        skip_teardown: bool,
    ) {
        self.modal = Some(ModalState::ConfirmAheadDiscard {
            path: target.path.clone(),
            identity: target.identity.clone(),
            branch: target.branch.clone(),
            head: target.head.clone(),
            dirty: target.dirty,
            skip_teardown,
        });
        let dirty_note = if target.dirty {
            " and uncommitted changes"
        } else {
            ""
        };
        self.add_log(LogEntry::warn(format!(
            "Worktree has unmerged commits{}: {} (press X to delete the worktree and branch '{}')",
            dirty_note,
            target.path.display(),
            target.branch
        )));
    }

    /// Grant commits-ahead discard for the pending destructive confirmation.
    ///
    /// Reachable only from the uppercase `X` keypress on the ahead confirmation.
    /// The dirty permission travels with it exactly when the confirmation
    /// disclosed uncommitted changes, so the one keypress never authorizes a
    /// loss the operator was not shown.
    pub fn confirm_ahead_discard(&mut self) -> Option<TuiCommand> {
        let Some(ModalState::ConfirmAheadDiscard {
            path,
            identity,
            branch,
            head,
            dirty,
            skip_teardown,
        }) = self.modal.clone()
        else {
            return None;
        };

        if let Err(invalidation) =
            modal_logic::evaluate_discard(&path, &branch, &head, &self.modal_validity_context())
        {
            self.invalidate_modal_with_report(invalidation, "Ahead worktree discard");
            return None;
        }

        self.modal = None;
        self.mark_worktree_deleting(path.clone());
        let teardown_note = if skip_teardown {
            " without teardown"
        } else {
            ""
        };
        let dirty_note = if dirty {
            " and uncommitted changes"
        } else {
            ""
        };
        self.add_log(LogEntry::warn(format!(
            "Discarding unmerged commits{} and deleting worktree{} with branch '{}': {}",
            dirty_note,
            teardown_note,
            branch,
            path.display()
        )));

        Some(TuiCommand::DeleteWorktree(DeleteIntent {
            path,
            branch,
            identity: Some(identity),
            head: Some(head),
            skip_teardown,
            allow_known_dirty: dirty,
            allow_commits_ahead: true,
        }))
    }

    /// Cancel pending worktree action.
    ///
    /// Only the overlay is dismissed; the execution mode underneath is whatever the
    /// latest lifecycle event left it as. Cancelling a destructive confirmation
    /// retains the worktree, everything uncommitted in it, and its branch.
    pub fn cancel_worktree_action(&mut self) {
        if matches!(
            self.modal,
            Some(
                ModalState::ConfirmWorktreeDelete { .. }
                    | ModalState::ConfirmDirtyDiscard { .. }
                    | ModalState::ConfirmAheadDiscard { .. }
            )
        ) {
            self.modal = None;
        }
    }

    /// Open the force-kill confirmation for the change under the cursor.
    ///
    /// Returns true when the confirmation was opened. Opening is advisory: the
    /// shared operator command service still revalidates before terminating.
    pub fn request_force_kill_confirmation(&mut self) -> bool {
        if self.view_mode != ViewMode::Changes
            || !matches!(self.execution_mode, AppExecutionMode::Running)
            || self.cursor_index >= self.changes.len()
        {
            return false;
        }

        let change = &self.changes[self.cursor_index];
        if !crate::orchestration::operator_command::is_active_status(&change.display_status_cache) {
            return false;
        }

        let change_id = change.id.clone();
        self.modal = Some(ModalState::ConfirmForceKill {
            change_id: change_id.clone(),
        });
        self.add_log(LogEntry::warn(format!(
            "Confirm force-kill for '{}': press Y to confirm, N/Esc to cancel",
            change_id
        )));
        true
    }

    /// Confirm the pending force-kill, dispatching through the shared service.
    ///
    /// The TUI display cache can only refuse; it never authorizes. A surviving but
    /// stale target is refused here, and everything that survives this check is
    /// still revalidated by `OperatorCommandService::stop_and_dequeue`, which owns
    /// cancellation, termination evidence, and the timeout path.
    pub fn confirm_force_kill(&mut self) -> Option<TuiCommand> {
        let Some(ModalState::ConfirmForceKill { change_id }) = self.modal.clone() else {
            return None;
        };

        if let Err(invalidation) =
            modal_logic::evaluate_force_kill(&change_id, &self.modal_validity_context())
        {
            self.invalidate_modal_with_report(invalidation, "Force-kill");
            return None;
        }

        self.modal = None;
        self.add_log(LogEntry::info(format!(
            "Force-kill confirmed: {}",
            change_id
        )));
        Some(TuiCommand::DequeueChange(change_id))
    }

    /// Cancel the pending force-kill without touching execution state.
    pub fn cancel_force_kill(&mut self) {
        if matches!(self.modal, Some(ModalState::ConfirmForceKill { .. })) {
            self.modal = None;
            self.add_log(LogEntry::info("Force-kill canceled".to_string()));
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
        selection_logic::can_bulk_toggle_change(self.execution_mode, change)
    }

    /// Returns true when at least one change can be targeted by bulk toggle.
    ///
    /// Mirrors the admission `toggle_all_marks` enforces — Changes view, no
    /// overlay owning input, and a mode the shared lifecycle matrix admits — so
    /// the `x` hint is never offered for a key that would be refused.
    pub fn has_bulk_toggle_targets(&self) -> bool {
        self.view_mode == ViewMode::Changes
            && !self.has_overlay()
            && selection_logic::is_bulk_toggle_mode(&self.execution_mode)
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
    /// Uncommitted changes remain excluded.
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
            self.execution_mode,
            AppExecutionMode::Select | AppExecutionMode::Stopped | AppExecutionMode::Running
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
    ///
    /// Assertion helper only: production reads the ledger through
    /// [`AppState::has_queued_resolves`] or the shared run-control service.
    #[cfg(test)]
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
    ///
    /// Both failures keep the change out of parallel queueing; they are recorded
    /// as distinct reasons so rendering and refusal messages can stay truthful
    /// about which one was actually observed.
    pub fn apply_parallel_eligibility(
        &mut self,
        committed_change_ids: &HashSet<String>,
        uncommitted_file_change_ids: &HashSet<String>,
    ) {
        for change in &mut self.changes {
            change.parallel_eligibility = ParallelEligibility::observe(
                &change.id,
                committed_change_ids,
                uncommitted_file_change_ids,
            );
        }

        if matches!(
            self.execution_mode,
            AppExecutionMode::Select | AppExecutionMode::Stopped
        ) {
            self.clear_parallel_ineligible_intent();
        }

        // A change that just became ineligible must reach the shared start
        // guard at the same observation, not one refresh later.
        self.publish_parallel_runtime();
    }

    /// Update worktree presence flags for changes.
    pub fn apply_worktree_status(&mut self, worktree_change_ids: &HashSet<String>) {
        for change in &mut self.changes {
            let sanitized = change.id.replace(['/', '\\', ' '], "-");
            change.has_worktree = worktree_change_ids.contains(&sanitized);
        }
    }

    /// Sync the Apply-lane operation label from the reducer.
    ///
    /// The commit subphase already arrives as an event, so this is a
    /// self-healing refresh rather than the only writer: a frontend that missed
    /// one event still converges on the reducer's view instead of rendering
    /// `[commit]` forever. It cannot change any row's status, color, or
    /// lifecycle — only which word the Apply lane prints.
    pub fn apply_operation_labels_from_reducer(
        &mut self,
        operation_map: &HashMap<String, &'static str>,
    ) {
        for change in &mut self.changes {
            if let Some(&label) = operation_map.get(&change.id) {
                change.apply_operation_cache = label.to_string();
            }
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
                continue;
            }

            // Reducer `queued` is queue presentation, not an execution mark.
            // Amplifying it into `selected = true` here used to invent operator
            // intent the shared store never recorded, which is exactly the drift
            // `/api/v2` would then report as `execution_marked` for a change no
            // one marked.
            if let Some(&status_str) = display_map.get(&change.id) {
                let normalized = match status_str {
                    "stopped" => "not queued",
                    other => other,
                };

                if normalized == "error" {
                    if change.display_status_cache == "error" {
                        continue;
                    }
                    // The diagnostic itself is never invented here. It arrives
                    // from the reducer through
                    // [`AppState::apply_error_details_from_reducer`], so an
                    // operator sees the retained failure text or the explicit
                    // "unavailable" fallback — never a placeholder token.
                    change.set_display_status_cache("error");
                } else {
                    change.set_display_status_cache(normalized);
                }
            }
        }

        self.prune_stale_error_details_popup();
    }

    /// Close an Error Details popup whose change has left `error`.
    ///
    /// A retry is the common case: once the row is queued again, the popup would
    /// otherwise keep presenting a diagnostic that no longer describes the
    /// change's current state.
    fn prune_stale_error_details_popup(&mut self) {
        let Some(popup) = self.error_details_popup.as_ref() else {
            return;
        };

        let still_failed = self
            .changes
            .iter()
            .any(|change| change.id == popup.change_id && change.display_status_cache == "error");
        if !still_failed {
            self.error_details_popup = None;
        }
    }

    /// Sync retained final diagnostics for error rows from the reducer.
    ///
    /// Presentation only. The diagnostic is what the operator reads in the row
    /// preview and the Error Details popup; it never becomes scheduling, retry,
    /// acceptance, archive, or merge input.
    ///
    /// It is kept independent of the bounded log buffer on purpose: a change can
    /// stay in `error` long after the failure log entry has been evicted, so the
    /// row must be able to explain itself without any retained `LogEntry`.
    ///
    /// The reducer's diagnostic wins over whatever the row already cached. A row
    /// can reach `error` carrying an unrelated compatibility reason (a skipped
    /// dependency, for example), and that stale non-error text must not survive
    /// as the operator-facing failure reason. Rows the reducer does not report as
    /// failed are left alone: `set_display_status_cache` already drops the cache
    /// when a row transitions away from `error`.
    pub fn apply_error_details_from_reducer(&mut self, error_details: &HashMap<String, String>) {
        for change in &mut self.changes {
            if change.display_status_cache != "error" {
                continue;
            }
            if let Some(detail) = error_details.get(&change.id) {
                change.error_message_cache = Some(detail.clone());
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
    /// Modal popups live on a separate axis, so there is nothing to see through
    /// here: the execution mode is already the only thing the shared lifecycle
    /// matrix must see, and a popup can neither widen nor narrow it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn operator_mode(&self) -> crate::orchestration::operator_command::OperatorMode {
        self.execution_mode.operator_mode()
    }

    /// Adopt the Core process lifecycle mode.
    ///
    /// One direction only, and this is the direction. `execution_mode` used to
    /// be command-admission authority as well as presentation, which is how the
    /// live TUI and the `/api/v2` `app_mode` could describe the same process
    /// differently after an accepted command. Core owns admission now; this
    /// frontend renders it.
    pub fn adopt_core_mode(&mut self, mode: crate::orchestration::operator_command::OperatorMode) {
        self.execution_mode = AppExecutionMode::from_operator_mode(mode);
    }

    /// Project an accepted run dispatch onto TUI presentation state.
    ///
    /// The shared service already decided the targets and applied the reducer
    /// intent; this only refreshes the row cache and the run-scoped UI state so
    /// the screen matches what was actually dispatched.
    pub fn begin_run(&mut self, change_ids: &[String]) {
        self.admit_run_targets(change_ids);
        self.execution_mode = AppExecutionMode::Running;
    }

    /// Project an accepted Start that only *woke* a live scheduler.
    ///
    /// The queue intent is real and the rows are queued, but nothing has been
    /// admitted for execution yet: `SchedulerEffect::Notified` means the
    /// scheduler was notified, not that it started anything. Claiming Running
    /// here would make a Start against a persistent-idle scheduler report
    /// execution the operator cannot see, so the execution axis is left for the
    /// first typed work-start event to move.
    pub fn queue_run(&mut self, change_ids: &[String]) {
        self.admit_run_targets(change_ids);
    }

    /// Row cache and run-scoped presentation shared by both Start projections.
    fn admit_run_targets(&mut self, change_ids: &[String]) {
        for change in &mut self.changes {
            if change_ids.iter().any(|id| id == &change.id) {
                change.set_display_status_cache("queued");
            }
        }
        self.reset_for_run();
        // The service resolved these targets *from* the shared marks, so the row
        // projection is read back from the store rather than written into it.
        self.sync_execution_marks_from_store();
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

        // Handle buffer trimming when exceeding max entries. The sequence base
        // moves with the evicted entry so surviving anchors keep their identity.
        if log_logic::apply_log_buffer_limit(self.logs.len(), MAX_LOG_ENTRIES) {
            self.logs.remove(0);
            self.log_seq_base = self.log_seq_base.saturating_add(1);
        }

        // The anchor is stored in source coordinates, so an append or a trim
        // needs no adjustment here: the next projection re-derives the top line
        // from the current wrapped sequence, and auto-scroll (`log_anchor ==
        // None`) keeps following the newest line.
    }

    /// Record the Logs-panel geometry the renderer just used.
    ///
    /// Navigation needs the same width the renderer wrapped with, otherwise the
    /// display-line indices the two compute would disagree.
    pub(crate) fn set_log_viewport(&mut self, width: usize, height: usize) {
        self.log_viewport = log_logic::LogViewport { width, height };
    }

    /// Visible entries paired with their process-local sequence numbers.
    pub(crate) fn visible_log_entries(&self) -> Vec<(u64, &LogEntry)> {
        self.logs
            .iter()
            .enumerate()
            .filter(|(_, entry)| self.log_entry_visible_for_selected_proposal_filter(entry))
            .map(|(index, entry)| (self.log_seq_base + index as u64, entry))
            .collect()
    }

    /// Current wrapped display-line sequence for the Logs panel.
    pub(crate) fn log_display_lines(&self) -> Vec<log_logic::LogDisplayLine> {
        log_logic::build_log_display_lines(&self.visible_log_entries(), self.log_viewport.width)
    }

    /// Index of the top visible display line under the current anchor.
    pub(crate) fn log_start_line(&self, lines: &[log_logic::LogDisplayLine]) -> usize {
        log_logic::resolve_start_line(lines, self.log_anchor, self.log_viewport.visible_height())
    }

    /// Scroll logs up by a page of display lines (show older content).
    ///
    /// A page is measured in display lines, so a single entry taller than the
    /// viewport is traversed line by line instead of being skipped whole.
    pub fn scroll_logs_up(&mut self, page_size: usize) {
        let lines = self.log_display_lines();
        let start = self.log_start_line(&lines);
        let next = start.saturating_sub(page_size.max(1));
        self.log_anchor = log_logic::anchor_at_line(&lines, next);
        // Disable auto-scroll when user scrolls up
        self.log_auto_scroll = false;
    }

    /// Scroll logs down by a page of display lines (show newer content).
    pub fn scroll_logs_down(&mut self, page_size: usize) {
        let lines = self.log_display_lines();
        let max_start = lines
            .len()
            .saturating_sub(self.log_viewport.visible_height());
        let start = self.log_start_line(&lines);
        let next = start.saturating_add(page_size.max(1));

        // Re-enable auto-scroll once the newest line is back in view.
        if next >= max_start {
            self.scroll_logs_to_bottom();
        } else {
            self.log_anchor = log_logic::anchor_at_line(&lines, next);
            self.log_auto_scroll = false;
        }
    }

    /// Jump to the oldest display line (top of history)
    pub fn scroll_logs_to_top(&mut self) {
        let lines = self.log_display_lines();
        self.log_anchor = log_logic::anchor_at_line(&lines, 0);
        self.log_auto_scroll = false;
    }

    /// Jump to the newest display line (bottom) and re-enable auto-scroll
    pub fn scroll_logs_to_bottom(&mut self) {
        self.log_anchor = None;
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
    /// Filtering changes which entries are visible, so the current display-line
    /// anchor is no longer meaningful; return to the newest visible output with
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

pub(crate) mod guards {
    use super::{ChangeState, ParallelEligibility, TuiCommand, ViewMode, WorktreeInfo};
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

        // Branch name must not be empty
        if worktree.branch.is_empty() {
            return MergeGuardResult::Blocked("Cannot merge: no branch name".to_string());
        }

        // Cannot merge if already merging
        if worktree.is_merging {
            return MergeGuardResult::Blocked(
                "Cannot merge: merge already in progress".to_string(),
            );
        }

        // A row periodic refresh skipped carries no ahead/conflict evidence, so
        // there is nothing here to refuse *on*. Blocking it would make the
        // filtering permanently unmergeable; instead the request goes through
        // and the shared service decides from its own fresh targeted
        // observation of this exact worktree.
        if !worktree.inspection.is_inspected() {
            return MergeGuardResult::Allowed;
        }

        // Cannot merge if conflicts detected
        if worktree.has_merge_conflict() {
            return MergeGuardResult::Blocked(format!(
                "Cannot merge: {} conflict(s) detected",
                worktree.conflict_file_count()
            ));
        }

        // Cannot merge if no commits ahead of base branch
        if !worktree.has_commits_ahead {
            return MergeGuardResult::Blocked(
                "Cannot merge: no commits ahead of base branch".to_string(),
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
        /// A change with uncommitted proposal files cannot be queued.
        ParallelUncommitted,
        /// A change absent from the `HEAD` tree cannot be queued.
        ParallelProposalAbsent,
        /// Rejected proposals are read-only.
        Rejected,
    }

    /// Classifies why a change is not toggleable, or `None` when it is.
    ///
    /// This is the single source of truth shared by the single-row guard and
    /// the bulk toggle classification, so both paths stay consistent.
    pub fn classify_toggle_block(
        parallel_eligibility: ParallelEligibility,
        display_status_cache: &str,
    ) -> Option<ToggleBlockReason> {
        // Active (in-flight) changes can be stopped via Space key in Running mode
        // This is allowed and handled by handle_toggle_running_mode
        // No need to block here

        // Worktree execution refuses every ineligible change (only applies to
        // non-active states); the reason only decides what the operator is told.
        if !matches!(
            display_status_cache,
            "preparing" | "applying" | "accepting" | "archiving" | "resolving"
        ) {
            match parallel_eligibility {
                ParallelEligibility::UncommittedProposalFiles => {
                    return Some(ToggleBlockReason::ParallelUncommitted)
                }
                ParallelEligibility::ProposalAbsentFromHead => {
                    return Some(ToggleBlockReason::ParallelProposalAbsent)
                }
                ParallelEligibility::Eligible => {}
            }
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
        parallel_eligibility: ParallelEligibility,
        display_status_cache: &str,
        change_id: &str,
    ) -> ToggleGuardResult {
        match classify_toggle_block(parallel_eligibility, display_status_cache) {
            Some(ToggleBlockReason::ParallelUncommitted) => ToggleGuardResult::Blocked(format!(
                "Cannot queue uncommitted change '{}'. Commit it first.",
                change_id
            )),
            // No dirty content exists to commit here, so the message names the
            // condition that is actually observable instead of asking for one.
            Some(ToggleBlockReason::ParallelProposalAbsent) => ToggleGuardResult::Blocked(format!(
                "Cannot queue change '{}': it is not present in HEAD.",
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

    /// Buffer trimming evicts the anchored entry, so the anchor must clamp to
    /// the oldest surviving display line without resuming auto-scroll.
    #[test]
    fn trimming_the_anchored_entry_clamps_to_the_oldest_surviving_line() {
        let mut app = AppState::new(vec![create_test_change("alpha", 0, 1)]);
        app.logs.clear();
        app.set_log_viewport(60, 7);
        for index in 0..MAX_LOG_ENTRIES {
            app.add_log(LogEntry::info(format!("log {index}")));
        }

        app.scroll_logs_to_top();
        let anchored = app.log_anchor.unwrap();
        assert_eq!(anchored.entry_seq, 0);

        // Five more entries evict the five oldest, including the anchored one.
        for index in 0..5 {
            app.add_log(LogEntry::info(format!("overflow {index}")));
        }
        assert_eq!(app.logs.len(), MAX_LOG_ENTRIES);
        assert_eq!(app.logs[0].message, "log 5");

        // The anchor object is untouched, but it now resolves to the oldest
        // surviving line rather than to a stale index or to the newest output.
        assert_eq!(app.log_anchor, Some(anchored));
        let lines = app.log_display_lines();
        assert_eq!(app.log_start_line(&lines), 0);
        assert_eq!(lines[0].text, "log 5");
        assert!(!app.log_auto_scroll);
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
        app.scroll_logs_to_top();
        assert!(app.log_anchor.is_some());

        app.cursor_down();

        assert_eq!(app.selected_proposal_log_filter_target(), Some("beta"));
        assert_eq!(visible_filtered_messages(&app), vec!["beta apply"]);
        assert_eq!(app.log_anchor, None);
        assert!(app.log_auto_scroll);
    }

    #[test]
    fn cursor_move_keeps_log_position_when_filter_is_off() {
        let mut app = app_with_mixed_proposal_logs();
        app.scroll_logs_to_top();
        let anchored = app.log_anchor;
        assert!(anchored.is_some());

        app.cursor_down();

        assert_eq!(app.log_anchor, anchored);
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
        app.scroll_logs_to_top();
        assert!(app.log_anchor.is_some());

        app.toggle_selected_proposal_log_filter();

        assert_eq!(app.log_anchor, None);
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
            inspection: crate::worktree_ops::InspectionState::Checked,
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
            &command,
            Some(TuiCommand::DeleteWorktree(intent))
                if intent.path == *"/tmp/worktree-a"
                    && intent.branch == "feature-a"
                    && !intent.skip_teardown
                    && !intent.allow_known_dirty
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
            &command,
            Some(TuiCommand::DeleteWorktree(intent))
                if intent.path == *"/tmp/worktree-a"
                    && intent.branch == "feature-a"
                    && intent.skip_teardown
                    && !intent.allow_known_dirty
        ));
        assert!(app.logs.iter().any(|entry| {
            entry
                .message
                .contains("Deleting worktree with skip-teardown: /tmp/worktree-a")
        }));
    }

    #[test]
    fn a_worktree_without_a_branch_identity_can_never_produce_a_delete_command() {
        // Both ways an observation loses its identity, driven through the whole
        // request → confirm path: no modal opens, so no confirmation exists to
        // dispatch, and the delete marker is never taken either.
        let mut detached = create_test_worktree("/tmp/worktree-a", "feature-a", false);
        detached.is_detached = true;
        let nameless = create_test_worktree("/tmp/worktree-a", "", false);

        for (name, worktree) in [("detached", detached), ("empty-branch", nameless)] {
            let mut app = AppState::new(vec![]);
            app.view_mode = ViewMode::Worktrees;
            app.worktrees = vec![worktree];

            assert!(app.request_worktree_delete_from_list().is_none());

            assert!(
                app.modal.is_none(),
                "{name}: no confirmation may open without a revalidatable identity"
            );
            assert!(
                app.warning_message
                    .as_deref()
                    .is_some_and(|msg| msg.contains("no branch to confirm against")),
                "{name}: the operator must be told why: {:?}",
                app.warning_message
            );
            assert!(
                app.confirm_worktree_action_delete().is_none(),
                "{name}: confirming without a modal must not emit a delete command"
            );
            assert!(
                app.confirm_worktree_action_delete_with_options(true)
                    .is_none(),
                "{name}: skip-teardown confirm must not emit a delete command either"
            );
            assert!(
                !app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")),
                "{name}: a refused request must not mark the worktree deleting"
            );
        }
    }

    #[test]
    fn a_branch_bearing_worktree_beside_a_detached_one_stays_deletable() {
        // The identity guard refuses the target it cannot revalidate without
        // narrowing deletion for ordinary branch-bearing worktrees.
        let mut detached = create_test_worktree("/tmp/worktree-detached", "feature-a", false);
        detached.is_detached = true;
        let mut app = AppState::new(vec![]);
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![
            detached,
            create_test_worktree("/tmp/worktree-b", "feature-b", false),
        ];
        app.worktree_cursor_index = 1;

        app.request_worktree_delete_from_list();

        assert_eq!(
            app.modal,
            Some(ModalState::ConfirmWorktreeDelete {
                path: PathBuf::from("/tmp/worktree-b"),
                branch: "feature-b".to_string(),
            })
        );
        assert!(matches!(
            &app.confirm_worktree_action_delete(),
            Some(TuiCommand::DeleteWorktree(intent))
                if intent.path == std::path::Path::new("/tmp/worktree-b") && intent.branch == "feature-b"
        ));
    }

    /// A named way a dirty-discard target can drift before the confirmation.
    type DirtyDriftCase = (&'static str, fn(&mut AppState));

    fn dirty_target(path: &str, branch: &str) -> crate::worktree_ops::service::DirtyTarget {
        crate::worktree_ops::service::DirtyTarget {
            path: PathBuf::from(path),
            identity: format!("gitdir: {path}/.git"),
            branch: branch.to_string(),
            head: "abc123".to_string(),
        }
    }

    /// An app whose single worktree matches [`dirty_target`]'s observation.
    fn dirty_discard_app() -> AppState {
        let mut app = AppState::new(vec![create_test_change("feature-a", 0, 1)]);
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a", false)];
        app
    }

    #[test]
    fn tui_dirty_worktree_delete_escalation_captures_the_services_observation() {
        for skip_teardown in [false, true] {
            let mut app = dirty_discard_app();

            app.open_dirty_discard_confirmation(
                &dirty_target("/tmp/worktree-a", "feature-a"),
                skip_teardown,
            );

            // Every field comes from the service's fresh look, including the
            // identity and HEAD the projection in `worktrees` does not carry.
            assert_eq!(
                app.modal,
                Some(ModalState::ConfirmDirtyDiscard {
                    path: PathBuf::from("/tmp/worktree-a"),
                    identity: "gitdir: /tmp/worktree-a/.git".to_string(),
                    branch: "feature-a".to_string(),
                    head: "abc123".to_string(),
                    skip_teardown,
                })
            );
            assert!(
                !app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")),
                "escalating is not deleting: the marker must not be taken yet"
            );
            assert!(app
                .logs
                .iter()
                .any(|entry| entry.message.contains("press X to discard them")));
        }
    }

    #[test]
    fn tui_dirty_worktree_delete_confirmation_grants_discard_with_the_captured_teardown_bit() {
        for skip_teardown in [false, true] {
            let mut app = dirty_discard_app();
            app.open_dirty_discard_confirmation(
                &dirty_target("/tmp/worktree-a", "feature-a"),
                skip_teardown,
            );

            let command = app.confirm_dirty_discard();

            let Some(TuiCommand::DeleteWorktree(intent)) = command else {
                panic!("expected a delete command, got {command:?}");
            };
            assert_eq!(intent.path, PathBuf::from("/tmp/worktree-a"));
            assert_eq!(intent.branch, "feature-a");
            assert_eq!(
                intent.identity.as_deref(),
                Some("gitdir: /tmp/worktree-a/.git")
            );
            assert_eq!(intent.head.as_deref(), Some("abc123"));
            assert!(
                intent.allow_known_dirty,
                "uppercase X is what grants the discard permission"
            );
            assert_eq!(
                intent.skip_teardown, skip_teardown,
                "the teardown decision is the one taken in the ordinary confirmation"
            );
            assert!(app.modal.is_none());
            assert!(app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")));
        }
    }

    #[test]
    fn tui_dirty_worktree_delete_ordinary_confirmations_never_grant_discard() {
        for skip_teardown in [false, true] {
            let mut app = dirty_discard_app();
            app.request_worktree_delete_from_list();

            let command = app.confirm_worktree_action_delete_with_options(skip_teardown);

            let Some(TuiCommand::DeleteWorktree(intent)) = command else {
                panic!("expected a delete command, got {command:?}");
            };
            assert!(
                !intent.allow_known_dirty,
                "neither Y nor S may grant permission to discard uncommitted work"
            );
            assert!(
                !intent.allow_commits_ahead,
                "neither Y nor S may grant permission to discard unmerged commits"
            );
            assert_eq!(intent.skip_teardown, skip_teardown);
            assert_eq!(intent.identity, None);
            assert_eq!(intent.head, None);
        }
    }

    fn ahead_target(
        path: &str,
        branch: &str,
        dirty: bool,
    ) -> crate::worktree_ops::service::AheadTarget {
        crate::worktree_ops::service::AheadTarget {
            path: PathBuf::from(path),
            identity: format!("gitdir: {path}/.git"),
            branch: branch.to_string(),
            head: "abc123".to_string(),
            dirty,
        }
    }

    #[test]
    fn tui_ahead_worktree_delete_escalation_captures_the_services_observation() {
        for dirty in [false, true] {
            for skip_teardown in [false, true] {
                let mut app = dirty_discard_app();

                app.open_ahead_discard_confirmation(
                    &ahead_target("/tmp/worktree-a", "feature-a", dirty),
                    skip_teardown,
                );

                assert_eq!(
                    app.modal,
                    Some(ModalState::ConfirmAheadDiscard {
                        path: PathBuf::from("/tmp/worktree-a"),
                        identity: "gitdir: /tmp/worktree-a/.git".to_string(),
                        branch: "feature-a".to_string(),
                        head: "abc123".to_string(),
                        dirty,
                        skip_teardown,
                    })
                );
                assert!(
                    !app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")),
                    "escalating is not deleting: the marker must not be taken yet"
                );
                assert!(app
                    .logs
                    .iter()
                    .any(|entry| entry.message.contains("has unmerged commits")
                        && entry.message.contains("press X")));
            }
        }
    }

    #[test]
    fn tui_ahead_worktree_delete_confirmation_grants_exactly_what_it_disclosed() {
        for dirty in [false, true] {
            for skip_teardown in [false, true] {
                let mut app = dirty_discard_app();
                app.open_ahead_discard_confirmation(
                    &ahead_target("/tmp/worktree-a", "feature-a", dirty),
                    skip_teardown,
                );

                let command = app.confirm_ahead_discard();

                let Some(TuiCommand::DeleteWorktree(intent)) = command else {
                    panic!("expected a delete command, got {command:?}");
                };
                assert_eq!(intent.path, PathBuf::from("/tmp/worktree-a"));
                assert_eq!(intent.branch, "feature-a");
                assert_eq!(
                    intent.identity.as_deref(),
                    Some("gitdir: /tmp/worktree-a/.git")
                );
                assert_eq!(intent.head.as_deref(), Some("abc123"));
                assert!(
                    intent.allow_commits_ahead,
                    "uppercase X is what grants the ahead-discard permission"
                );
                assert_eq!(
                    intent.allow_known_dirty, dirty,
                    "dirty discard travels with X only when the modal disclosed that loss too"
                );
                assert_eq!(intent.skip_teardown, skip_teardown);
                assert!(app.modal.is_none());
                assert!(app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")));
            }
        }
    }

    #[test]
    fn tui_ahead_worktree_delete_cancellation_retains_the_worktree_and_branch() {
        let mut app = dirty_discard_app();
        app.open_ahead_discard_confirmation(
            &ahead_target("/tmp/worktree-a", "feature-a", true),
            false,
        );

        app.cancel_worktree_action();

        assert!(app.modal.is_none());
        assert!(
            !app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")),
            "cancelling must leave the worktree, its content, and its branch alone"
        );
        assert!(
            app.confirm_ahead_discard().is_none(),
            "confirming without a modal must not emit a delete command"
        );
    }

    #[test]
    fn tui_ahead_worktree_delete_refuses_a_target_that_drifted_before_dispatch() {
        let cases: [DirtyDriftCase; 5] = [
            ("absent", |app: &mut AppState| app.worktrees.clear()),
            ("main", |app: &mut AppState| app.worktrees[0].is_main = true),
            ("rebranded", |app: &mut AppState| {
                app.worktrees[0].branch = "feature-z".to_string()
            }),
            ("head-moved", |app: &mut AppState| {
                app.worktrees[0].head = "def456".to_string()
            }),
            ("active", |app: &mut AppState| {
                app.changes[0].set_display_status_cache("applying")
            }),
        ];

        for (name, mutate) in cases {
            let mut app = dirty_discard_app();
            app.open_ahead_discard_confirmation(
                &ahead_target("/tmp/worktree-a", "feature-a", false),
                false,
            );
            mutate(&mut app);

            assert!(
                app.confirm_ahead_discard().is_none(),
                "{name}: a drifted target must not be discarded"
            );
            assert!(
                app.modal.is_none(),
                "{name}: the stale modal must be cleared"
            );
            assert!(
                !app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")),
                "{name}: a refused discard must not mark the worktree deleting"
            );
        }
    }

    #[test]
    fn tui_ahead_and_dirty_confirmations_never_answer_for_each_other() {
        // Two overlays, two permissions. Pressing through one must not satisfy
        // the other, in either direction.
        let mut ahead = dirty_discard_app();
        ahead.open_ahead_discard_confirmation(
            &ahead_target("/tmp/worktree-a", "feature-a", true),
            false,
        );
        assert!(
            ahead.confirm_dirty_discard().is_none(),
            "the dirty confirmation must not act on an ahead modal"
        );
        assert!(ahead.modal.is_some(), "and must leave it standing");

        let mut dirty = dirty_discard_app();
        dirty.open_dirty_discard_confirmation(&dirty_target("/tmp/worktree-a", "feature-a"), false);
        assert!(
            dirty.confirm_ahead_discard().is_none(),
            "the ahead confirmation must not act on a dirty modal"
        );
        assert!(dirty.modal.is_some(), "and must leave it standing");
    }

    #[test]
    fn tui_dirty_worktree_delete_cancellation_retains_the_worktree() {
        let mut app = dirty_discard_app();
        app.open_dirty_discard_confirmation(&dirty_target("/tmp/worktree-a", "feature-a"), false);

        app.cancel_worktree_action();

        assert!(app.modal.is_none());
        assert!(
            !app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")),
            "cancelling must leave the worktree and its content alone"
        );
        assert!(
            app.confirm_dirty_discard().is_none(),
            "confirming without a modal must not emit a delete command"
        );
    }

    #[test]
    fn tui_dirty_worktree_delete_refuses_a_target_that_drifted_before_dispatch() {
        let cases: [DirtyDriftCase; 5] = [
            ("absent", |app: &mut AppState| app.worktrees.clear()),
            ("main", |app: &mut AppState| app.worktrees[0].is_main = true),
            ("rebranded", |app: &mut AppState| {
                app.worktrees[0].branch = "feature-z".to_string()
            }),
            ("head-moved", |app: &mut AppState| {
                app.worktrees[0].head = "def456".to_string()
            }),
            ("active", |app: &mut AppState| {
                app.changes[0].set_display_status_cache("applying")
            }),
        ];

        for (name, mutate) in cases {
            let mut app = dirty_discard_app();
            app.open_dirty_discard_confirmation(
                &dirty_target("/tmp/worktree-a", "feature-a"),
                false,
            );
            mutate(&mut app);

            assert!(
                app.confirm_dirty_discard().is_none(),
                "{name}: a drifted target must not be discarded"
            );
            assert!(
                app.modal.is_none(),
                "{name}: the stale modal must be cleared"
            );
            assert!(
                !app.is_worktree_deleting(&PathBuf::from("/tmp/worktree-a")),
                "{name}: a refused discard must not mark the worktree deleting"
            );
        }
    }

    #[test]
    fn worktree_delete_request_is_suppressed_while_marker_is_active() {
        let mut app = AppState::new(vec![]);
        let path = PathBuf::from("/tmp/worktree-a");
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a", false)];
        app.mark_worktree_deleting(path);

        app.request_worktree_delete_from_list();

        assert!(app.modal.is_none());
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
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
            apply_operation_cache: "apply".to_string(),
            apply_iteration_limit_active: false,
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

        assert_eq!(app.execution_mode, AppExecutionMode::Select);
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

        assert_eq!(app.execution_mode, AppExecutionMode::Select);
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
        assert_eq!(app.execution_mode, AppExecutionMode::Select);

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
        app.execution_mode = AppExecutionMode::Stopped;

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
    fn test_toggle_all_marks_excludes_uncommitted() {
        // Test that toggle all respects worktree eligibility restrictions
        let changes = vec![
            create_test_change("committed", 0, 1),
            create_test_change("uncommitted", 0, 1),
        ];

        let mut app = AppState::new(changes);
        app.execution_mode = AppExecutionMode::Select;

        // Mark first as committed, second as uncommitted
        app.changes[0].parallel_eligibility = ParallelEligibility::Eligible;
        app.changes[1].parallel_eligibility = ParallelEligibility::UncommittedProposalFiles;

        // Toggle all should only mark the committed change
        app.toggle_all_marks();
        assert!(app.changes[0].selected);
        assert!(!app.changes[1].selected); // Excluded: uncommitted

        // Toggle all again should unmark
        app.toggle_all_marks();
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);
    }

    /// Eligibility observation keeps the two Git facts apart.
    ///
    /// Both answers still refuse parallel admission; only the reason differs,
    /// and only one of them is a working-tree condition.
    #[test]
    fn parallel_eligibility_records_why_a_change_is_refused() {
        let changes = vec![
            create_test_change("clean", 0, 1),
            create_test_change("absent", 0, 1),
            create_test_change("dirty", 0, 1),
            create_test_change("brand-new", 0, 1),
        ];
        let mut app = AppState::new(changes);

        let committed = HashSet::from(["clean".to_string(), "dirty".to_string()]);
        let uncommitted = HashSet::from(["dirty".to_string(), "brand-new".to_string()]);
        app.apply_parallel_eligibility(&committed, &uncommitted);

        let by_id = |id: &str| {
            app.changes
                .iter()
                .find(|change| change.id == id)
                .unwrap_or_else(|| panic!("'{id}' must be tracked"))
                .clone()
        };

        assert_eq!(
            by_id("clean").parallel_eligibility,
            ParallelEligibility::Eligible
        );
        assert_eq!(
            by_id("absent").parallel_eligibility,
            ParallelEligibility::ProposalAbsentFromHead
        );
        assert_eq!(
            by_id("dirty").parallel_eligibility,
            ParallelEligibility::UncommittedProposalFiles
        );
        // Untracked and absent at once: committing it is what fixes both, so it
        // is reported as the condition the operator can act on.
        assert_eq!(
            by_id("brand-new").parallel_eligibility,
            ParallelEligibility::UncommittedProposalFiles
        );

        // The admission guard is unchanged: every non-eligible reason is false.
        assert!(by_id("clean").is_parallel_eligible());
        for refused in ["absent", "dirty", "brand-new"] {
            assert!(
                !by_id(refused).is_parallel_eligible(),
                "'{refused}' must stay out of parallel queueing"
            );
        }

        // Only observed dirty content may be presented as uncommitted state.
        assert!(!by_id("absent").has_uncommitted_proposal_files());
        assert!(by_id("dirty").has_uncommitted_proposal_files());

        // The shared runtime store carries the same reasons, not just the set.
        let runtime = app.parallel_runtime();
        assert_eq!(
            runtime.ineligible_ids(),
            vec![
                "absent".to_string(),
                "brand-new".to_string(),
                "dirty".to_string()
            ]
        );
        assert_eq!(
            runtime.eligibility("absent"),
            ParallelEligibility::ProposalAbsentFromHead
        );
        assert_eq!(
            runtime.eligibility("dirty"),
            ParallelEligibility::UncommittedProposalFiles
        );
        assert_eq!(runtime.eligibility("clean"), ParallelEligibility::Eligible);
    }

    /// Both refusals block the toggle; each one names what was observed.
    #[test]
    fn single_row_toggle_refusals_distinguish_dirty_content_from_absence() {
        let cases = [
            (
                ParallelEligibility::UncommittedProposalFiles,
                guards::ToggleBlockReason::ParallelUncommitted,
                "Commit it first",
            ),
            (
                ParallelEligibility::ProposalAbsentFromHead,
                guards::ToggleBlockReason::ParallelProposalAbsent,
                "not present in HEAD",
            ),
        ];

        for (eligibility, expected_reason, expected_text) in cases {
            assert_eq!(
                guards::classify_toggle_block(eligibility, "not queued"),
                Some(expected_reason),
                "{eligibility:?} must block the toggle"
            );

            let guards::ToggleGuardResult::Blocked(message) =
                guards::validate_change_toggleable(eligibility, "not queued", "change-a")
            else {
                panic!("{eligibility:?} must be refused");
            };
            assert!(
                message.contains(expected_text),
                "{eligibility:?} must be explained as '{expected_text}': {message}"
            );
        }

        // An absent proposal has nothing to commit, so it is never described as
        // uncommitted.
        let guards::ToggleGuardResult::Blocked(absent) = guards::validate_change_toggleable(
            ParallelEligibility::ProposalAbsentFromHead,
            "not queued",
            "change-a",
        ) else {
            panic!("an absent proposal must be refused");
        };
        assert!(!absent.to_lowercase().contains("uncommitted"), "{absent}");

        // An eligible row is never refused on eligibility grounds.
        assert_eq!(
            guards::classify_toggle_block(ParallelEligibility::Eligible, "not queued"),
            None
        );
        // An in-flight row is judged by its activity, not by eligibility.
        for eligibility in [
            ParallelEligibility::UncommittedProposalFiles,
            ParallelEligibility::ProposalAbsentFromHead,
        ] {
            assert_eq!(guards::classify_toggle_block(eligibility, "applying"), None);
        }
    }

    /// A bulk toggle reports each excluded row with the reason it was observed.
    #[test]
    fn bulk_toggle_exclusion_summary_names_each_parallel_reason() {
        let changes = vec![
            create_test_change("eligible", 0, 1),
            create_test_change("dirty", 0, 1),
            create_test_change("absent", 0, 1),
        ];
        let mut app = AppState::new(changes);
        app.execution_mode = AppExecutionMode::Select;
        app.apply_parallel_eligibility(
            &HashSet::from(["eligible".to_string(), "dirty".to_string()]),
            &HashSet::from(["dirty".to_string()]),
        );

        app.toggle_all_marks();

        assert!(app.changes[0].selected);
        assert!(!app.changes[1].selected, "a dirty proposal is excluded");
        assert!(!app.changes[2].selected, "an absent proposal is excluded");

        let summary = app
            .warning_message
            .clone()
            .expect("excluded rows are reported");
        assert!(
            summary.contains("uncommitted (commit first)"),
            "the dirty row keeps its actionable instruction: {summary}"
        );
        assert!(
            summary.contains("not present in HEAD (cannot queue)"),
            "the absent row is named for what it is: {summary}"
        );
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Select;

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
        app.execution_mode = AppExecutionMode::Stopped;

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
        mode: AppExecutionMode,
        rows: &[BulkToggleRow],
    ) -> (AppState, Vec<TuiCommand>) {
        let changes = rows
            .iter()
            .map(|(id, _, _, _)| create_test_change(id, 0, 1))
            .collect();
        let mut app = AppState::new(changes);
        app.execution_mode = mode;
        for (index, (_, status, parallel_eligible, selected)) in rows.iter().enumerate() {
            app.changes[index].display_status_cache = status.to_string();
            app.changes[index].parallel_eligibility = if *parallel_eligible {
                ParallelEligibility::Eligible
            } else {
                ParallelEligibility::UncommittedProposalFiles
            };
            app.changes[index].selected = *selected;
        }

        let commands = app.toggle_all_marks();
        (app, commands)
    }

    /// One table-driven bulk toggle regression case.
    struct BulkToggleCase {
        name: &'static str,
        mode: AppExecutionMode,
        rows: Vec<BulkToggleRow>,
        /// Expected `selected` flag for each row after the toggle.
        expected: Vec<bool>,
    }

    #[test]
    fn test_toggle_all_marks_mixed_eligible_and_ineligible_leaves_no_partial_eligible_rows() {
        let cases = vec![
            BulkToggleCase {
                name: "select mode marks every eligible row alongside a rejected row",
                mode: AppExecutionMode::Select,
                rows: vec![
                    ("eligible-marked", "not queued", true, true),
                    ("eligible-unmarked", "not queued", true, false),
                    ("rejected", "rejected", true, false),
                ],
                expected: vec![true, true, false],
            },
            BulkToggleCase {
                name: "select mode unmarks every eligible row when all are marked",
                mode: AppExecutionMode::Select,
                rows: vec![
                    ("eligible-a", "not queued", true, true),
                    ("eligible-b", "not queued", true, true),
                    ("rejected", "rejected", true, false),
                ],
                expected: vec![false, false, false],
            },
            BulkToggleCase {
                name: "stopped mode marks every eligible row alongside a rejected row",
                mode: AppExecutionMode::Stopped,
                rows: vec![
                    ("eligible-marked", "merge wait", true, true),
                    ("eligible-unmarked", "not queued", true, false),
                    ("rejected", "rejected", true, false),
                ],
                expected: vec![true, true, false],
            },
            BulkToggleCase {
                name: "running mode marks every eligible row and skips active rows",
                mode: AppExecutionMode::Running,
                rows: vec![
                    ("active", "applying", true, false),
                    ("eligible-marked", "merge wait", true, true),
                    ("eligible-unmarked", "not queued", true, false),
                    ("rejected", "rejected", true, false),
                ],
                expected: vec![false, true, true, false],
            },
            BulkToggleCase {
                name: "worktree execution marks committed rows and skips uncommitted rows",
                mode: AppExecutionMode::Select,
                rows: vec![
                    ("committed-marked", "not queued", true, true),
                    ("committed-unmarked", "not queued", true, false),
                    ("uncommitted", "not queued", false, false),
                ],
                expected: vec![true, true, false],
            },
        ];

        for case in cases {
            let (app, _) = run_bulk_toggle_case(case.mode, &case.rows);

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
            AppExecutionMode::Running,
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
            AppExecutionMode::Select,
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
            AppExecutionMode::Running,
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
        app.execution_mode = AppExecutionMode::Select;

        let commands = app.toggle_all_marks();

        assert!(commands.is_empty());
        assert!(app
            .warning_message
            .as_ref()
            .is_some_and(|msg| msg.contains("no changes to toggle")));
    }

    #[test]
    fn test_toggle_all_marks_in_error_mode_reports_retry_ownership() {
        let (app, commands) =
            run_bulk_toggle_case(AppExecutionMode::Error, &[("a", "error", true, false)]);

        assert!(commands.is_empty());
        assert!(!app.changes[0].selected);
        let message = app.warning_message.clone().expect("rejection is reported");
        assert!(
            message.contains("Error mode") && message.contains("retry"),
            "Error rejection must name the execution mode and retry ownership: {message}"
        );
    }

    #[test]
    fn test_toggle_all_marks_in_stopping_mode_reports_immutability() {
        let (app, commands) = run_bulk_toggle_case(
            AppExecutionMode::Stopping,
            &[("a", "not queued", true, false)],
        );

        assert!(commands.is_empty());
        assert!(!app.changes[0].selected);
        let message = app.warning_message.clone().expect("rejection is reported");
        assert!(
            message.contains("Stopping mode") && message.contains("immutable"),
            "Stopping rejection must name the execution mode and immutability: {message}"
        );
    }

    /// The original incident: a fatal `ExecutionEvent::Error` put the TUI in
    /// `Error`, and `x` reported a generic mode list that named modal variants as
    /// execution states. Rejection text must now name only execution modes.
    #[test]
    fn bulk_mark_rejection_never_describes_a_modal_as_an_execution_mode() {
        for mode in [AppExecutionMode::Stopping, AppExecutionMode::Error] {
            let (app, commands) = run_bulk_toggle_case(mode, &[("a", "not queued", true, false)]);

            assert!(commands.is_empty());
            let message = app.warning_message.clone().expect("rejection is reported");
            for modal_word in ["QR", "Confirm", "popup", "ConfirmForceKill"] {
                assert!(
                    !message.contains(modal_word),
                    "{mode:?} rejection must not mention modal presentation: {message}"
                );
            }
        }
    }

    #[test]
    fn bulk_mark_admission_is_derived_from_the_shared_lifecycle_matrix() {
        use crate::orchestration::operator_command::{classify_mark_route, MarkRoute};

        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            let shared_admits = !matches!(
                classify_mark_route(mode.operator_mode(), "not queued"),
                MarkRoute::Immutable | MarkRoute::RetryRequired
            );
            let mut app = AppState::new(vec![create_test_change("a", 0, 1)]);
            app.execution_mode = mode;

            assert_eq!(
                app.has_bulk_toggle_targets(),
                shared_admits,
                "{mode:?} must follow the shared operator lifecycle matrix"
            );
        }
    }

    #[test]
    fn bulk_mark_requires_changes_view_without_an_overlay() {
        let mut app = AppState::new(vec![create_test_change("a", 0, 1)]);
        app.execution_mode = AppExecutionMode::Select;
        assert!(app.has_bulk_toggle_targets());

        app.view_mode = ViewMode::Worktrees;
        assert!(!app.has_bulk_toggle_targets());

        app.view_mode = ViewMode::Changes;
        app.web_url = Some("http://127.0.0.1:8080".to_string());
        app.show_qr_popup();
        assert!(!app.has_bulk_toggle_targets());
        assert!(
            app.toggle_all_marks().is_empty(),
            "an overlay owning input must not let `x` mutate marks"
        );
        assert!(!app.changes[0].selected);

        app.modal = None;
        app.show_warning_popup("warning", "diagnostic");
        assert!(!app.has_bulk_toggle_targets());
        assert!(app.toggle_all_marks().is_empty());
        assert!(!app.changes[0].selected);
    }

    #[test]
    fn test_toggle_all_marks_running_partial_selection_emits_command_for_every_eligible_row() {
        let (app, commands) = run_bulk_toggle_case(
            AppExecutionMode::Running,
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;

        let command = app.toggle_selection();
        assert!(matches!(command, Some(TuiCommand::AddToQueue(ref id)) if id == "dynamic-change"));
        assert!(app.changes[0].selected);
        // The interaction records a target-scoped write; the run loop applies it
        // through the shared service before the next event is projected.
        app.flush_pending_mark_writes();

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

    /// Reducer `queued` is queue presentation, not an execution mark.
    ///
    /// Synthesizing a mark from queue status used to invent operator intent the
    /// shared store never recorded, which `/api/v2` then published as
    /// `execution_marked` for a change nobody marked. A queued row is unmarked
    /// until the store says otherwise, and unqueueing a *marked* queued row still
    /// takes the queue intent back out.
    #[test]
    fn reducer_queued_status_never_synthesizes_an_execution_mark() {
        let changes = vec![create_test_change("queued-change", 0, 1)];
        let mut app = AppState::new(changes);
        app.execution_mode = AppExecutionMode::Running;

        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "queued-change".to_string(),
            "queued",
        )]));

        assert_eq!(app.changes[0].display_status_cache, "queued");
        assert!(
            !app.changes[0].selected,
            "queue intent must not become an execution mark"
        );
        assert!(
            app.execution_marks().marked_ids().is_empty(),
            "queue intent must not reach the shared mark store"
        );

        // An operator mark on the same row is a separate fact, and unmarking it
        // still withdraws the queue intent.
        app.execution_marks().set("queued-change", true);
        app.sync_execution_marks_from_store();
        assert!(app.changes[0].selected);

        let command = app.toggle_selection();
        assert!(
            matches!(command, Some(TuiCommand::RemoveFromQueue(ref id)) if id == "queued-change")
        );
        assert!(!app.changes[0].selected);
        app.flush_pending_mark_writes();
        assert!(app.execution_marks().marked_ids().is_empty());
    }

    #[test]
    fn active_status_survives_reducer_sync_and_changes_refreshed() {
        let changes = vec![create_test_change("active-change", 0, 1)];
        let mut app = AppState::new(changes.clone());
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].set_display_status_cache("queued");
        app.changes[0].selected = true;
        app.publish_execution_marks();

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
        app.execution_mode = AppExecutionMode::Stopped;
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

    // ------------------------------------------------------------------
    // Retained final diagnostics for error rows
    // ------------------------------------------------------------------

    /// Build a reducer holding one change that failed with `error`.
    ///
    /// The reducer is a pure in-memory state machine, so driving it with its own
    /// event is unit-scoped: no process, filesystem, VCS, or clock boundary is
    /// touched.
    fn reducer_with_failed_change(
        change_id: &str,
        error: &str,
    ) -> crate::orchestration::state::OrchestratorState {
        let mut state =
            crate::orchestration::state::OrchestratorState::new(vec![change_id.to_string()], 1);
        state.apply_execution_event(&crate::events::ExecutionEvent::ApplyFailed {
            change_id: change_id.to_string(),
            error: error.to_string(),
        });
        state
    }

    /// A direct failure event the TUI observed itself must keep the whole
    /// diagnostic, not a summary of it.
    #[test]
    fn direct_failure_events_retain_the_full_final_diagnostic() {
        let diagnostic = "Apply failed: stalled after 5 empty WIP commits";
        for (label, apply) in [
            (
                "processing error",
                Box::new(|app: &mut AppState, detail: String| {
                    app.handle_orchestrator_event(OrchestratorEvent::ProcessingError {
                        id: "test-change".to_string(),
                        error: detail,
                    })
                }) as Box<dyn Fn(&mut AppState, String)>,
            ),
            (
                "apply failed",
                Box::new(|app: &mut AppState, detail: String| {
                    app.handle_orchestrator_event(OrchestratorEvent::ApplyFailed {
                        change_id: "test-change".to_string(),
                        error: detail,
                    })
                }),
            ),
            (
                "archive failed",
                Box::new(|app: &mut AppState, detail: String| {
                    app.handle_orchestrator_event(OrchestratorEvent::ArchiveFailed {
                        change_id: "test-change".to_string(),
                        error: detail,
                        reason: None,
                        summary: None,
                    })
                }),
            ),
        ] {
            let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
            apply(&mut app, diagnostic.to_string());

            assert_eq!(app.changes[0].display_status_cache, "error", "{label}");
            assert_eq!(
                app.changes[0].error_message_cache.as_deref(),
                Some(diagnostic),
                "{label} must retain the complete diagnostic"
            );
        }
    }

    /// The reducer's retained diagnostic is what an operator reads, even when a
    /// row already cached an unrelated compatibility reason before failing.
    #[test]
    fn reducer_error_detail_replaces_a_stale_non_error_reason() {
        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        // `ChangeSkipped` leaves a dependency note behind on the row.
        app.handle_orchestrator_event(OrchestratorEvent::ChangeSkipped {
            change_id: "test-change".to_string(),
            reason: "Dependency 'other-change' failed".to_string(),
        });
        assert_eq!(
            app.changes[0].error_message_cache.as_deref(),
            Some("Dependency 'other-change' failed")
        );

        let reducer = reducer_with_failed_change("test-change", "Apply failed: stalled");
        app.apply_display_statuses_from_reducer(&reducer.all_display_statuses());
        app.apply_error_details_from_reducer(&reducer.all_error_details());

        assert_eq!(app.changes[0].display_status_cache, "error");
        assert_eq!(
            app.changes[0].error_message_cache.as_deref(),
            Some("Apply failed: stalled"),
            "the reducer's retained failure must replace the stale skip reason"
        );
    }

    /// A row that reaches `error` only through the reducer must never show a
    /// placeholder token; it shows the retained diagnostic instead.
    #[test]
    fn reducer_only_error_row_never_resolves_to_a_placeholder() {
        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        let reducer = reducer_with_failed_change("test-change", "Apply failed: stalled");

        app.apply_display_statuses_from_reducer(&reducer.all_display_statuses());
        app.apply_error_details_from_reducer(&reducer.all_error_details());

        assert_eq!(app.changes[0].display_status_cache, "error");
        assert_eq!(
            app.changes[0].error_message_cache.as_deref(),
            Some("Apply failed: stalled")
        );
        assert_ne!(
            app.changes[0].error_message_cache.as_deref(),
            Some("reducer")
        );
    }

    /// The diagnostic a TUI row shows and the one `/api/v2` projects are the
    /// same reducer-owned text, so an operator reading either surface reads the
    /// same failure.
    #[test]
    fn tui_error_detail_matches_the_api_projected_error_detail() {
        let reducer = reducer_with_failed_change("test-change", "Apply failed: \u{1b}[31mstalled");
        let projected = reducer
            .change_runtime("test-change")
            .and_then(crate::orchestration::state::ChangeRuntimeState::error_message)
            .map(crate::events::sanitize_detail);

        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        app.apply_display_statuses_from_reducer(&reducer.all_display_statuses());
        app.apply_error_details_from_reducer(&reducer.all_error_details());

        assert_eq!(app.changes[0].error_message_cache, projected);
    }

    /// With no reducer-retained diagnostic there is nothing to adopt; the row
    /// keeps an empty cache so rendering can state that details are unavailable.
    #[test]
    fn missing_reducer_error_detail_leaves_the_cache_empty() {
        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "test-change".to_string(),
            "error",
        )]));
        app.apply_error_details_from_reducer(&HashMap::new());

        assert_eq!(app.changes[0].display_status_cache, "error");
        assert_eq!(app.changes[0].error_message_cache, None);
    }

    /// Leaving `error` drops the diagnostic, and a later reducer sync must not
    /// resurrect it onto the now-healthy row.
    #[test]
    fn transition_away_from_error_clears_the_retained_diagnostic() {
        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        app.changes[0].set_error_message_cache("Apply failed: stalled".to_string());

        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "test-change".to_string(),
            "queued",
        )]));
        assert_eq!(app.changes[0].display_status_cache, "queued");
        assert_eq!(app.changes[0].error_message_cache, None);

        // A stale detail map from before the retry must not repaint the row.
        app.apply_error_details_from_reducer(&HashMap::from([(
            "test-change".to_string(),
            "Apply failed: stalled".to_string(),
        )]));
        assert_eq!(app.changes[0].error_message_cache, None);
    }

    /// A retry closes an open Error Details popup rather than leaving it
    /// describing a state the change has already left.
    #[test]
    fn leaving_error_closes_an_open_error_details_popup() {
        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        app.changes[0].set_error_message_cache("Apply failed: stalled".to_string());
        assert!(app.open_error_details_popup());

        // Still failing: the popup survives an ordinary sync.
        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "test-change".to_string(),
            "error",
        )]));
        assert!(app.error_details_popup.is_some());

        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "test-change".to_string(),
            "queued",
        )]));
        assert!(app.error_details_popup.is_none());
    }

    /// Opening, scrolling, copying, and closing the popup are presentation only.
    #[test]
    fn error_details_popup_is_presentation_only() {
        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        app.changes[0].set_error_message_cache("Apply failed: stalled".to_string());
        app.changes[0].selected = true;

        assert!(app.open_error_details_popup());
        app.scroll_error_details_popup(5);
        app.scroll_error_details_popup(-99);
        assert_eq!(
            app.error_details_popup.as_ref().map(|popup| popup.scroll),
            Some(0),
            "the scroll offset stays non-negative"
        );
        app.close_error_details_popup();

        assert_eq!(app.changes[0].display_status_cache, "error");
        assert!(app.changes[0].selected);
        assert_eq!(app.execution_mode, AppExecutionMode::Select);
        assert!(app.modal.is_none());
    }

    /// Only an `error` row can open the popup.
    #[test]
    fn a_non_error_row_cannot_open_the_error_details_popup() {
        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        for status in ["not queued", "queued", "applying", "blocked", "archived"] {
            app.changes[0].set_display_status_cache(status);
            assert!(
                !app.open_error_details_popup(),
                "{status} must not open the Error Details popup"
            );
            assert!(app.error_details_popup.is_none());
        }
    }

    /// An error row with no retained diagnostic still opens, stating that the
    /// details are unavailable rather than showing an empty popup.
    #[test]
    fn error_details_popup_states_when_no_diagnostic_is_available() {
        let mut app = AppState::new(vec![create_test_change("test-change", 0, 1)]);
        app.changes[0].set_display_status_cache("error");

        assert!(app.open_error_details_popup());
        assert_eq!(
            app.error_details_popup
                .as_ref()
                .map(|popup| popup.error.as_str()),
            Some(ERROR_DETAILS_UNAVAILABLE)
        );
    }

    /// The copied text is stable by contract.
    #[test]
    fn error_details_clipboard_text_is_stable_plain_text() {
        let popup = ErrorDetailsPopup {
            change_id: "alpha".to_string(),
            error: "Apply failed: stalled".to_string(),
            scroll: 0,
            copy_feedback: None,
        };

        assert_eq!(
            popup.clipboard_text(),
            "Change: alpha\nError: Apply failed: stalled"
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
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
            apply_operation_cache: "apply".to_string(),
            apply_iteration_limit_active: false,
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
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: Some(3),
            apply_operation_cache: "apply".to_string(),
            apply_iteration_limit_active: false,
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
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: Some(2),
            apply_operation_cache: "apply".to_string(),
            apply_iteration_limit_active: false,
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
        app.handle_resolve_completed("change-a".to_string(), None);

        // change-b is promoted in the ledger; the reducer-owned ResolveWait it
        // already carries is what the scheduler dispatches from, so promotion
        // emits no command.
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
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
        app.handle_resolve_completed("change-a".to_string(), None);

        // is_resolving should be cleared
        assert!(!app.is_resolving());
    }

    #[test]
    fn test_resolve_completed_does_not_log_duplicate_after_merge_completed() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].display_status_cache = "merged".to_string();

        app.handle_resolve_completed("change-a".to_string(), None);

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
        app.execution_mode = AppExecutionMode::Running;

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
        app.execution_mode = AppExecutionMode::Running;

        assert!(app.resolve_merge().is_none());
    }

    #[test]
    fn m_offers_the_same_command_in_every_mode_that_allows_it() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Stopped,
            AppExecutionMode::Running,
        ] {
            let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
            app.changes[0].display_status_cache = "merge wait".to_string();
            app.cursor_index = 0;
            app.execution_mode = mode;

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
        app.execution_mode = AppExecutionMode::Running;

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
        app.execution_mode = AppExecutionMode::Running;

        // What the shared run-control service does when it accepts the resolve:
        // record the reducer intent, then let the adapter advance the row.
        shared.blocking_write().apply_command(
            crate::orchestration::state::ReducerCommand::ResolveMerge("change-b".to_string()),
        );
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
        app.execution_mode = AppExecutionMode::Running;
        app.clear_resolving();

        // What the shared run-control service does when it accepts the resolve.
        shared.blocking_write().apply_command(
            crate::orchestration::state::ReducerCommand::ResolveMerge("change-a".to_string()),
        );
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
    fn test_handle_orchestrator_event_merge_completed_promotes_queued_resolve() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);
        app.execution_mode = AppExecutionMode::Running;
        app.set_resolving("__active__");
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "resolve pending".to_string();
        app.add_to_resolve_queue("change-b");

        // The promoted change is dispatched from the reducer-owned ResolveWait it
        // already holds, so the event path emits no command for it: a
        // re-submission could only be refused as no longer `merge wait`.
        app.handle_orchestrator_event(OrchestratorEvent::MergeCompleted {
            change_id: "change-a".to_string(),
            revision: "abc123".to_string(),
        });

        assert!(!app.is_resolving());
        assert!(app.queued_resolves().is_empty());
        assert!(app.warning_message.is_none());
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
        app.publish_execution_marks();

        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "not queued");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(
            app.changes[0].display_status_cache, "rejected",
            "rejected row must stay immutable during reducer display sync"
        );
        assert!(
            app.changes[0].selected,
            "a display-status sync must not decide mark truth; only the shared store does"
        );

        // The authoritative revocation is what clears it, and the row follows.
        app.execution_marks().set("change-a", false);
        app.sync_execution_marks_from_store();
        assert!(!app.changes[0].selected);
    }

    #[test]
    fn test_update_changes_reactivates_rejected_row_when_marker_removed() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes.clone());
        app.changes[0].display_status_cache = "rejected".to_string();

        app.update_changes_with_rejected_for_test(changes, Vec::new());

        assert_eq!(
            app.changes[0].display_status_cache, "not queued",
            "active refresh without marker should reactivate rejected row"
        );
        assert!(
            !app.changes[0].selected,
            "reactivated row must remain unselected until explicit user action"
        );
        assert!(
            app.execution_marks().marked_ids().is_empty(),
            "reactivation must not invent operator intent"
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
        app.execution_mode = AppExecutionMode::Select;

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
        app.execution_mode = AppExecutionMode::Select;
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

    /// A rejection clears the mark through the authoritative store, not through
    /// the display-status sync.
    ///
    /// The sync is presentation: it renders the reducer's word for the row. The
    /// mark it renders alongside comes from `ExecutionMarkStore`, which the
    /// dispatch boundary already reconciled for the same rejection event.
    #[test]
    fn test_apply_display_statuses_rejected_follows_the_shared_mark_store() {
        let changes = vec![create_test_change("change-a", 1, 1)];
        let mut app = AppState::new(changes);
        app.changes[0].selected = true;
        app.publish_execution_marks();

        let mut display_map = std::collections::HashMap::new();
        display_map.insert("change-a".to_string(), "rejected");
        app.apply_display_statuses_from_reducer(&display_map);

        assert_eq!(app.changes[0].display_status_cache, "rejected");

        app.execution_marks().set("change-a", false);
        app.sync_execution_marks_from_store();
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
        app.execution_mode = AppExecutionMode::Running;

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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.execution_mode = AppExecutionMode::Running;
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
        app.changes[0].parallel_eligibility = ParallelEligibility::Eligible;

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
        app.changes[0].parallel_eligibility = ParallelEligibility::Eligible;

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
            *guard = OrchestratorState::new(vec!["change-a".to_string()], 0);
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
        app.changes[0].parallel_eligibility = ParallelEligibility::Eligible;
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
        app.changes[0].parallel_eligibility = ParallelEligibility::Eligible;
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
        app.execution_mode = AppExecutionMode::Running;

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
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "resolving".to_string();

        app.handle_all_completed();

        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Select,
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
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "merged".to_string(); // already done
        app.changes[1].display_status_cache = "resolving".to_string();
        app.set_resolving("__active__");

        // Simulate resolve completion
        app.handle_resolve_completed("change-b".to_string(), None);

        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Select,
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
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "applying".to_string(); // still active
        app.changes[1].display_status_cache = "resolving".to_string();
        app.set_resolving("__active__");

        app.handle_resolve_completed("change-b".to_string(), None);

        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Running,
            "Should stay Running when other active changes remain"
        );
    }

    // ========================================================================
    // Execution / modal state separation
    // ========================================================================

    const ALL_EXECUTION_MODES: [AppExecutionMode; 5] = [
        AppExecutionMode::Select,
        AppExecutionMode::Running,
        AppExecutionMode::Stopping,
        AppExecutionMode::Stopped,
        AppExecutionMode::Error,
    ];

    fn modal_app() -> AppState {
        let mut app = AppState::new(vec![create_test_change("change-a", 0, 1)]);
        app.web_url = Some("http://127.0.0.1:8080".to_string());
        app.worktrees = vec![create_test_worktree("/tmp/wt-a", "change-a", false)];
        app
    }

    #[test]
    fn new_state_starts_in_select_execution_with_no_modal() {
        let app = AppState::new(vec![create_test_change("change-a", 0, 1)]);

        assert_eq!(app.execution_mode, AppExecutionMode::Select);
        assert_eq!(app.modal, None);
        assert!(!app.has_overlay());
    }

    #[test]
    fn qr_round_trip_preserves_the_execution_mode_it_opened_over() {
        for mode in ALL_EXECUTION_MODES {
            let mut app = modal_app();
            app.execution_mode = mode;

            app.show_qr_popup();
            assert_eq!(app.modal, Some(ModalState::QrPopup));
            assert_eq!(
                app.execution_mode, mode,
                "opening QR must not capture or replace the execution mode"
            );

            app.hide_qr_popup();
            assert_eq!(app.modal, None);
            assert_eq!(
                app.execution_mode, mode,
                "closing QR must not restore a captured execution mode"
            );
        }
    }

    #[test]
    fn qr_survives_a_background_execution_transition_and_exposes_the_latest_mode() {
        let mut app = modal_app();
        app.execution_mode = AppExecutionMode::Running;
        app.show_qr_popup();

        for mode in [
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            app.execution_mode = mode;
            assert_eq!(app.revalidate_modal(), None);
            assert_eq!(app.modal, Some(ModalState::QrPopup));
        }

        app.hide_qr_popup();
        assert_eq!(app.execution_mode, AppExecutionMode::Error);
    }

    #[test]
    fn qr_is_not_opened_while_web_monitoring_is_disabled() {
        let mut app = modal_app();
        app.web_url = None;

        app.show_qr_popup();

        assert_eq!(app.modal, None);
        assert_eq!(app.execution_mode, AppExecutionMode::Select);
    }

    #[test]
    fn worktree_confirmation_carries_the_identity_it_was_opened_with() {
        let mut app = modal_app();
        app.view_mode = ViewMode::Worktrees;

        assert!(app.request_worktree_delete_from_list().is_none());

        assert_eq!(
            app.modal,
            Some(ModalState::ConfirmWorktreeDelete {
                path: PathBuf::from("/tmp/wt-a"),
                branch: "change-a".to_string(),
            })
        );
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Select,
            "opening a confirmation must not move the execution axis"
        );
    }

    #[test]
    fn cancelling_a_confirmation_leaves_the_latest_execution_mode_in_place() {
        let mut app = modal_app();
        app.view_mode = ViewMode::Worktrees;
        app.request_worktree_delete_from_list();

        // A background transition lands while the confirmation is visible.
        app.execution_mode = AppExecutionMode::Stopping;
        app.cancel_worktree_action();

        assert_eq!(app.modal, None);
        assert_eq!(app.execution_mode, AppExecutionMode::Stopping);
    }

    #[test]
    fn a_stale_worktree_confirmation_is_refused_and_cleared_atomically() {
        let mut app = modal_app();
        app.view_mode = ViewMode::Worktrees;
        app.request_worktree_delete_from_list();

        // The path now carries a different branch identity.
        app.worktrees[0].branch = "change-z".to_string();

        assert!(app.confirm_worktree_action_delete().is_none());
        assert_eq!(app.modal, None, "modal and payload are cleared together");
        assert!(!app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")));
        assert!(app
            .warning_message
            .as_ref()
            .is_some_and(|msg| msg.contains("Worktree delete canceled")));
    }

    #[test]
    fn force_kill_confirmation_opens_only_for_active_work_in_running_execution() {
        // Not Running: no confirmation.
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            let mut app = modal_app();
            app.execution_mode = mode;
            app.changes[0].set_display_status_cache("applying");
            assert!(!app.request_force_kill_confirmation(), "{mode:?}");
            assert_eq!(app.modal, None);
        }

        // Running but the row is not active: no confirmation.
        let mut idle = modal_app();
        idle.execution_mode = AppExecutionMode::Running;
        assert!(!idle.request_force_kill_confirmation());
        assert_eq!(idle.modal, None);

        // Running with active work: confirmation carries the target identity.
        let mut active = modal_app();
        active.execution_mode = AppExecutionMode::Running;
        active.changes[0].set_display_status_cache("applying");
        assert!(active.request_force_kill_confirmation());
        assert_eq!(
            active.modal,
            Some(ModalState::ConfirmForceKill {
                change_id: "change-a".to_string()
            })
        );
        assert_eq!(active.execution_mode, AppExecutionMode::Running);
    }

    #[test]
    fn force_kill_survives_running_to_stopping_and_cancel_preserves_stopping() {
        let mut app = modal_app();
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].set_display_status_cache("applying");
        app.request_force_kill_confirmation();

        app.execution_mode = AppExecutionMode::Stopping;
        assert_eq!(app.revalidate_modal(), None);
        assert!(app.modal.is_some());

        app.cancel_force_kill();
        assert_eq!(app.modal, None);
        assert_eq!(app.execution_mode, AppExecutionMode::Stopping);
    }

    #[test]
    fn revalidation_never_mutates_execution_state() {
        for mode in ALL_EXECUTION_MODES {
            let mut app = modal_app();
            app.execution_mode = mode;
            app.modal = Some(ModalState::ConfirmForceKill {
                change_id: "gone".to_string(),
            });

            app.revalidate_modal();

            assert_eq!(app.modal, None);
            assert_eq!(
                app.execution_mode, mode,
                "clearing an invalid modal must leave execution untouched"
            );
        }
    }
}
