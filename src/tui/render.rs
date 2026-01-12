//! Rendering functions for the TUI
//!
//! Contains all render_* functions for drawing the UI.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use std::time::Duration;

use super::state::AppState;
use super::types::{AppMode, QueueStatus};
use super::utils::{get_version_string, truncate_to_display_width};

/// Determine checkbox display and color for a change item
///
/// Returns (checkbox_text, checkbox_color) based on the change's status.
/// Archived changes are always shown as gray "[x]" to indicate they are
/// no longer actionable.
fn get_checkbox_display(
    queue_status: &QueueStatus,
    is_approved: bool,
    is_selected: bool,
) -> (&'static str, Color) {
    if matches!(queue_status, QueueStatus::Archived) {
        ("[x]", Color::DarkGray) // Archived - grayed out
    } else if !is_approved {
        ("[ ]", Color::Gray) // Unapproved
    } else if is_selected {
        ("[x]", Color::Green) // Selected/In queue
    } else {
        ("[@]", Color::Yellow) // Approved but not selected
    }
}

/// Format a duration as a human-readable string (e.g., "1m 23s", "45s")
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs >= 3600 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h {:02}m", hours, mins)
    } else if secs >= 60 {
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        format!("{}m {:02}s", mins, remaining_secs)
    } else {
        format!("{}s", secs)
    }
}

/// Spinner characters for processing animation (Braille dot pattern)
pub const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Render the TUI
pub fn render(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();

    // Check minimum terminal size
    if area.width < 60 || area.height < 15 {
        let warning = Paragraph::new("Terminal too small. Minimum: 60x15")
            .style(Style::default().fg(Color::Red));
        frame.render_widget(warning, area);
        return;
    }

    // Show logs panel when logs exist, regardless of mode
    if app.logs.is_empty() {
        render_select_mode(frame, app, area);
    } else {
        render_running_mode(frame, app, area);
    }

    // Render proposal modal on top if in Proposing mode
    if app.mode == AppMode::Proposing {
        render_propose_modal(frame, app, area);
    }

    // Render QR popup on top if in QrPopup mode
    if app.mode == AppMode::QrPopup {
        render_qr_popup(frame, app, area);
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
        AppMode::Stopping => "Stopping...",
        AppMode::Stopped => "Stopped",
        AppMode::Error => "Error",
        AppMode::Proposing => "Proposing",
        AppMode::QrPopup => "QR Code",
    };

    let mode_color = match app.mode {
        AppMode::Select => Color::Cyan,
        AppMode::Running => Color::Yellow,
        AppMode::Stopping => Color::Yellow,
        AppMode::Stopped => Color::DarkGray,
        AppMode::Error => Color::Red,
        AppMode::Proposing => Color::Magenta,
        AppMode::QrPopup => Color::Green,
    };

    // Build header spans
    let mut header_spans = vec![
        Span::styled("OpenSpec Orchestrator", Style::default().fg(Color::White)),
        Span::raw("  "),
        Span::styled(
            format!("[{}]", mode_text),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
    ];

    // Add parallel mode badge if enabled
    if app.parallel_mode {
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled(
            format!("[parallel:{}:{}]", app.max_concurrent, app.vcs_backend),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let header_text = Line::from(header_spans);

    let version = get_version_string();
    let version_width = version.len() as u16 + 2; // +2 for padding

    // Split area into left content and right-aligned version
    let chunks =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(version_width)]).split(area);

    // Render left content (title and mode) with left and top/bottom borders
    let left_header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(left_header, chunks[0]);

    // Render right content (version) with right and top/bottom borders
    let right_header = Paragraph::new(Line::from(vec![Span::styled(
        version,
        Style::default().fg(Color::DarkGray),
    )]))
    .block(
        Block::default()
            .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(right_header, chunks[1]);
}

/// Render changes list in selection mode
fn render_changes_list_select(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .changes
        .iter()
        .enumerate()
        .map(|(i, change)| {
            // Checkbox display (Select mode):
            // [ ] - unapproved (cannot be selected)
            // [@] - approved but not selected (ready to select)
            // [x] - selected/reserved (will become Queued when F5 is pressed)
            // [x] (gray) - archived (processing complete, no longer actionable)
            let (checkbox, checkbox_color) =
                get_checkbox_display(&change.queue_status, change.is_approved, change.selected);

            let cursor = if i == app.cursor_index { "►" } else { " " };
            let new_badge = if change.is_new { " NEW" } else { "" };

            // Use brighter colors for selected row to ensure visibility on DarkGray background
            let is_selected_row = i == app.cursor_index;
            let dim_color = if is_selected_row {
                Color::Gray // Brighter than DarkGray for visibility on selected row
            } else {
                Color::DarkGray
            };

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
                        Color::Gray
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
                    Style::default().fg(dim_color),
                ),
                Span::styled(
                    format!("  {:>5.1}%", change.progress_percent()),
                    Style::default().fg(Color::Cyan),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    // Build dynamic key hints based on current state
    let has_selection = !app.changes.is_empty();
    let has_queue = app.changes.iter().any(|c| c.selected);
    let current_item = if has_selection && app.cursor_index < app.changes.len() {
        Some(&app.changes[app.cursor_index])
    } else {
        None
    };

    let mut keys = vec!["↑↓/jk: move"];
    if let Some(item) = current_item {
        keys.push(if item.selected {
            "Space: unqueue"
        } else {
            "Space: queue"
        });
        keys.push(if item.is_approved {
            "@: unapprove"
        } else {
            "@: approve"
        });
        keys.push("e: edit");
    }
    if has_queue {
        keys.push("F5: run");
    }
    // Show parallel toggle hint only if parallel execution is available
    if app.parallel_available {
        keys.push(if app.parallel_mode {
            "=: sequential"
        } else {
            "=: parallel"
        });
    }
    // Show QR code hint if web server is enabled
    if app.web_url.is_some() {
        keys.push("w: QR");
    }

    let title = format!(" Changes ({}) ", keys.join(", "));

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
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
        .enumerate()
        .map(|(i, change)| {
            // Checkbox display (Running mode - same symbols, different meaning):
            // [ ] - unapproved (cannot be added to queue)
            // [@] - approved but not in queue
            // [x] - in queue or being processed
            // [x] (gray) - archived (processing complete, no longer actionable)
            // Note: In Running mode, queue_status shows actual state (Queued/Processing/etc.)
            let (checkbox, checkbox_color) =
                get_checkbox_display(&change.queue_status, change.is_approved, change.selected);

            let cursor = if i == app.cursor_index { "►" } else { " " };
            let new_badge = if change.is_new { " NEW" } else { "" };

            // Use brighter colors for selected row to ensure visibility on DarkGray background
            let is_selected_row = i == app.cursor_index;
            let dim_color = if is_selected_row {
                Color::Gray // Brighter than DarkGray for visibility on selected row
            } else {
                Color::DarkGray
            };

            let status_text = match &change.queue_status {
                QueueStatus::Processing => {
                    format!("{} [{:>3.0}%]", spinner_char, change.progress_percent())
                }
                QueueStatus::Archiving => {
                    format!("{} [{}]", spinner_char, change.queue_status.display())
                }
                QueueStatus::Completed | QueueStatus::Archived | QueueStatus::Error(_) => {
                    format!("[{}]", change.queue_status.display())
                }
                status => format!("[{}]", status.display()),
            };

            let elapsed_text = if let Some(elapsed) = change.elapsed_time {
                format_duration(elapsed)
            } else if let Some(started) = change.started_at {
                format_duration(started.elapsed())
            } else {
                "--".to_string()
            };

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
                        Color::Gray
                    }),
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
                    Style::default().fg(dim_color),
                ),
                Span::styled(
                    format!("  {:>7}", elapsed_text),
                    Style::default().fg(dim_color),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    // Build dynamic key hints based on current state (same logic as select mode)
    let has_selection = !app.changes.is_empty();
    let current_item = if has_selection && app.cursor_index < app.changes.len() {
        Some(&app.changes[app.cursor_index])
    } else {
        None
    };

    let mut keys = vec!["↑↓/jk: move"];
    if let Some(item) = current_item {
        keys.push(if item.selected {
            "Space: unqueue"
        } else {
            "Space: queue"
        });
        keys.push(if item.is_approved {
            "@: unapprove"
        } else {
            "@: approve"
        });
        keys.push("e: edit");
    }
    // Show QR code hint if web server is enabled
    if app.web_url.is_some() {
        keys.push("w: QR");
    }

    let title = format!(" Changes ({}) ", keys.join(", "));

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
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
    // Check if all queued changes have completed
    let all_completed = !app.logs.is_empty()
        && app.mode == AppMode::Select
        && app.changes.iter().any(|c| {
            matches!(
                c.queue_status,
                QueueStatus::Completed | QueueStatus::Archived
            )
        });

    let (current_text, current_color) = match app.mode {
        AppMode::Error => {
            let error_id = app.error_change_id.as_deref().unwrap_or("unknown");
            (format!("Error in: {}", error_id), Color::Red)
        }
        AppMode::Select if all_completed => ("Done".to_string(), Color::Green),
        AppMode::Select => ("Ready".to_string(), Color::DarkGray),
        AppMode::Stopped => ("Stopped".to_string(), Color::DarkGray),
        AppMode::Proposing => ("Proposing".to_string(), Color::Magenta),
        AppMode::QrPopup => ("QR Code".to_string(), Color::Green),
        AppMode::Running | AppMode::Stopping => {
            // Count changes that are currently processing or archiving
            let processing_count = app
                .changes
                .iter()
                .filter(|c| {
                    matches!(
                        c.queue_status,
                        QueueStatus::Processing | QueueStatus::Archiving
                    )
                })
                .count();
            if processing_count > 1 {
                (format!("Running {}", processing_count), Color::Cyan)
            } else if processing_count == 1 {
                // Show the single change being processed
                let current = app
                    .changes
                    .iter()
                    .find(|c| {
                        matches!(
                            c.queue_status,
                            QueueStatus::Processing | QueueStatus::Archiving
                        )
                    })
                    .map(|c| c.id.as_str())
                    .unwrap_or("unknown");
                (format!("Status: {}", current), Color::White)
            } else {
                ("Waiting...".to_string(), Color::White)
            }
        }
    };

    let (status_text, status_color) = match app.mode {
        AppMode::Select if all_completed => {
            ("All processing completed. Press 'q' to quit.", Color::Green)
        }
        AppMode::Running => ("Processing... Esc: stop", Color::Cyan),
        AppMode::Stopping => (
            "Stopping after current change... Esc: force stop",
            Color::Yellow,
        ),
        AppMode::Stopped => (
            "Stopped. F5: resume, Space: toggle queue, q: quit",
            Color::DarkGray,
        ),
        AppMode::Select => ("", Color::White),
        AppMode::Error => ("Press F5 to retry, or 'q' to quit.", Color::Yellow),
        AppMode::Proposing => ("Enter: newline, Ctrl+S: submit, Esc: cancel", Color::Magenta),
        AppMode::QrPopup => ("Esc: close QR code", Color::Green),
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

    if let Some(started) = app.orchestration_started_at {
        let elapsed = if matches!(app.mode, AppMode::Running | AppMode::Stopping) {
            started.elapsed()
        } else {
            app.orchestration_elapsed
                .unwrap_or_else(|| started.elapsed())
        };
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled(
            format!("Elapsed {}", format_duration(elapsed)),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let content = Line::from(spans);

    // Build title with app control keys based on mode
    let title = match app.mode {
        AppMode::Running => " Status (Esc: stop, q: quit) ".to_string(),
        AppMode::Stopping => " Status (Esc: force stop, q: quit) ".to_string(),
        AppMode::Stopped => " Status (F5: resume, q: quit) ".to_string(),
        _ => " Status (q: quit) ".to_string(),
    };

    let status = Paragraph::new(content).block(
        Block::default()
            .title(title)
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

    // Colors for change_id prefixes (cycling through distinct colors)
    let change_colors = [
        Color::Cyan,
        Color::Magenta,
        Color::LightBlue,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightMagenta,
        Color::LightCyan,
    ];

    let log_items: Vec<Line> = app
        .logs
        .iter()
        .skip(start_index)
        .take(end_index - start_index)
        .map(|entry| {
            let mut spans = vec![Span::styled(
                format!("{} ", entry.timestamp),
                Style::default().fg(Color::DarkGray),
            )];

            // Add change_id prefix with color if present
            let msg_width = if let Some(ref change_id) = entry.change_id {
                // Use hash of change_id to pick a consistent color
                let color_index = change_id
                    .bytes()
                    .fold(0usize, |acc, b| acc.wrapping_add(b as usize))
                    % change_colors.len();
                let prefix_color = change_colors[color_index];
                spans.push(Span::styled(
                    format!("[{}] ", change_id),
                    Style::default()
                        .fg(prefix_color)
                        .add_modifier(Modifier::BOLD),
                ));
                // Reduce available width by prefix length
                available_width.saturating_sub(change_id.len() + 3) // "[" + "]" + " "
            } else {
                available_width
            };

            // Truncate message to fit in available width using Unicode display width
            // This correctly handles CJK characters that occupy 2 terminal columns
            let message = truncate_to_display_width(&entry.message, msg_width);
            spans.push(Span::styled(message, Style::default().fg(entry.color)));

            Line::from(spans)
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

    let footer = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(footer, area);
}

/// Render the proposal input modal
fn render_propose_modal(frame: &mut Frame, app: &mut AppState, area: Rect) {
    // Calculate modal dimensions (centered, 60% width, 50% height)
    let modal_width = (area.width * 60 / 100).clamp(40, 80);
    let modal_height = (area.height * 50 / 100).clamp(8, 20);
    let modal_x = (area.width - modal_width) / 2;
    let modal_y = (area.height - modal_height) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the modal area background
    frame.render_widget(Clear, modal_area);

    // Render textarea if available
    if let Some(ref textarea) = app.propose_textarea {
        // Build the border block first
        let block = Block::default()
            .title(" New Proposal (Enter: newline, Ctrl+S: submit, Esc: cancel) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));

        // Calculate inner area for text content
        let inner_area = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        // Render text content line by line
        let lines: Vec<Line> = textarea
            .lines()
            .iter()
            .map(|s| Line::from(s.as_str()))
            .collect();
        let text = Paragraph::new(lines);
        frame.render_widget(text, inner_area);
    }
}

/// Render the QR code popup
fn render_qr_popup(frame: &mut Frame, app: &AppState, area: Rect) {
    // Get the web URL
    let url = match &app.web_url {
        Some(url) => url.as_str(),
        None => return,
    };

    // Generate QR code
    let qr_content = match super::qr::generate_qr_string(url) {
        Ok(qr) => qr,
        Err(e) => format!("Failed to generate QR code: {}", e),
    };

    // Calculate QR code dimensions
    let qr_lines: Vec<&str> = qr_content.lines().collect();
    let qr_height = qr_lines.len() as u16;
    let qr_width = qr_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0) as u16;

    // Calculate modal dimensions (add padding for borders and title)
    let modal_width = (qr_width + 4).max(40).min(area.width - 4);
    let modal_height = (qr_height + 6).max(10).min(area.height - 4); // +6 for borders, title, URL, and instructions

    // Center the modal
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the modal area background
    frame.render_widget(Clear, modal_area);

    // Build the border block
    let block = Block::default()
        .title(" Web UI QR Code (press any key to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    // Calculate inner area for content
    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    // Split inner area into QR code and URL sections
    let content_chunks = Layout::vertical([
        Constraint::Min(1),    // QR code
        Constraint::Length(2), // URL and instructions
    ])
    .split(inner_area);

    // Render QR code (centered)
    let qr_lines: Vec<Line> = qr_content
        .lines()
        .map(|line| Line::from(Span::raw(line)))
        .collect();
    let qr_paragraph = Paragraph::new(qr_lines)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(Color::White));
    frame.render_widget(qr_paragraph, content_chunks[0]);

    // Render URL at the bottom
    let url_text = Line::from(vec![
        Span::styled("URL: ", Style::default().fg(Color::DarkGray)),
        Span::styled(url, Style::default().fg(Color::Cyan)),
    ]);
    let url_paragraph = Paragraph::new(url_text).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(url_paragraph, content_chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_checkbox_display_archived_always_gray() {
        // Archived status should always result in gray checkbox,
        // regardless of is_approved or is_selected values
        let (text, color) = get_checkbox_display(&QueueStatus::Archived, true, true);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::DarkGray);

        let (text, color) = get_checkbox_display(&QueueStatus::Archived, true, false);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::DarkGray);

        let (text, color) = get_checkbox_display(&QueueStatus::Archived, false, true);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::DarkGray);

        let (text, color) = get_checkbox_display(&QueueStatus::Archived, false, false);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::DarkGray);
    }

    #[test]
    fn test_get_checkbox_display_unapproved() {
        let (text, color) = get_checkbox_display(&QueueStatus::NotQueued, false, false);
        assert_eq!(text, "[ ]");
        assert_eq!(color, Color::Gray);
    }

    #[test]
    fn test_get_checkbox_display_approved_selected() {
        let (text, color) = get_checkbox_display(&QueueStatus::NotQueued, true, true);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);

        let (text, color) = get_checkbox_display(&QueueStatus::Queued, true, true);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_get_checkbox_display_approved_not_selected() {
        let (text, color) = get_checkbox_display(&QueueStatus::NotQueued, true, false);
        assert_eq!(text, "[@]");
        assert_eq!(color, Color::Yellow);
    }

    #[test]
    fn test_get_checkbox_display_processing_states() {
        // Processing and Completed states should show green when selected
        let (text, color) = get_checkbox_display(&QueueStatus::Processing, true, true);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);

        let (text, color) = get_checkbox_display(&QueueStatus::Completed, true, true);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);
    }
}
