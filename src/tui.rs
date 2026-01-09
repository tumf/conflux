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
    event::{self, Event, KeyCode, KeyEventKind},
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
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Auto-refresh interval in seconds
const AUTO_REFRESH_INTERVAL_SECS: u64 = 5;

/// Maximum number of log entries to keep
const MAX_LOG_ENTRIES: usize = 100;

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
}

impl AppState {
    /// Create a new AppState with initial changes
    pub fn new(changes: Vec<Change>) -> Self {
        let known_ids: HashSet<String> = changes.iter().map(|c| c.id.clone()).collect();
        let change_states: Vec<ChangeState> = changes
            .iter()
            .map(|c| ChangeState::from_change(c, true)) // Default: all selected
            .collect();

        let mut list_state = ListState::default();
        if !change_states.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            mode: AppMode::Select,
            changes: change_states,
            cursor_index: 0,
            list_state,
            current_change: None,
            error_change_id: None,
            logs: Vec::new(),
            last_refresh: Instant::now(),
            new_change_count: 0,
            known_change_ids: known_ids,
            should_quit: false,
            warning_message: None,
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
    pub fn toggle_selection(&mut self) -> Option<TuiCommand> {
        if self.changes.is_empty() || self.cursor_index >= self.changes.len() {
            return None;
        }

        let change = &mut self.changes[self.cursor_index];

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
            AppMode::Completed | AppMode::Error => None,
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
        }
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

    let result = run_tui_loop(&mut terminal, initial_changes, openspec_cmd, config).await;

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

    // Cancellation token for graceful shutdown
    let cancel_token = CancellationToken::new();

    // Start auto-refresh task
    let refresh_cmd = openspec_cmd.clone();
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
                    match openspec::list_changes(&refresh_cmd).await {
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
        // Draw the UI
        terminal.draw(|frame| render(frame, &mut app))?;

        // Handle events with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                            break;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.cursor_up();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.cursor_down();
                        }
                        KeyCode::Char(' ') => {
                            if let Some(cmd) = app.toggle_selection() {
                                let _ = cmd_tx.send(cmd).await;
                            }
                        }
                        KeyCode::F(5) => {
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
                                    orchestrator_cancel = Some(orch_cancel.clone());

                                    orchestrator_handle = Some(tokio::spawn(async move {
                                        run_orchestrator(
                                            selected_ids,
                                            orch_openspec_cmd,
                                            orch_config,
                                            orch_tx,
                                            orch_cancel,
                                        )
                                        .await
                                    }));
                                }
                            }
                        }
                        _ => {}
                    }
                    // Clear warning message on any key press
                    app.warning_message = None;
                }
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
                    // The orchestrator should pick this up on next iteration
                    app.add_log(LogEntry::info(format!("Queued: {}", id)));
                }
                TuiCommand::RemoveFromQueue(id) => {
                    // Log the removal (orchestrator will see the updated status)
                    app.add_log(LogEntry::info(format!("Removed from queue: {}", id)));
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

/// Run the orchestrator for selected changes
/// Uses streaming output to send log entries in real-time
/// Supports cancellation via CancellationToken for graceful shutdown
async fn run_orchestrator(
    change_ids: Vec<String>,
    openspec_cmd: String,
    config: OrchestratorConfig,
    tx: mpsc::Sender<OrchestratorEvent>,
    cancel_token: CancellationToken,
) -> Result<()> {
    use crate::agent::OutputLine;
    use crate::hooks::{HookContext, HookRunner, HookType};
    use crate::openspec;

    let hooks = HookRunner::new(config.get_hooks());
    let agent = AgentRunner::new(config);

    let total_changes = change_ids.len();
    let mut iteration: u32 = 0;
    let mut first_apply_executed = false;

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

    for change_id in change_ids {
        iteration += 1;
        // Check for cancellation before starting each change
        if cancel_token.is_cancelled() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Notify processing started
        let _ = tx
            .send(OrchestratorEvent::ProcessingStarted(change_id.clone()))
            .await;

        // Get current change state
        let changes = openspec::list_changes(&openspec_cmd).await?;
        let change = changes.iter().find(|c| c.id == change_id);

        let queue_size = total_changes - iteration as usize + 1;

        if let Some(change) = change {
            if change.is_complete() {
                // Run on_change_complete hook
                let complete_context =
                    HookContext::new(iteration, total_changes, queue_size, false).with_change(
                        &change_id,
                        change.completed_tasks,
                        change.total_tasks,
                    );
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
                    HookContext::new(iteration, total_changes, queue_size, false).with_change(
                        &change_id,
                        change.completed_tasks,
                        change.total_tasks,
                    );
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

                // Debug: Log the archive command
                let archive_cmd = agent.get_archive_command().to_string();
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Archive command: {}",
                        archive_cmd
                    ))))
                    .await;

                // Run archive command with streaming output
                let (mut child, mut output_rx) = agent.run_archive_streaming(&change_id).await?;

                // Stream output to TUI log, with cancellation support
                loop {
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            // Kill child process on cancellation
                            let _ = child.kill().await;
                            let _ = tx
                                .send(OrchestratorEvent::Log(LogEntry::warn(
                                    "Process killed due to cancellation".to_string(),
                                )))
                                .await;
                            return Ok(());
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
                    // Run post_archive hook
                    let post_archive_context =
                        HookContext::new(iteration, total_changes, queue_size - 1, false)
                            .with_change(&change_id, change.completed_tasks, change.total_tasks);
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
                        .send(OrchestratorEvent::ChangeArchived(change_id.clone()))
                        .await;
                } else {
                    // Run on_error hook
                    let error_msg = format!("Archive failed with exit code: {:?}", status.code());
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
                }
            } else {
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
                let (mut child, mut output_rx) = agent.run_apply_streaming(&change_id).await?;

                // Stream output to TUI log, with cancellation support
                loop {
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            // Kill child process on cancellation
                            let _ = child.kill().await;
                            let _ = tx
                                .send(OrchestratorEvent::Log(LogEntry::warn(
                                    "Process killed due to cancellation".to_string(),
                                )))
                                .await;
                            return Ok(());
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

                    let _ = tx
                        .send(OrchestratorEvent::ProcessingCompleted(change_id.clone()))
                        .await;

                    // Re-check if change is now complete and needs archiving
                    let changes_after = openspec::list_changes(&openspec_cmd).await?;
                    if let Some(updated_change) = changes_after.iter().find(|c| c.id == change_id) {
                        if updated_change.is_complete() {
                            // Run on_change_complete hook
                            let complete_context =
                                HookContext::new(iteration, total_changes, queue_size, false)
                                    .with_change(
                                        &change_id,
                                        updated_change.completed_tasks,
                                        updated_change.total_tasks,
                                    );
                            if let Err(e) = hooks
                                .run_hook(HookType::OnChangeComplete, &complete_context)
                                .await
                            {
                                let _ = tx
                                    .send(OrchestratorEvent::ProcessingError {
                                        id: change_id.clone(),
                                        error: format!("on_change_complete hook failed: {}", e),
                                    })
                                    .await;
                                break;
                            }

                            // Run pre_archive hook
                            let pre_archive_context =
                                HookContext::new(iteration, total_changes, queue_size, false)
                                    .with_change(
                                        &change_id,
                                        updated_change.completed_tasks,
                                        updated_change.total_tasks,
                                    );
                            if let Err(e) = hooks
                                .run_hook(HookType::PreArchive, &pre_archive_context)
                                .await
                            {
                                let _ = tx
                                    .send(OrchestratorEvent::ProcessingError {
                                        id: change_id.clone(),
                                        error: format!("pre_archive hook failed: {}", e),
                                    })
                                    .await;
                                break;
                            }

                            // Archive the now-complete change
                            let _ = tx
                                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                                    "Change complete, archiving: {}",
                                    change_id
                                ))))
                                .await;

                            let (mut archive_child, mut archive_rx) =
                                agent.run_archive_streaming(&change_id).await?;

                            // Stream archive output
                            loop {
                                tokio::select! {
                                    _ = cancel_token.cancelled() => {
                                        let _ = archive_child.kill().await;
                                        let _ = tx
                                            .send(OrchestratorEvent::Log(LogEntry::warn(
                                                "Archive process killed due to cancellation".to_string(),
                                            )))
                                            .await;
                                        return Ok(());
                                    }
                                    line = archive_rx.recv() => {
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

                            let archive_status = archive_child.wait().await.map_err(|e| {
                                crate::error::OrchestratorError::AgentCommand(format!(
                                    "Failed to wait for archive process: {}",
                                    e
                                ))
                            })?;

                            if archive_status.success() {
                                let _ = tx
                                    .send(OrchestratorEvent::ChangeArchived(change_id.clone()))
                                    .await;
                            } else {
                                let _ = tx
                                    .send(OrchestratorEvent::ProcessingError {
                                        id: change_id.clone(),
                                        error: format!(
                                            "Archive failed with exit code: {:?}",
                                            archive_status.code()
                                        ),
                                    })
                                    .await;
                            }
                        }
                    }
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
                }
            }
        } else {
            let _ = tx
                .send(OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: "Change not found".to_string(),
                })
                .await;
        }
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

    let elapsed = app.last_refresh.elapsed().as_secs();
    let next_refresh = AUTO_REFRESH_INTERVAL_SECS.saturating_sub(elapsed);

    let header_text = Line::from(vec![
        Span::styled("OpenSpec Orchestrator", Style::default().fg(Color::White)),
        Span::raw("  "),
        Span::styled(
            format!("[{}]", mode_text),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Auto-refresh: {}s ↻", next_refresh),
            Style::default().fg(Color::DarkGray),
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
            let checkbox = if change.selected { "[x]" } else { "[ ]" };
            let cursor = if i == app.cursor_index { "►" } else { " " };
            let new_badge = if change.is_new { " NEW" } else { "" };

            let line = Line::from(vec![
                Span::styled(
                    format!("{} {} ", checkbox, cursor),
                    Style::default().fg(if change.selected {
                        Color::Green
                    } else {
                        Color::White
                    }),
                ),
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
                .title(" Changes (↑↓: move, Space: toggle, F5: run, q: quit) ")
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
                    format!("[{:>3.0}%]", change.progress_percent())
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
                    format!(" {:>12}", status_text),
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
                .title(" Changes (Space: toggle queue) ")
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

    let content = Line::from(vec![
        Span::styled(current_text, Style::default().fg(current_color)),
        Span::raw("  |  "),
        Span::styled(status_text, Style::default().fg(status_color)),
    ]);

    let status = Paragraph::new(content).block(
        Block::default()
            .title(" Status ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(status, area);
}

/// Render logs panel
fn render_logs(frame: &mut Frame, app: &AppState, area: Rect) {
    // Calculate available width for message (subtract borders, timestamp, and padding)
    // Timestamp format: "HH:MM:SS " = 9 chars, borders = 2 chars
    let available_width = (area.width as usize).saturating_sub(2 + 9 + 1);

    let log_items: Vec<Line> = app
        .logs
        .iter()
        .rev()
        .take((area.height as usize).saturating_sub(2))
        .rev()
        .map(|entry| {
            // Truncate message to fit in available width
            let message = if entry.message.len() > available_width {
                format!("{}...", &entry.message[..available_width.saturating_sub(3)])
            } else {
                entry.message.clone()
            };

            Line::from(vec![
                Span::styled(
                    format!("{} ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(message, Style::default().fg(entry.color)),
            ])
        })
        .collect();

    let logs = Paragraph::new(log_items).block(
        Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(logs, area);
}

/// Render footer in selection mode
fn render_footer_select(frame: &mut Frame, app: &AppState, area: Rect) {
    let selected = app.selected_count();
    let new_count = app.new_change_count;

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
    } else {
        spans.push(Span::styled(
            "Press F5 to start processing",
            Style::default().fg(Color::Cyan),
        ));
    }

    let footer = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(footer, area);
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
        }
    }

    #[test]
    fn test_app_state_new() {
        let changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 0, 3),
        ];

        let app = AppState::new(changes);

        assert_eq!(app.mode, AppMode::Select);
        assert_eq!(app.changes.len(), 2);
        assert_eq!(app.cursor_index, 0);
        assert!(app.changes[0].selected);
        assert!(app.changes[1].selected);
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
        let changes = vec![create_test_change("a", 0, 1)];

        let mut app = AppState::new(changes);

        assert!(app.changes[0].selected);

        app.toggle_selection();
        assert!(!app.changes[0].selected);

        app.toggle_selection();
        assert!(app.changes[0].selected);
    }

    #[test]
    fn test_selected_count() {
        let changes = vec![
            create_test_change("a", 0, 1),
            create_test_change("b", 0, 1),
            create_test_change("c", 0, 1),
        ];

        let mut app = AppState::new(changes);

        assert_eq!(app.selected_count(), 3);

        app.toggle_selection(); // Deselect first
        assert_eq!(app.selected_count(), 2);
    }

    #[test]
    fn test_start_processing_with_selection() {
        let changes = vec![create_test_change("a", 0, 1)];

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
        let changes = vec![create_test_change("a", 0, 1)];
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
        let changes = vec![create_test_change("a", 0, 1)];
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
        let changes = vec![create_test_change("a", 0, 1)];
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
        let changes = vec![create_test_change("a", 0, 1)];
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
        let changes = vec![create_test_change("a", 0, 1)];
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
        let changes = vec![create_test_change("a", 0, 1)];
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
        let changes = vec![create_test_change("a", 0, 1)];
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
        let changes = vec![create_test_change("a", 0, 1), create_test_change("b", 0, 2)];
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
        let changes = vec![create_test_change("a", 0, 1)];
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
        app.last_refresh = std::time::Instant::now() - Duration::from_secs(AUTO_REFRESH_INTERVAL_SECS + 1);

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
    fn test_change_state_is_new_default_false() {
        let change = create_test_change("test", 1, 2);
        // By default, changes created via from_change should not be new
        // since selected=true implies initial state
        let state = ChangeState::from_change(&change, true);
        assert!(!state.is_new);
    }
}
