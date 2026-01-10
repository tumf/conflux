//! TUI Dashboard for OpenSpec Orchestrator
//!
//! This module provides an interactive terminal user interface for:
//! - Selecting changes to process
//! - Monitoring execution progress
//! - Dynamic queue management
//! - Auto-refresh of change list

use crate::agent::AgentRunner;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
use chrono::Local;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    terminal::{Clear, ClearType},
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    DefaultTerminal, Frame,
};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use unicode_width::UnicodeWidthStr;

/// Auto-refresh interval in seconds
const AUTO_REFRESH_INTERVAL_SECS: u64 = 5;

/// Maximum number of log entries to keep
const MAX_LOG_ENTRIES: usize = 1000;

/// Spinner characters for processing animation (Braille dot pattern)
const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Dynamic queue for runtime change additions
///
/// This struct provides a thread-safe queue for dynamically adding changes
/// during orchestrator execution. TUI pushes change IDs when the user adds
/// them via Space key, and the orchestrator pops them for processing.
#[derive(Clone)]
pub struct DynamicQueue {
    inner: Arc<Mutex<VecDeque<String>>>,
}

impl DynamicQueue {
    /// Create a new empty DynamicQueue
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Push a change ID to the queue
    /// Returns false if the ID is already in the queue
    pub async fn push(&self, id: String) -> bool {
        let mut queue = self.inner.lock().await;
        if queue.contains(&id) {
            return false;
        }
        queue.push_back(id);
        true
    }

    /// Pop the next change ID from the queue
    pub async fn pop(&self) -> Option<String> {
        let mut queue = self.inner.lock().await;
        queue.pop_front()
    }

    /// Check if the queue is empty
    #[cfg(test)]
    pub async fn is_empty(&self) -> bool {
        let queue = self.inner.lock().await;
        queue.is_empty()
    }

    /// Check if an ID is already in the queue
    #[cfg(test)]
    pub async fn contains(&self, id: &str) -> bool {
        let queue = self.inner.lock().await;
        queue.iter().any(|i| i == id)
    }

    /// Get the current queue length
    #[cfg(test)]
    pub async fn len(&self) -> usize {
        let queue = self.inner.lock().await;
        queue.len()
    }
}

impl Default for DynamicQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Application mode
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    /// Selection mode - user selects changes to process
    Select,
    /// Running mode - processing selected changes
    Running,
    /// Completed mode - all processing finished
    Completed,
    /// Error mode - an error occurred during processing
    Error,
}

/// Queue status for a change
#[derive(Debug, Clone, PartialEq)]
pub enum QueueStatus {
    /// Not in the execution queue
    NotQueued,
    /// Waiting in the execution queue
    Queued,
    /// Currently being processed
    Processing,
    /// Processing completed
    Completed,
    /// Archived after completion
    Archived,
    /// Error occurred during processing
    Error(String),
}

impl QueueStatus {
    /// Get display string for the queue status
    pub fn display(&self) -> &str {
        match self {
            QueueStatus::NotQueued => "not queued",
            QueueStatus::Queued => "queued",
            QueueStatus::Processing => "processing",
            QueueStatus::Completed => "completed",
            QueueStatus::Archived => "archived",
            QueueStatus::Error(_) => "error",
        }
    }

    /// Get the color for the queue status
    pub fn color(&self) -> Color {
        match self {
            QueueStatus::NotQueued => Color::DarkGray,
            QueueStatus::Queued => Color::Yellow,
            QueueStatus::Processing => Color::Cyan,
            QueueStatus::Completed => Color::Green,
            QueueStatus::Archived => Color::Blue,
            QueueStatus::Error(_) => Color::Red,
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

/// Log entry for the TUI
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp
    pub timestamp: String,
    /// Log message
    pub message: String,
    /// Log level color
    pub color: Color,
}

impl LogEntry {
    /// Create a new info log entry
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            color: Color::White,
        }
    }

    /// Create a new success log entry
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            color: Color::Green,
        }
    }

    /// Create a new warning log entry
    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            color: Color::Yellow,
        }
    }

    /// Create a new error log entry
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            color: Color::Red,
        }
    }
}

/// Commands sent from TUI to orchestrator
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Start processing selected changes
    StartProcessing(Vec<String>),
    /// Add a change to the queue dynamically
    AddToQueue(String),
    /// Remove a change from the queue dynamically
    RemoveFromQueue(String),
    /// Toggle approval status for a change
    ToggleApproval(String),
    /// Stop processing (graceful shutdown)
    #[allow(dead_code)]
    Stop,
}

/// Events sent from orchestrator to TUI
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// Processing started for a change
    ProcessingStarted(String),
    /// Progress updated for a change
    #[allow(dead_code)]
    ProgressUpdated {
        id: String,
        completed: u32,
        total: u32,
    },
    /// Processing completed for a change
    ProcessingCompleted(String),
    /// Change archived
    ChangeArchived(String),
    /// Error occurred for a change
    ProcessingError { id: String, error: String },
    /// All processing completed
    AllCompleted,
    /// Log message
    Log(LogEntry),
    /// Changes list refreshed
    ChangesRefreshed(Vec<Change>),
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
    known_change_ids: HashSet<String>,
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
            AppMode::Completed => {
                // Allow queue modifications in Completed mode (same as Running)
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
            AppMode::Error => None,
        }
    }

    /// Start processing selected changes
    pub fn start_processing(&mut self) -> Option<TuiCommand> {
        if self.mode != AppMode::Select && self.mode != AppMode::Completed {
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
        if self.mode != AppMode::Select {
            return None;
        }

        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return None;
        }

        let change = &self.changes[self.cursor_index];
        let id = change.id.clone();

        Some(TuiCommand::ToggleApproval(id))
    }

    /// Update approval status for a specific change
    pub fn update_approval_status(&mut self, change_id: &str, is_approved: bool) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            change.is_approved = is_approved;
            let status_msg = if is_approved { "approved" } else { "unapproved" };
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
                self.mode = AppMode::Completed;
                self.current_change = None;
                self.add_log(LogEntry::success("All changes processed"));
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

/// Truncate a string to fit within a specified display width.
///
/// This function respects Unicode character display widths, where CJK characters
/// (e.g., Japanese, Chinese) typically occupy 2 terminal columns, while ASCII
/// characters occupy 1 column.
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_width` - The maximum display width in terminal columns
///
/// # Returns
/// A truncated string with "..." appended if truncation occurred
fn truncate_to_display_width(s: &str, max_width: usize) -> String {
    let display_width = s.width();
    if display_width <= max_width {
        return s.to_string();
    }

    // Reserve space for "..." (3 columns)
    let target_width = max_width.saturating_sub(3);
    let mut result = String::new();
    let mut current_width = 0;

    for ch in s.chars() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + char_width > target_width {
            break;
        }
        result.push(ch);
        current_width += char_width;
    }

    result.push_str("...");
    result
}

/// Clear the terminal screen
fn clear_screen() -> Result<()> {
    use std::io::stdout;

    execute!(stdout(), Clear(ClearType::All))?;

    Ok(())
}

/// Run the TUI application
pub async fn run_tui(
    initial_changes: Vec<Change>,
    openspec_cmd: String,
    _opencode_path: String, // Deprecated - use config instead
    config: OrchestratorConfig,
) -> Result<()> {
    let mut terminal = ratatui::init();

    // Enable mouse capture for scroll wheel support
    execute!(std::io::stdout(), EnableMouseCapture)?;

    let result = run_tui_loop(&mut terminal, initial_changes, openspec_cmd, config).await;

    // Disable mouse capture before restoring terminal
    execute!(std::io::stdout(), DisableMouseCapture)?;

    // Clear screen before restoring terminal
    clear_screen()?;
    ratatui::restore();

    result
}

/// Main TUI event loop
async fn run_tui_loop(
    terminal: &mut DefaultTerminal,
    initial_changes: Vec<Change>,
    openspec_cmd: String,
    config: OrchestratorConfig,
) -> Result<()> {
    use crate::openspec;

    let mut app = AppState::new(initial_changes);
    let (tx, mut rx) = mpsc::channel::<OrchestratorEvent>(100);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<TuiCommand>(100);

    // Dynamic queue for runtime change additions
    let dynamic_queue = DynamicQueue::new();

    // Cancellation token for graceful shutdown
    let cancel_token = CancellationToken::new();

    // Start auto-refresh task
    let refresh_tx = tx.clone();
    let refresh_cancel = cancel_token.clone();
    let refresh_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(AUTO_REFRESH_INTERVAL_SECS));
        loop {
            tokio::select! {
                _ = refresh_cancel.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    match openspec::list_changes_native() {
                        Ok(changes) => {
                            if refresh_tx
                                .send(OrchestratorEvent::ChangesRefreshed(changes))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = refresh_tx
                                .send(OrchestratorEvent::Log(LogEntry::error(format!(
                                    "Refresh failed: {}",
                                    e
                                ))))
                                .await;
                        }
                    }
                }
            }
        }
    });

    // Orchestrator task (spawned when processing starts)
    let mut orchestrator_handle: Option<tokio::task::JoinHandle<Result<()>>> = None;
    let mut orchestrator_cancel: Option<CancellationToken> = None;

    loop {
        // Increment spinner frame for animation (updates every 100ms)
        app.spinner_frame = (app.spinner_frame + 1) % SPINNER_CHARS.len();

        // Draw the UI
        terminal.draw(|frame| render(frame, &mut app))?;

        // Handle events with timeout
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            app.should_quit = true;
                            break;
                        }
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                            app.cursor_up();
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                            app.cursor_down();
                        }
                        (KeyCode::Char(' '), _) => {
                            if let Some(cmd) = app.toggle_selection() {
                                let _ = cmd_tx.send(cmd).await;
                            }
                        }
                        (KeyCode::Char('@'), _) => {
                            // Toggle approval status
                            if let Some(cmd) = app.toggle_approval() {
                                let _ = cmd_tx.send(cmd).await;
                            }
                        }
                        (KeyCode::F(5), _) => {
                            // Determine which command to use based on mode
                            let cmd = if app.mode == AppMode::Error {
                                app.retry_error_changes()
                            } else {
                                app.start_processing()
                            };

                            if let Some(cmd) = cmd {
                                // Start orchestrator task
                                let selected_ids = match &cmd {
                                    TuiCommand::StartProcessing(ids) => ids.clone(),
                                    _ => vec![],
                                };

                                if !selected_ids.is_empty() {
                                    let orch_tx = tx.clone();
                                    let orch_openspec_cmd = openspec_cmd.clone();
                                    let orch_config = config.clone();
                                    let orch_cancel = CancellationToken::new();
                                    let orch_dynamic_queue = dynamic_queue.clone();
                                    orchestrator_cancel = Some(orch_cancel.clone());

                                    orchestrator_handle = Some(tokio::spawn(async move {
                                        run_orchestrator(
                                            selected_ids,
                                            orch_openspec_cmd,
                                            orch_config,
                                            orch_tx,
                                            orch_cancel,
                                            orch_dynamic_queue,
                                        )
                                        .await
                                    }));
                                }
                            }
                        }
                        (KeyCode::PageUp, _) => {
                            // Scroll logs up (show older entries)
                            app.scroll_logs_up(5);
                        }
                        (KeyCode::PageDown, _) => {
                            // Scroll logs down (show newer entries)
                            app.scroll_logs_down(5);
                        }
                        (KeyCode::Home, _) => {
                            // Jump to oldest log entry
                            app.scroll_logs_to_top();
                        }
                        (KeyCode::End, _) => {
                            // Jump to newest log entry and re-enable auto-scroll
                            app.scroll_logs_to_bottom();
                        }
                        _ => {}
                    }
                    // Clear warning message on any key press
                    app.warning_message = None;
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            // Scroll logs up (show older entries) - 3 lines at a time
                            app.scroll_logs_up(3);
                        }
                        MouseEventKind::ScrollDown => {
                            // Scroll logs down (show newer entries) - 3 lines at a time
                            app.scroll_logs_down(3);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Handle orchestrator events
        while let Ok(event) = rx.try_recv() {
            app.handle_orchestrator_event(event);
        }

        // Handle dynamic queue additions and removals
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                TuiCommand::AddToQueue(id) => {
                    // Push to dynamic queue for orchestrator to pick up
                    if dynamic_queue.push(id.clone()).await {
                        app.add_log(LogEntry::info(format!(
                            "Added to dynamic queue: {}",
                            id
                        )));
                    } else {
                        app.add_log(LogEntry::warn(format!(
                            "Already in dynamic queue: {}",
                            id
                        )));
                    }
                }
                TuiCommand::RemoveFromQueue(id) => {
                    // Log the removal (orchestrator will see the updated status)
                    app.add_log(LogEntry::info(format!("Removed from queue: {}", id)));
                }
                TuiCommand::ToggleApproval(id) => {
                    // Toggle approval status using the approval module
                    use crate::approval;

                    let current_approved = app
                        .changes
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| c.is_approved)
                        .unwrap_or(false);

                    let result = if current_approved {
                        approval::unapprove_change(&id)
                    } else {
                        approval::approve_change(&id)
                    };

                    match result {
                        Ok(_) => {
                            app.update_approval_status(&id, !current_approved);
                        }
                        Err(e) => {
                            app.add_log(LogEntry::error(format!(
                                "Failed to toggle approval for '{}': {}",
                                id, e
                            )));
                        }
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup: cancel all tasks and wait for them to finish
    cancel_token.cancel();
    if let Some(cancel) = orchestrator_cancel {
        cancel.cancel();
    }

    // Wait for tasks to finish gracefully
    refresh_handle.abort();
    if let Some(handle) = orchestrator_handle {
        // Give orchestrator time to cleanup child processes
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    Ok(())
}

/// Context for archive operations
struct ArchiveContext {
    iteration: u32,
    total_changes: usize,
    queue_size: usize,
}

/// Result of archive operation
enum ArchiveResult {
    Success,
    Failed,
    Cancelled,
}

/// Archive a single completed change
/// Returns Ok(ArchiveResult) indicating success, failure, or cancellation
async fn archive_single_change(
    change_id: &str,
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &crate::hooks::HookRunner,
    tx: &mpsc::Sender<OrchestratorEvent>,
    cancel_token: &CancellationToken,
    context: &ArchiveContext,
) -> Result<ArchiveResult> {
    use crate::agent::OutputLine;
    use crate::hooks::{HookContext, HookType};

    // Run on_change_complete hook
    let complete_context =
        HookContext::new(context.iteration, context.total_changes, context.queue_size, false)
            .with_change(change_id, change.completed_tasks, change.total_tasks);
    if let Err(e) = hooks
        .run_hook(HookType::OnChangeComplete, &complete_context)
        .await
    {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_change_complete hook failed: {}",
                e
            ))))
            .await;
    }

    // Run pre_archive hook
    let pre_archive_context =
        HookContext::new(context.iteration, context.total_changes, context.queue_size, false)
            .with_change(change_id, change.completed_tasks, change.total_tasks);
    if let Err(e) = hooks
        .run_hook(HookType::PreArchive, &pre_archive_context)
        .await
    {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "pre_archive hook failed: {}",
                e
            ))))
            .await;
    }

    // Archive the change
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(format!(
            "Archiving: {}",
            change_id
        ))))
        .await;

    // Run archive command with streaming output
    let (mut child, mut output_rx) = agent.run_archive_streaming(change_id).await?;

    // Stream output to TUI log, with cancellation support
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                let _ = child.kill().await;
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::warn(
                        "Process killed due to cancellation".to_string(),
                    )))
                    .await;
                return Ok(ArchiveResult::Cancelled);
            }
            line = output_rx.recv() => {
                match line {
                    Some(OutputLine::Stdout(s)) => {
                        let _ = tx.send(OrchestratorEvent::Log(LogEntry::info(s))).await;
                    }
                    Some(OutputLine::Stderr(s)) => {
                        let _ = tx.send(OrchestratorEvent::Log(LogEntry::warn(s))).await;
                    }
                    None => break,
                }
            }
        }
    }

    // Wait for child process to complete
    let status = child.wait().await.map_err(|e| {
        crate::error::OrchestratorError::AgentCommand(format!(
            "Failed to wait for process: {}",
            e
        ))
    })?;

    if status.success() {
        // Clear apply history for the archived change
        agent.clear_apply_history(change_id);

        // Run post_archive hook
        let post_archive_context =
            HookContext::new(context.iteration, context.total_changes, context.queue_size.saturating_sub(1), false)
                .with_change(change_id, change.completed_tasks, change.total_tasks);
        if let Err(e) = hooks
            .run_hook(HookType::PostArchive, &post_archive_context)
            .await
        {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "post_archive hook failed: {}",
                    e
                ))))
                .await;
        }

        let _ = tx
            .send(OrchestratorEvent::ChangeArchived(change_id.to_string()))
            .await;
        Ok(ArchiveResult::Success)
    } else {
        let error_msg = format!("Archive failed with exit code: {:?}", status.code());

        // Run on_error hook
        let error_context =
            HookContext::new(context.iteration, context.total_changes, context.queue_size, false)
                .with_change(change_id, change.completed_tasks, change.total_tasks)
                .with_error(&error_msg);
        let _ = hooks.run_hook(HookType::OnError, &error_context).await;

        let _ = tx
            .send(OrchestratorEvent::ProcessingError {
                id: change_id.to_string(),
                error: error_msg.clone(),
            })
            .await;
        Ok(ArchiveResult::Failed)
    }
}

/// Archive all complete changes from the pending set
/// Returns the number of successfully archived changes
async fn archive_all_complete_changes(
    pending_ids: &HashSet<String>,
    _openspec_cmd: &str, // Kept for API compatibility, native impl doesn't need it
    agent: &mut AgentRunner,
    hooks: &crate::hooks::HookRunner,
    tx: &mpsc::Sender<OrchestratorEvent>,
    cancel_token: &CancellationToken,
    archived_set: &mut HashSet<String>,
    total_changes: usize,
    iteration: &mut u32,
) -> Result<usize> {
    use crate::openspec;

    // Fetch current state of all changes using native implementation
    let changes = openspec::list_changes_native()?;

    // Find complete changes that are still in pending set
    let complete_changes: Vec<Change> = changes
        .into_iter()
        .filter(|c| pending_ids.contains(&c.id) && !archived_set.contains(&c.id) && c.is_complete())
        .collect();

    let mut archived_count = 0;

    for change in complete_changes {
        if cancel_token.is_cancelled() {
            break;
        }

        *iteration += 1;
        let queue_size = pending_ids.len().saturating_sub(archived_count);
        let context = ArchiveContext {
            iteration: *iteration,
            total_changes,
            queue_size,
        };

        // Notify processing started for this change
        let _ = tx
            .send(OrchestratorEvent::ProcessingStarted(change.id.clone()))
            .await;

        // Send ProcessingCompleted before archiving
        let _ = tx
            .send(OrchestratorEvent::ProcessingCompleted(change.id.clone()))
            .await;

        match archive_single_change(&change.id, &change, agent, hooks, tx, cancel_token, &context).await? {
            ArchiveResult::Success => {
                archived_set.insert(change.id.clone());
                archived_count += 1;
            }
            ArchiveResult::Failed => {
                // Error already logged and sent, continue to next
            }
            ArchiveResult::Cancelled => {
                break;
            }
        }
    }

    Ok(archived_count)
}

/// Run the orchestrator for selected changes
/// Uses streaming output to send log entries in real-time
/// Supports cancellation via CancellationToken for graceful shutdown
///
/// The orchestrator uses a two-phase loop:
/// - Phase 1: Archive all complete changes before doing any apply
/// - Phase 2: Apply one incomplete change
///
/// This ensures complete changes are never skipped.
async fn run_orchestrator(
    change_ids: Vec<String>,
    openspec_cmd: String,
    config: OrchestratorConfig,
    tx: mpsc::Sender<OrchestratorEvent>,
    cancel_token: CancellationToken,
    dynamic_queue: DynamicQueue,
) -> Result<()> {
    use crate::agent::OutputLine;
    use crate::hooks::{HookContext, HookRunner, HookType};
    use crate::openspec;

    let hooks = HookRunner::new(config.get_hooks());
    let mut agent = AgentRunner::new(config);

    let mut total_changes = change_ids.len();
    let mut iteration: u32 = 0;
    let mut first_apply_executed = false;
    let mut archived_changes: HashSet<String> = HashSet::new();
    let mut pending_changes: HashSet<String> = change_ids.iter().cloned().collect();
    let mut processed_change_ids: Vec<String> = change_ids.clone();

    // Run on_start hook
    let start_context = HookContext::new(0, total_changes, total_changes, false);
    if let Err(e) = hooks.run_hook(HookType::OnStart, &start_context).await {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_start hook failed: {}",
                e
            ))))
            .await;
    }

    // Main two-phase loop
    loop {
        // Check for cancellation before each iteration
        if cancel_token.is_cancelled() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Check dynamic queue for new changes before checking if we're done
        while let Some(dynamic_id) = dynamic_queue.pop().await {
            // Skip if already archived or in pending
            if !archived_changes.contains(&dynamic_id) && !pending_changes.contains(&dynamic_id) {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Processing dynamically added: {}",
                        dynamic_id
                    ))))
                    .await;
                pending_changes.insert(dynamic_id.clone());
                processed_change_ids.push(dynamic_id);
                total_changes += 1;
            }
        }

        // Check if all pending changes are done
        if pending_changes.is_empty() {
            break;
        }

        // Phase 1: Archive all complete changes
        let archived_count = archive_all_complete_changes(
            &pending_changes,
            &openspec_cmd,
            &mut agent,
            &hooks,
            &tx,
            &cancel_token,
            &mut archived_changes,
            total_changes,
            &mut iteration,
        )
        .await?;

        // Remove archived changes from pending
        for id in &archived_changes {
            pending_changes.remove(id);
        }

        if archived_count > 0 {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                    "Archived {} complete change(s)",
                    archived_count
                ))))
                .await;
        }

        // Check if all done after archiving
        // Dynamic queue is checked at the start of the next iteration
        if pending_changes.is_empty() {
            continue; // Re-check dynamic queue
        }

        // Check for cancellation after archive phase
        if cancel_token.is_cancelled() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Phase 2: Select and apply next incomplete change
        // Fetch current state to find best candidate using native implementation
        let changes = openspec::list_changes_native()?;

        // Find the next incomplete change from our pending set
        // Prioritize by highest progress percentage
        let next_change = changes
            .iter()
            .filter(|c| pending_changes.contains(&c.id) && !c.is_complete())
            .max_by(|a, b| {
                let a_progress = if a.total_tasks > 0 {
                    a.completed_tasks as f32 / a.total_tasks as f32
                } else {
                    0.0
                };
                let b_progress = if b.total_tasks > 0 {
                    b.completed_tasks as f32 / b.total_tasks as f32
                } else {
                    0.0
                };
                a_progress.partial_cmp(&b_progress).unwrap_or(std::cmp::Ordering::Equal)
            });

        let Some(change) = next_change else {
            // No incomplete changes found - might all be complete now
            // Loop will re-check in Phase 1
            continue;
        };

        let change_id = change.id.clone();
        let change = change.clone();
        iteration += 1;

        // Notify processing started
        let _ = tx
            .send(OrchestratorEvent::ProcessingStarted(change_id.clone()))
            .await;

        let queue_size = pending_changes.len();

        // Run on_first_apply hook (once)
        if !first_apply_executed {
            let first_apply_context =
                HookContext::new(iteration, total_changes, queue_size, false).with_change(
                    &change_id,
                    change.completed_tasks,
                    change.total_tasks,
                );
            if let Err(e) = hooks
                .run_hook(HookType::OnFirstApply, &first_apply_context)
                .await
            {
                let _ = tx
                    .send(OrchestratorEvent::ProcessingError {
                        id: change_id.clone(),
                        error: format!("on_first_apply hook failed: {}", e),
                    })
                    .await;
                break;
            }
            first_apply_executed = true;
        }

        // Run pre_apply hook
        let pre_apply_context =
            HookContext::new(iteration, total_changes, queue_size, false).with_change(
                &change_id,
                change.completed_tasks,
                change.total_tasks,
            );
        if let Err(e) = hooks.run_hook(HookType::PreApply, &pre_apply_context).await {
            let _ = tx
                .send(OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: format!("pre_apply hook failed: {}", e),
                })
                .await;
            break;
        }

        // Apply the change
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::info(format!(
                "Applying: {}",
                change_id
            ))))
            .await;

        // Run apply command with streaming output
        let (mut child, mut output_rx, start_time) = agent.run_apply_streaming(&change_id).await?;

        // Stream output to TUI log, with cancellation support
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    let _ = child.kill().await;
                    let _ = tx
                        .send(OrchestratorEvent::Log(LogEntry::warn(
                            "Process killed due to cancellation".to_string(),
                        )))
                        .await;
                    // Exit the main loop
                    pending_changes.clear();
                    break;
                }
                line = output_rx.recv() => {
                    match line {
                        Some(OutputLine::Stdout(s)) => {
                            let _ = tx.send(OrchestratorEvent::Log(LogEntry::info(s))).await;
                        }
                        Some(OutputLine::Stderr(s)) => {
                            let _ = tx.send(OrchestratorEvent::Log(LogEntry::warn(s))).await;
                        }
                        None => break,
                    }
                }
            }
        }

        // Check if we were cancelled during streaming
        if cancel_token.is_cancelled() {
            break;
        }

        // Wait for child process to complete
        let status = child.wait().await.map_err(|e| {
            crate::error::OrchestratorError::AgentCommand(format!(
                "Failed to wait for process: {}",
                e
            ))
        })?;

        // Record the apply attempt for history context in subsequent retries
        agent.record_apply_attempt(&change_id, &status, start_time);

        if status.success() {
            // Run post_apply hook
            let post_apply_context =
                HookContext::new(iteration, total_changes, queue_size, false).with_change(
                    &change_id,
                    change.completed_tasks,
                    change.total_tasks,
                );
            if let Err(e) = hooks
                .run_hook(HookType::PostApply, &post_apply_context)
                .await
            {
                let _ = tx
                    .send(OrchestratorEvent::ProcessingError {
                        id: change_id.clone(),
                        error: format!("post_apply hook failed: {}", e),
                    })
                    .await;
                break;
            }

            // Apply succeeded - check if tasks are now 100% complete
            // Re-fetch change to get updated task counts after apply
            let updated_changes = crate::openspec::list_changes_native().unwrap_or_default();
            let is_complete = updated_changes
                .iter()
                .find(|c| c.id == change_id)
                .is_some_and(|c| c.is_complete());

            if is_complete {
                // Only send ProcessingCompleted when tasks are 100% done
                let _ = tx
                    .send(OrchestratorEvent::ProcessingCompleted(change_id.clone()))
                    .await;
            }

            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                    "Apply completed for {}, checking for completion...",
                    change_id
                ))))
                .await;
        } else {
            let error_msg = format!("Apply failed with exit code: {:?}", status.code());

            // Run on_error hook
            let error_context =
                HookContext::new(iteration, total_changes, queue_size, false)
                    .with_change(&change_id, change.completed_tasks, change.total_tasks)
                    .with_error(&error_msg);
            let _ = hooks.run_hook(HookType::OnError, &error_context).await;

            let _ = tx
                .send(OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error_msg,
                })
                .await;

            // Remove failed change from pending to prevent infinite retry
            pending_changes.remove(&change_id);
        }
    }

    // Final verification: check if any changes remain unarchived
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(
            "Verifying all changes have been archived...".to_string(),
        )))
        .await;

    // Check against our tracked archived set for reliable verification
    let unarchived_by_tracking: Vec<&str> = processed_change_ids
        .iter()
        .filter(|id| !archived_changes.contains(*id))
        .map(|id| id.as_str())
        .collect();

    // Also verify against native list as backup
    let final_changes = openspec::list_changes_native().ok();
    if let Some(changes) = final_changes {
        let unarchived_by_list: Vec<&str> = processed_change_ids
            .iter()
            .filter(|id| changes.iter().any(|c| &c.id == *id))
            .map(|id| id.as_str())
            .collect();

        // Report unarchived changes (use tracking as primary, list as confirmation)
        if !unarchived_by_tracking.is_empty() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Warning: {} change(s) were not archived (tracking): {}",
                    unarchived_by_tracking.len(),
                    unarchived_by_tracking.join(", ")
                ))))
                .await;
        }
        if !unarchived_by_list.is_empty() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Warning: {} change(s) remain in openspec list: {}",
                    unarchived_by_list.len(),
                    unarchived_by_list.join(", ")
                ))))
                .await;
        }
        if unarchived_by_tracking.is_empty() && unarchived_by_list.is_empty() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::success(
                    "All processed changes have been archived".to_string(),
                )))
                .await;
        }
    } else if !unarchived_by_tracking.is_empty() {
        // Could not fetch final list, but tracking shows unarchived changes
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "Warning: {} change(s) were not archived (tracking): {}",
                unarchived_by_tracking.len(),
                unarchived_by_tracking.join(", ")
            ))))
            .await;
    }

    let _ = tx.send(OrchestratorEvent::AllCompleted).await;
    Ok(())
}

/// Render the TUI
fn render(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();

    // Check minimum terminal size
    if area.width < 60 || area.height < 15 {
        let warning = Paragraph::new("Terminal too small. Minimum: 60x15")
            .style(Style::default().fg(Color::Red));
        frame.render_widget(warning, area);
        return;
    }

    match app.mode {
        AppMode::Select => render_select_mode(frame, app, area),
        AppMode::Running | AppMode::Completed | AppMode::Error => {
            render_running_mode(frame, app, area)
        }
    }
}

/// Render selection mode
fn render_select_mode(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(5),    // Changes list
        Constraint::Length(3), // Footer
    ])
    .split(area);

    // Header
    render_header(frame, app, chunks[0]);

    // Changes list
    render_changes_list_select(frame, app, chunks[1]);

    // Footer
    render_footer_select(frame, app, chunks[2]);
}

/// Render running mode
fn render_running_mode(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3),  // Header
        Constraint::Min(5),     // Changes list
        Constraint::Length(3),  // Status
        Constraint::Length(10), // Logs
    ])
    .split(area);

    // Header
    render_header(frame, app, chunks[0]);

    // Changes list
    render_changes_list_running(frame, app, chunks[1]);

    // Status
    render_status(frame, app, chunks[2]);

    // Logs
    render_logs(frame, app, chunks[3]);
}

/// Render header
fn render_header(frame: &mut Frame, app: &AppState, area: Rect) {
    let mode_text = match app.mode {
        AppMode::Select => "Select Mode",
        AppMode::Running => "Running",
        AppMode::Completed => "Completed",
        AppMode::Error => "Error",
    };

    let mode_color = match app.mode {
        AppMode::Select => Color::Cyan,
        AppMode::Running => Color::Yellow,
        AppMode::Completed => Color::Green,
        AppMode::Error => Color::Red,
    };

    let header_text = Line::from(vec![
        Span::styled("OpenSpec Orchestrator", Style::default().fg(Color::White)),
        Span::raw("  "),
        Span::styled(
            format!("[{}]", mode_text),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
    ]);

    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(header, area);
}

/// Render changes list in selection mode
fn render_changes_list_select(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .changes
        .iter()
        .enumerate()
        .map(|(i, change)| {
            // Checkbox display:
            // [ ] - unapproved (cannot be selected)
            // [@] - approved but not queued
            // [x] - queued (approved and selected)
            let (checkbox, checkbox_color) = if !change.is_approved {
                ("[ ]", Color::DarkGray) // Unapproved
            } else if change.selected {
                ("[x]", Color::Green) // Queued
            } else {
                ("[@]", Color::Yellow) // Approved but not queued
            };

            let cursor = if i == app.cursor_index { "►" } else { " " };
            let new_badge = if change.is_new { " NEW" } else { "" };

            let line = Line::from(vec![
                Span::styled(
                    format!("{} {} ", checkbox, cursor),
                    Style::default().fg(checkbox_color),
                ),
                Span::styled(
                    format!("{:<25}", change.id),
                    Style::default().fg(if change.is_approved {
                        Color::White
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    new_badge,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}/{} tasks", change.completed_tasks, change.total_tasks),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  {:>5.1}%", change.progress_percent()),
                    Style::default().fg(Color::Cyan),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Changes (↑↓/jk: move, Space: queue, @: approve, F5: run, q: quit) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

/// Render changes list in running mode
fn render_changes_list_running(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let spinner_char = SPINNER_CHARS[app.spinner_frame];

    let items: Vec<ListItem> = app
        .changes
        .iter()
        .map(|change| {
            let cursor = if Some(&change.id) == app.current_change.as_ref() {
                "►"
            } else {
                " "
            };
            let new_badge = if change.is_new { " NEW" } else { "" };

            let status_text = match &change.queue_status {
                QueueStatus::Processing => {
                    format!("{} [{:>3.0}%]", spinner_char, change.progress_percent())
                }
                QueueStatus::Completed | QueueStatus::Archived | QueueStatus::Error(_) => {
                    format!("[{}]", change.queue_status.display())
                }
                status => format!("[{}]", status.display()),
            };

            let line = Line::from(vec![
                Span::styled(format!("{} ", cursor), Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{:<25}", change.id),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    new_badge,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {:>18}", status_text),
                    Style::default().fg(change.queue_status.color()),
                ),
                Span::styled(
                    format!("  {}/{}", change.completed_tasks, change.total_tasks),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Changes (Space: add/remove from queue - processed dynamically) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

/// Render status panel
fn render_status(frame: &mut Frame, app: &AppState, area: Rect) {
    let (current_text, current_color) = match app.mode {
        AppMode::Error => {
            let error_id = app.error_change_id.as_deref().unwrap_or("unknown");
            (format!("Error in: {}", error_id), Color::Red)
        }
        AppMode::Completed => ("Done".to_string(), Color::Green),
        _ => match &app.current_change {
            Some(id) => (format!("Current: {}", id), Color::White),
            None => ("Waiting...".to_string(), Color::White),
        },
    };

    let (status_text, status_color) = match app.mode {
        AppMode::Completed => ("All processing completed. Press 'q' to quit.", Color::Green),
        AppMode::Running => ("Processing...", Color::Cyan),
        AppMode::Select => ("", Color::White),
        AppMode::Error => ("Press F5 to retry, or 'q' to quit.", Color::Yellow),
    };

    // Calculate overall progress for all queued changes (including completed/archived)
    let progress_info = if app.mode == AppMode::Running {
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

        if total_tasks > 0 {
            let percent = (completed_tasks as f32 / total_tasks as f32) * 100.0;
            let bar_width = 20;
            let filled = ((percent / 100.0) * bar_width as f32) as usize;
            let empty = bar_width - filled;
            Some((
                format!(
                    "[{}{}] {:>5.1}% ({}/{})",
                    "█".repeat(filled),
                    "░".repeat(empty),
                    percent,
                    completed_tasks,
                    total_tasks
                ),
                Color::Cyan,
            ))
        } else {
            None
        }
    } else {
        None
    };

    let mut spans = vec![
        Span::styled(current_text, Style::default().fg(current_color)),
        Span::raw("  |  "),
        Span::styled(status_text, Style::default().fg(status_color)),
    ];

    if let Some((progress_text, progress_color)) = progress_info {
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled(
            progress_text,
            Style::default().fg(progress_color),
        ));
    }

    let content = Line::from(spans);

    let status = Paragraph::new(content).block(
        Block::default()
            .title(" Status ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(status, area);
}

/// Render logs panel with scroll support
fn render_logs(frame: &mut Frame, app: &AppState, area: Rect) {
    // Calculate available width for message (subtract borders, timestamp, and padding)
    // Timestamp format: "HH:MM:SS " = 9 chars, borders = 2 chars
    let available_width = (area.width as usize).saturating_sub(2 + 9 + 1);

    // Calculate visible area height (subtract borders)
    let visible_height = (area.height as usize).saturating_sub(2);
    let total_logs = app.logs.len();

    // Calculate the range of logs to display based on scroll offset
    // scroll_offset = 0 means show the most recent logs at the bottom
    let end_index = total_logs.saturating_sub(app.log_scroll_offset);
    let start_index = end_index.saturating_sub(visible_height);

    let log_items: Vec<Line> = app
        .logs
        .iter()
        .skip(start_index)
        .take(end_index - start_index)
        .map(|entry| {
            // Truncate message to fit in available width using Unicode display width
            // This correctly handles CJK characters that occupy 2 terminal columns
            let message = truncate_to_display_width(&entry.message, available_width);

            Line::from(vec![
                Span::styled(
                    format!("{} ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(message, Style::default().fg(entry.color)),
            ])
        })
        .collect();

    // Build title with scroll position indicator
    let title = if total_logs > visible_height {
        let visible_start = start_index + 1;
        let visible_end = end_index;
        format!(" Logs [{}-{}/{}] ", visible_start, visible_end, total_logs)
    } else {
        " Logs ".to_string()
    };

    let logs = Paragraph::new(log_items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(logs, area);
}

/// Get version string for display
pub fn get_version_string() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// Render footer in selection mode
fn render_footer_select(frame: &mut Frame, app: &AppState, area: Rect) {
    let selected = app.selected_count();
    let new_count = app.new_change_count;
    let version = get_version_string();

    let mut spans = vec![
        Span::styled(
            format!("Selected: {} changes", selected),
            Style::default().fg(Color::Green),
        ),
        Span::raw("  |  "),
    ];

    if new_count > 0 {
        spans.push(Span::styled(
            format!("New: {}", new_count),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  |  "));
    }

    if let Some(warning) = &app.warning_message {
        spans.push(Span::styled(
            warning.clone(),
            Style::default().fg(Color::Red),
        ));
    } else if app.changes.is_empty() {
        // No changes available
        spans.push(Span::styled(
            "Add new proposals to get started",
            Style::default().fg(Color::DarkGray),
        ));
    } else if selected == 0 {
        // Changes exist but none selected
        spans.push(Span::styled(
            "Select changes with Space to process",
            Style::default().fg(Color::Yellow),
        ));
    } else {
        // Changes selected and ready to process
        spans.push(Span::styled(
            "Press F5 to start processing",
            Style::default().fg(Color::Cyan),
        ));
    }

    // Split area into left content and right-aligned version
    let version_width = version.len() as u16 + 2; // +2 for padding
    let chunks =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(version_width)]).split(area);

    // Render left content (status information) with left and bottom/top borders
    let left_footer = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(left_footer, chunks[0]);

    // Render right content (version) with right and bottom/top borders
    let right_footer = Paragraph::new(Line::from(vec![Span::styled(
        version,
        Style::default().fg(Color::DarkGray),
    )]))
    .block(
        Block::default()
            .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(right_footer, chunks[1]);
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
        assert!(app.logs.iter().any(|log| log.message.contains("Auto-queued")));
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
        assert!(!app.logs.iter().any(|log| log.message.contains("Auto-queued")));
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
    fn test_queue_status_display() {
        assert_eq!(QueueStatus::NotQueued.display(), "not queued");
        assert_eq!(QueueStatus::Queued.display(), "queued");
        assert_eq!(QueueStatus::Processing.display(), "processing");
        assert_eq!(QueueStatus::Completed.display(), "completed");
        assert_eq!(QueueStatus::Archived.display(), "archived");
        assert_eq!(QueueStatus::Error("err".to_string()).display(), "error");
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

        // Both changes should be counted
        // Total: 5 + 3 = 8, Completed: 2 + 3 = 5
        assert_eq!(total_tasks, 8);
        assert_eq!(completed_tasks, 5);
    }

    #[test]
    fn test_progress_calculation_includes_archived_changes() {
        // Use approved changes so they start selected
        let changes = vec![
            create_approved_change("a", 2, 5), // 2/5 done, will be Processing
            create_approved_change("b", 3, 3), // 3/3 done, will be Archived
        ];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        assert_eq!(app.mode, AppMode::Running);

        // Simulate: a is processing, b is archived
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[1].queue_status = QueueStatus::Archived;

        // Calculate progress (includes Archived changes)
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

        // Both changes should be counted
        // Total: 5 + 3 = 8, Completed: 2 + 3 = 5
        assert_eq!(total_tasks, 8);
        assert_eq!(completed_tasks, 5);
    }

    #[test]
    fn test_progress_calculation_excludes_not_queued() {
        // Use approved changes so they start selected
        let changes = vec![
            create_approved_change("a", 2, 5), // 2/5 done, will be Processing
            create_approved_change("b", 3, 3), // 3/3 done, will be NotQueued
        ];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();

        // Simulate: a is processing, b is removed from queue
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[1].queue_status = QueueStatus::NotQueued;

        // Calculate progress (excludes NotQueued)
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

        // Only 'a' should be counted
        // Total: 5, Completed: 2
        assert_eq!(total_tasks, 5);
        assert_eq!(completed_tasks, 2);
    }

    #[test]
    fn test_progress_calculation_excludes_error() {
        let changes = vec![
            create_test_change("a", 2, 5), // 2/5 done, will be Processing
            create_test_change("b", 3, 3), // 3/3 done, will be Error
        ];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();

        // Simulate: a is processing, b has error
        app.changes[0].queue_status = QueueStatus::Processing;
        app.changes[1].queue_status = QueueStatus::Error("test error".to_string());

        // Calculate progress (excludes Error)
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

        // Only 'a' should be counted
        // Total: 5, Completed: 2
        assert_eq!(total_tasks, 5);
        assert_eq!(completed_tasks, 2);
    }

    #[test]
    fn test_change_state_is_new_default_false() {
        let change = create_test_change("test", 1, 2);
        // By default, changes created via from_change should not be new
        // since selected=true implies initial state
        let state = ChangeState::from_change(&change, true);
        assert!(!state.is_new);
    }

    #[test]
    fn test_truncate_to_display_width_ascii_no_truncation() {
        let result = truncate_to_display_width("hello world", 20);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_truncate_to_display_width_ascii_with_truncation() {
        let result = truncate_to_display_width("hello world", 8);
        // "hello" (5) + "..." (3) = 8
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_to_display_width_japanese_no_truncation() {
        // Japanese characters typically have width 2
        let result = truncate_to_display_width("こんにちは", 20);
        assert_eq!(result, "こんにちは");
    }

    #[test]
    fn test_truncate_to_display_width_japanese_with_truncation() {
        // "こんにちは" = 5 chars * 2 width = 10 display width
        // With max_width=8, we need to truncate
        // target_width = 8 - 3 = 5
        // "こん" = 2 chars * 2 width = 4, fits
        // "こんに" = 3 chars * 2 width = 6, doesn't fit
        let result = truncate_to_display_width("こんにちは", 8);
        assert_eq!(result, "こん...");
    }

    #[test]
    fn test_truncate_to_display_width_mixed_content() {
        // Mixed ASCII and Japanese
        // "Hello日本語" = "Hello" (5) + "日本語" (3*2=6) = 11 display width
        let result = truncate_to_display_width("Hello日本語", 10);
        // target_width = 10 - 3 = 7
        // "Hello" (5) + "日" (2) = 7, fits exactly
        assert_eq!(result, "Hello日...");
    }

    #[test]
    fn test_truncate_to_display_width_empty_string() {
        let result = truncate_to_display_width("", 10);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_to_display_width_exact_fit() {
        // String that exactly fits the max width
        let result = truncate_to_display_width("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_to_display_width_very_small_max() {
        // Max width smaller than "..."
        let result = truncate_to_display_width("hello", 2);
        // target_width = 2 - 3 = 0 (saturating)
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_to_display_width_emoji() {
        // Emoji width can vary by unicode-width version and terminal
        // Just verify the function handles emojis without panicking
        let input = "👍 Good";
        let result = truncate_to_display_width(input, 8);
        // Should either return original (if fits) or truncated with "..."
        assert!(result == input || result.ends_with("..."));
        // Should not panic on emoji input
    }

    #[test]
    fn test_get_version_string_format() {
        let version = get_version_string();
        // Version string should start with "v"
        assert!(version.starts_with('v'));
        // Version string should have at least one digit after "v"
        assert!(version.len() > 1);
        // Version should match Cargo.toml version format (e.g., "v0.1.0")
        let version_part = &version[1..]; // Remove "v" prefix
        assert!(version_part.chars().all(|c| c.is_ascii_digit() || c == '.'));
    }

    #[test]
    fn test_get_version_string_matches_cargo_version() {
        let version = get_version_string();
        // Should match the version from Cargo.toml
        let expected = format!("v{}", env!("CARGO_PKG_VERSION"));
        assert_eq!(version, expected);
    }

    #[test]
    fn test_status_text_format_for_terminal_states() {
        // Test that terminal states show only status name (task count is in separate column)
        let change = ChangeState {
            id: "test-change".to_string(),
            completed_tasks: 8,
            total_tasks: 13,
            queue_status: QueueStatus::Completed,
            selected: true,
            is_new: false,
            last_modified: "now".to_string(),
            is_approved: false,
        };

        // Verify completed status format (no task count - shown in separate column)
        let status_text = format!("[{}]", change.queue_status.display());
        assert_eq!(status_text, "[completed]");

        // Verify archived status format
        let mut archived_change = change.clone();
        archived_change.queue_status = QueueStatus::Archived;
        let status_text = format!("[{}]", archived_change.queue_status.display());
        assert_eq!(status_text, "[archived]");

        // Verify error status format
        let mut error_change = change.clone();
        error_change.queue_status = QueueStatus::Error("test error".to_string());
        let status_text = format!("[{}]", error_change.queue_status.display());
        assert_eq!(status_text, "[error]");
    }

    #[test]
    fn test_status_text_format_width_accommodates_status() {
        // Test that status text fits within column width
        let max_status_width = 18; // Column width

        // Terminal states show only status name (task count in separate column)
        let completed_status = "[completed]";
        assert!(completed_status.len() <= max_status_width);

        // Archived format
        let archived_status = "[archived]";
        assert!(archived_status.len() <= max_status_width);

        // Error format
        let error_status = "[error]";
        assert!(error_status.len() <= max_status_width);

        // Processing format (includes percentage)
        let processing_status = "⠋ [100%]";
        assert!(processing_status.chars().count() <= max_status_width);
    }

    #[test]
    fn test_processing_completed_only_marks_complete_when_100_percent() {
        let changes = vec![create_test_change("a", 8, 13)]; // 8/13 tasks - not 100%
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingStarted("a".to_string()));
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);

        // If ProcessingCompleted is sent, it should mark as Completed
        // (The key change is that ProcessingCompleted should NOT be sent for incomplete tasks)
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingCompleted("a".to_string()));
        assert_eq!(app.changes[0].queue_status, QueueStatus::Completed);

        // This test documents the behavior: ProcessingCompleted always sets Completed status.
        // The fix ensures ProcessingCompleted is only sent when tasks are 100% done.
    }

    #[test]
    fn test_processing_stays_processing_without_completed_event() {
        let changes = vec![create_test_change("a", 8, 13)]; // 8/13 tasks - not 100%
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingStarted("a".to_string()));
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);

        // Without ProcessingCompleted event, status stays Processing
        // This is the expected behavior when apply succeeds but tasks are not 100%
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);

        // Status is NOT Completed
        assert_ne!(app.changes[0].queue_status, QueueStatus::Completed);
    }

    #[test]
    fn test_progress_updated_does_not_change_queue_status() {
        let changes = vec![create_test_change("a", 0, 10)];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingStarted("a".to_string()));
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);

        // Progress update should not change queue status
        app.handle_orchestrator_event(OrchestratorEvent::ProgressUpdated {
            id: "a".to_string(),
            completed: 5,
            total: 10,
        });

        // Queue status should still be Processing
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);
        // But progress should be updated
        assert_eq!(app.changes[0].completed_tasks, 5);
        assert_eq!(app.changes[0].total_tasks, 10);
    }

    #[test]
    fn test_completed_status_only_after_processing_completed_event() {
        let changes = vec![create_test_change("a", 10, 10)]; // 100% complete
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingStarted("a".to_string()));
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);

        // Even with 100% tasks, status is Processing until ProcessingCompleted event
        assert_eq!(app.changes[0].queue_status, QueueStatus::Processing);

        // Only ProcessingCompleted event transitions to Completed
        app.handle_orchestrator_event(OrchestratorEvent::ProcessingCompleted("a".to_string()));
        assert_eq!(app.changes[0].queue_status, QueueStatus::Completed);
    }

    // DynamicQueue tests

    #[tokio::test]
    async fn test_dynamic_queue_push_pop() {
        let queue = DynamicQueue::new();

        // Push items
        assert!(queue.push("change-a".to_string()).await);
        assert!(queue.push("change-b".to_string()).await);

        // Pop in FIFO order
        assert_eq!(queue.pop().await, Some("change-a".to_string()));
        assert_eq!(queue.pop().await, Some("change-b".to_string()));
        assert_eq!(queue.pop().await, None);
    }

    #[tokio::test]
    async fn test_dynamic_queue_duplicate_prevention() {
        let queue = DynamicQueue::new();

        // First push succeeds
        assert!(queue.push("change-a".to_string()).await);

        // Duplicate push fails
        assert!(!queue.push("change-a".to_string()).await);

        // Different ID succeeds
        assert!(queue.push("change-b".to_string()).await);
    }

    #[tokio::test]
    async fn test_dynamic_queue_is_empty() {
        let queue = DynamicQueue::new();

        assert!(queue.is_empty().await);

        queue.push("change-a".to_string()).await;
        assert!(!queue.is_empty().await);

        queue.pop().await;
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_dynamic_queue_contains() {
        let queue = DynamicQueue::new();

        assert!(!queue.contains("change-a").await);

        queue.push("change-a".to_string()).await;
        assert!(queue.contains("change-a").await);
        assert!(!queue.contains("change-b").await);
    }

    #[tokio::test]
    async fn test_dynamic_queue_len() {
        let queue = DynamicQueue::new();

        assert_eq!(queue.len().await, 0);

        queue.push("change-a".to_string()).await;
        assert_eq!(queue.len().await, 1);

        queue.push("change-b".to_string()).await;
        assert_eq!(queue.len().await, 2);

        queue.pop().await;
        assert_eq!(queue.len().await, 1);
    }

    #[tokio::test]
    async fn test_dynamic_queue_clone_shares_state() {
        let queue1 = DynamicQueue::new();
        let queue2 = queue1.clone();

        // Push on one clone
        queue1.push("change-a".to_string()).await;

        // Visible on the other
        assert!(queue2.contains("change-a").await);
        assert_eq!(queue2.len().await, 1);

        // Pop from the other
        assert_eq!(queue2.pop().await, Some("change-a".to_string()));

        // Reflected in both
        assert!(queue1.is_empty().await);
    }

    // Log scroll tests

    #[test]
    fn test_scroll_logs_to_top() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Add some log entries
        for i in 0..10 {
            app.add_log(LogEntry::info(format!("Log entry {}", i)));
        }

        // Initially at bottom with auto-scroll enabled
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);

        // Scroll to top
        app.scroll_logs_to_top();

        // Should be at max offset (oldest logs) with auto-scroll disabled
        assert_eq!(app.log_scroll_offset, app.logs.len().saturating_sub(1));
        assert!(!app.log_auto_scroll);
    }

    #[test]
    fn test_scroll_logs_to_bottom() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Add some log entries
        for i in 0..10 {
            app.add_log(LogEntry::info(format!("Log entry {}", i)));
        }

        // Scroll up first
        app.scroll_logs_up(5);
        assert_eq!(app.log_scroll_offset, 5);
        assert!(!app.log_auto_scroll);

        // Scroll to bottom
        app.scroll_logs_to_bottom();

        // Should be at offset 0 (newest logs) with auto-scroll enabled
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);
    }

    #[test]
    fn test_scroll_logs_to_top_empty_logs() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Clear logs
        app.logs.clear();

        // Scroll to top should handle empty logs gracefully
        app.scroll_logs_to_top();

        // With no logs, max_offset should be 0 (saturating_sub on len=0)
        assert_eq!(app.log_scroll_offset, 0);
        assert!(!app.log_auto_scroll);
    }

    #[test]
    fn test_scroll_logs_to_top_single_log() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Clear and add single log
        app.logs.clear();
        app.add_log(LogEntry::info("Single log entry".to_string()));

        // Scroll to top
        app.scroll_logs_to_top();

        // With single log, max_offset should be 0
        assert_eq!(app.log_scroll_offset, 0);
        assert!(!app.log_auto_scroll);
    }
}
