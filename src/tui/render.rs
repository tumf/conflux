//! Rendering functions for the TUI
//!
//! Contains all render_* functions for drawing the UI.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use std::time::Duration;

use super::state::AppState;
use super::types::{AppMode, QueueStatus};
use super::utils::{get_version_string, truncate_to_display_width_with_suffix};

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
    if matches!(queue_status, QueueStatus::Archived | QueueStatus::Merged) {
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

/// Format a timestamp as relative time (e.g., "just now", "2m ago", "1d 12h ago")
///
/// - Less than 1 minute: "just now"
/// - 1 minute or more: "<n><unit> ago" (e.g., "2m ago", "3h ago")
/// - For times >= 1 minute: show up to 2 units (e.g., "1d 12h ago", "3h 20m ago")
/// - Units are d (days), h (hours), m (minutes)
/// - Values are truncated (no rounding up)
fn format_relative_time(created_at: &chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Utc;

    let now = Utc::now();
    let duration = now.signed_duration_since(*created_at);
    let total_seconds = duration.num_seconds();

    // Less than 1 minute
    if total_seconds < 60 {
        return "just now".to_string();
    }

    let total_minutes = total_seconds / 60;
    let total_hours = total_minutes / 60;
    let total_days = total_hours / 24;

    // Calculate up to 2 units
    if total_days > 0 {
        let remaining_hours = total_hours % 24;
        if remaining_hours > 0 {
            format!("{}d {}h ago", total_days, remaining_hours)
        } else {
            format!("{}d ago", total_days)
        }
    } else if total_hours > 0 {
        let remaining_minutes = total_minutes % 60;
        if remaining_minutes > 0 {
            format!("{}h {}m ago", total_hours, remaining_minutes)
        } else {
            format!("{}h ago", total_hours)
        }
    } else {
        // Only minutes
        format!("{}m ago", total_minutes)
    }
}

/// Spinner characters for processing animation (Braille dot pattern)
pub const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Render the TUI
pub fn render(frame: &mut Frame, app: &mut AppState) {
    use crate::tui::types::ViewMode;

    let area = frame.area();

    // Check minimum terminal size
    if area.width < 60 || area.height < 15 {
        let warning = Paragraph::new("Terminal too small. Minimum: 60x15")
            .style(Style::default().fg(Color::Red));
        frame.render_widget(warning, area);
        return;
    }

    // Route to appropriate view based on ViewMode
    match app.view_mode {
        ViewMode::Changes => {
            // Show logs panel when logs exist, regardless of mode
            if app.logs.is_empty() {
                render_select_mode(frame, app, area);
            } else {
                render_running_mode(frame, app, area);
            }
        }
        ViewMode::Worktrees => {
            render_worktree_view(frame, app, area);
        }
    }

    // Render QR popup on top if in QrPopup mode
    if app.mode == AppMode::QrPopup {
        render_qr_popup(frame, app, area);
    }

    // Render worktree delete confirmation modal on top if needed
    if app.mode == AppMode::ConfirmWorktreeDelete {
        render_worktree_delete_confirm(frame, app, area);
    }

    // Render warning popup on top if present
    if app.warning_popup.is_some() {
        render_warning_popup(frame, app, area);
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
        Constraint::Length(20), // Logs (2x height for better visibility)
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
    let active_count = app
        .changes
        .iter()
        .filter(|c| c.queue_status.is_active())
        .count();

    // Per spec (update-tui-status-display):
    // - Ready: when in Select mode
    // - Running <count>: when changes are processing (count > 0)
    // - No status: in Stopped and Error modes
    let (mode_text, mode_color, show_status) = match app.mode {
        AppMode::Select | AppMode::Running | AppMode::Stopping => {
            if active_count > 0 {
                (format!("Running {}", active_count), Color::Yellow, true)
            } else {
                ("Ready".to_string(), Color::Cyan, true)
            }
        }
        AppMode::Stopped | AppMode::Error => {
            // Hide status in Stopped and Error modes per spec
            (String::new(), Color::White, false)
        }
        AppMode::ConfirmWorktreeDelete => ("Confirm Delete".to_string(), Color::Yellow, true),
        AppMode::QrPopup => ("QR Code".to_string(), Color::Green, true),
    };

    // Build header spans
    let mut header_spans = vec![Span::styled("Conflux", Style::default().fg(Color::White))];

    // Add status label only when show_status is true
    if show_status && !mode_text.is_empty() {
        header_spans.push(Span::raw("  "));
        header_spans.push(Span::styled(
            format!("[{}]", mode_text),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ));
    }

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
            // [x] - selected (will become Queued when F5 is pressed)
            // [x] (gray) - archived (processing complete, no longer actionable)
            // Note: 'selected' field indicates selection for next run
            let is_archived = matches!(
                change.queue_status,
                QueueStatus::Archived | QueueStatus::Merged
            );
            let show_uncommitted_badge = app.parallel_mode
                && !change.is_parallel_eligible
                && !is_archived
                && matches!(
                    change.queue_status,
                    QueueStatus::NotQueued | QueueStatus::Queued
                );
            let is_parallel_blocked = show_uncommitted_badge;
            let (checkbox, checkbox_color) = if is_parallel_blocked {
                ("[ ]", Color::DarkGray)
            } else {
                get_checkbox_display(&change.queue_status, change.is_approved, change.selected)
            };

            let cursor = if i == app.cursor_index { "►" } else { " " };
            let worktree_badge = if change.has_worktree { " WT" } else { "" };
            let worktree_color = if is_parallel_blocked {
                Color::DarkGray
            } else {
                Color::Green
            };
            let new_badge = if change.is_new { " NEW" } else { "" };
            let uncommitted_badge = if show_uncommitted_badge {
                " UNCOMMITED"
            } else {
                ""
            };

            // Use brighter colors for selected row to ensure visibility on DarkGray background
            let is_selected_row = i == app.cursor_index;
            let dim_color = if is_parallel_blocked {
                Color::DarkGray
            } else if is_selected_row {
                Color::Gray // Brighter than DarkGray for visibility on selected row
            } else {
                Color::DarkGray
            };

            let name_color = if is_parallel_blocked {
                Color::DarkGray
            } else if change.is_approved {
                Color::White
            } else {
                Color::Gray
            };

            let mut spans = vec![
                Span::styled(
                    format!("{} {} ", checkbox, cursor),
                    Style::default().fg(checkbox_color),
                ),
                Span::styled(
                    format!("{:<25}", change.id),
                    Style::default().fg(name_color),
                ),
                Span::styled(
                    worktree_badge,
                    Style::default()
                        .fg(worktree_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    new_badge,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    uncommitted_badge,
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
            ];

            // Add log preview if available
            if let Some(log) = app.get_latest_log_for_change(&change.id) {
                // Calculate actual occupied width dynamically
                let checkbox_cursor_text = format!("{} {} ", checkbox, cursor);
                let checkbox_cursor_width = checkbox_cursor_text.len(); // Actual: "[x] ► " is 6 chars
                let id_text = format!("{:<25}", change.id);
                let id_width = id_text.len(); // max(25, change.id.len())
                let worktree_badge_width = if change.has_worktree { 3 } else { 0 }; // " WT"
                let new_badge_width = if change.is_new { 4 } else { 0 }; // " NEW"
                let uncommitted_badge_width = if show_uncommitted_badge { 11 } else { 0 }; // " UNCOMMITED"
                let tasks_text =
                    format!(" {}/{} tasks", change.completed_tasks, change.total_tasks);
                let tasks_width = tasks_text.len();
                let percent_text = format!("  {:>5.1}%", change.progress_percent());
                let percent_width = percent_text.len();
                let list_border_width = 2; // List widget border

                let base_width = checkbox_cursor_width
                    + id_width
                    + worktree_badge_width
                    + new_badge_width
                    + uncommitted_badge_width
                    + tasks_width
                    + percent_width
                    + list_border_width;

                let available = (area.width as usize).saturating_sub(base_width);

                // Only show preview if available width >= 10 chars
                if available >= 10 {
                    // Format relative time with parentheses
                    let relative_time = format!("({})", format_relative_time(&log.created_at));

                    // Build shortened header: [operation:iteration] or [operation]
                    let header = match (&log.operation, log.iteration) {
                        (Some(op), Some(iter)) => format!(" [{}:{}]", op, iter),
                        (Some(op), None) => format!(" [{}]", op),
                        (None, _) => String::new(),
                    };

                    // Combine relative time, header, and message
                    let preview_text = if !header.is_empty() {
                        format!(" {}{} {}", relative_time, header, log.message)
                    } else {
                        format!(" {} {}", relative_time, log.message)
                    };

                    // Truncate if necessary (Unicode-safe)
                    let truncated =
                        truncate_to_display_width_with_suffix(&preview_text, available, "…");

                    // Use brighter color for selected row to ensure visibility on DarkGray background
                    let preview_color = if is_selected_row {
                        Color::Gray
                    } else {
                        Color::DarkGray
                    };

                    spans.push(Span::styled(truncated, Style::default().fg(preview_color)));
                }
            }

            ListItem::new(Line::from(spans))
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
        // Show M key hint based on resolve state (only in Select, Running, Stopped modes)
        // - When resolve is NOT running and current item is MergeWait: "M: resolve"
        // - When resolve IS running and current item is MergeWait: "M: queue resolve"
        if matches!(item.queue_status, QueueStatus::MergeWait)
            && matches!(
                app.mode,
                AppMode::Select | AppMode::Running | AppMode::Stopped
            )
        {
            if app.is_resolving {
                keys.push("M: queue resolve");
            } else {
                keys.push("M: resolve");
            }
        }
    }
    if has_queue {
        keys.push("F5: run");
    }
    keys.push("Tab: worktrees");
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
            // Checkbox display (Running/Stopped mode):
            // [ ] - unapproved (cannot be added to queue)
            // [@] - approved but not in queue / not marked
            // [x] - in queue OR marked for execution (Stopped mode)
            // [x] (gray) - archived (processing complete, no longer actionable)
            // Note: Display is driven by 'selected' field, which serves dual purpose:
            //   - Running: shows queue membership (selected=true means Queued/Processing)
            //   - Stopped: shows execution mark (selected=true, queue_status=NotQueued)
            let is_archived = matches!(
                change.queue_status,
                QueueStatus::Archived | QueueStatus::Merged
            );
            let show_uncommitted_badge = app.parallel_mode
                && !change.is_parallel_eligible
                && !is_archived
                && matches!(
                    change.queue_status,
                    QueueStatus::NotQueued | QueueStatus::Queued
                );
            let is_parallel_blocked = show_uncommitted_badge;
            let (checkbox, checkbox_color) = if is_parallel_blocked {
                ("[ ]", Color::DarkGray)
            } else {
                get_checkbox_display(&change.queue_status, change.is_approved, change.selected)
            };

            let cursor = if i == app.cursor_index { "►" } else { " " };
            let worktree_badge = if change.has_worktree { " WT" } else { "" };
            let worktree_color = if is_parallel_blocked {
                Color::DarkGray
            } else {
                Color::Green
            };
            let new_badge = if change.is_new { " NEW" } else { "" };
            let uncommitted_badge = if show_uncommitted_badge {
                " UNCOMMITED"
            } else {
                ""
            };

            // Use brighter colors for selected row to ensure visibility on DarkGray background
            let is_selected_row = i == app.cursor_index;
            let dim_color = if is_parallel_blocked {
                Color::DarkGray
            } else if is_selected_row {
                Color::Gray // Brighter than DarkGray for visibility on selected row
            } else {
                Color::DarkGray
            };

            let name_color = if is_parallel_blocked {
                Color::DarkGray
            } else if change.is_approved {
                Color::White
            } else {
                Color::Gray
            };

            // Calculate elapsed time first
            let elapsed_text = if let Some(elapsed) = change.elapsed_time {
                format_duration(elapsed)
            } else if let Some(started) = change.started_at {
                format_duration(started.elapsed())
            } else {
                "--".to_string()
            };

            // Build status text (without spinner for in-flight states)
            // For in-flight states, spinner will be prepended separately with elapsed time
            let (spinner_prefix, status_text) = match &change.queue_status {
                QueueStatus::Applying => {
                    let status = if let Some(iter) = change.iteration_number {
                        format!("[{}:{}]", change.queue_status.display(), iter)
                    } else {
                        format!("[{}]", change.queue_status.display())
                    };
                    (format!("{} ", spinner_char), status)
                }
                QueueStatus::Archiving | QueueStatus::Resolving | QueueStatus::Accepting => {
                    let status = if let Some(iter) = change.iteration_number {
                        format!("[{}:{}]", change.queue_status.display(), iter)
                    } else {
                        format!("[{}]", change.queue_status.display())
                    };
                    (format!("{} ", spinner_char), status)
                }
                QueueStatus::Archived | QueueStatus::Merged | QueueStatus::Error(_) => (
                    String::new(),
                    format!("[{}]", change.queue_status.display()),
                ),
                status => (String::new(), format!("[{}]", status.display())),
            };

            // Pre-calculate widths before moving values into Spans
            let (spinner_elapsed_width, status_only_width) = if !spinner_prefix.is_empty() {
                let spinner_elapsed_text =
                    format!(" {}{:>7} ", spinner_prefix.trim(), elapsed_text);
                (spinner_elapsed_text.len(), status_text.len())
            } else {
                let status_formatted = format!(" {:>18}", status_text);
                (0, status_formatted.len())
            };

            let mut spans = vec![
                Span::styled(
                    format!("{} {} ", checkbox, cursor),
                    Style::default().fg(checkbox_color),
                ),
                Span::styled(
                    format!("{:<25}", change.id),
                    Style::default().fg(name_color),
                ),
                Span::styled(
                    worktree_badge,
                    Style::default()
                        .fg(worktree_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    new_badge,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    uncommitted_badge,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ];

            // For in-flight states: spinner → elapsed → status
            // For other states: status only
            if !spinner_prefix.is_empty() {
                spans.push(Span::styled(
                    format!(" {}{:>7} ", spinner_prefix.trim(), elapsed_text),
                    Style::default().fg(dim_color),
                ));
                spans.push(Span::styled(
                    status_text,
                    Style::default().fg(change.queue_status.color()),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {:>18}", status_text),
                    Style::default().fg(change.queue_status.color()),
                ));
            }

            // For Applying status, show progress as "completed/total(percent%)"
            // For other statuses, show just "completed/total"
            let tasks_text = if matches!(change.queue_status, QueueStatus::Applying) {
                format!(
                    "  {}/{}({:.0}%)",
                    change.completed_tasks,
                    change.total_tasks,
                    change.progress_percent()
                )
            } else {
                format!("  {}/{}", change.completed_tasks, change.total_tasks)
            };
            spans.push(Span::styled(
                tasks_text.clone(),
                Style::default().fg(dim_color),
            ));

            // Add log preview if available
            if let Some(log) = app.get_latest_log_for_change(&change.id) {
                // Calculate actual occupied width dynamically
                let checkbox_cursor_text = format!("{} {} ", checkbox, cursor);
                let checkbox_cursor_width = checkbox_cursor_text.len(); // Actual: "[x] ► " is 6 chars
                let id_text = format!("{:<25}", change.id);
                let id_width = id_text.len(); // max(25, change.id.len())
                let worktree_badge_width = if change.has_worktree { 3 } else { 0 }; // " WT"
                let new_badge_width = if change.is_new { 4 } else { 0 }; // " NEW"
                let uncommitted_badge_width = if show_uncommitted_badge { 11 } else { 0 }; // " UNCOMMITED"

                // Use the actual tasks_text that was already formatted above
                let tasks_width = tasks_text.len();
                let list_border_width = 2; // List widget border

                let base_width = checkbox_cursor_width
                    + id_width
                    + worktree_badge_width
                    + new_badge_width
                    + uncommitted_badge_width
                    + spinner_elapsed_width
                    + status_only_width
                    + tasks_width
                    + list_border_width;

                let available = (area.width as usize).saturating_sub(base_width);

                // Only show preview if available width >= 10 chars
                if available >= 10 {
                    // Format relative time with parentheses
                    let relative_time = format!("({})", format_relative_time(&log.created_at));

                    // Build shortened header: [operation:iteration] or [operation]
                    let header = match (&log.operation, log.iteration) {
                        (Some(op), Some(iter)) => format!(" [{}:{}]", op, iter),
                        (Some(op), None) => format!(" [{}]", op),
                        (None, _) => String::new(),
                    };

                    // Combine relative time, header, and message
                    let preview_text = if !header.is_empty() {
                        format!(" {}{} {}", relative_time, header, log.message)
                    } else {
                        format!(" {} {}", relative_time, log.message)
                    };

                    // Truncate if necessary (Unicode-safe)
                    let truncated =
                        truncate_to_display_width_with_suffix(&preview_text, available, "…");

                    // Use brighter color for selected row to ensure visibility on DarkGray background
                    let preview_color = if is_selected_row {
                        Color::Gray
                    } else {
                        Color::DarkGray
                    };

                    spans.push(Span::styled(truncated, Style::default().fg(preview_color)));
                }
            }

            ListItem::new(Line::from(spans))
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
        // Show M key hint based on resolve state (only in Select, Running, Stopped modes)
        // - When resolve is NOT running and current item is MergeWait: "M: resolve"
        // - When resolve IS running and current item is MergeWait: "M: queue resolve"
        if matches!(item.queue_status, QueueStatus::MergeWait)
            && matches!(
                app.mode,
                AppMode::Select | AppMode::Running | AppMode::Stopped
            )
        {
            if app.is_resolving {
                keys.push("M: queue resolve");
            } else {
                keys.push("M: resolve");
            }
        }
    }
    keys.push("Tab: worktrees");
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
    // Per spec (update-tui-status-display):
    // Status line shows only progress bar + elapsed time
    // Progress is calculated from selected (x) changes in all modes

    // Calculate progress based on selected changes only
    let (total_tasks, completed_tasks) = app
        .changes
        .iter()
        .filter(|c| c.selected) // Only count selected (x) changes
        .fold((0u32, 0u32), |(total, completed), c| {
            (total + c.total_tasks, completed + c.completed_tasks)
        });

    let mut spans = vec![];

    // Show progress bar if there are selected changes with tasks
    if total_tasks > 0 {
        let percent = (completed_tasks as f32 / total_tasks as f32) * 100.0;
        let bar_width = 20;
        let filled = ((percent / 100.0) * bar_width as f32) as usize;
        let empty = bar_width - filled;
        let progress_text = format!(
            "[{}{}] {:>5.1}% ({}/{})",
            "█".repeat(filled),
            "░".repeat(empty),
            percent,
            completed_tasks,
            total_tasks
        );
        spans.push(Span::styled(
            progress_text,
            Style::default().fg(Color::Cyan),
        ));
    }

    // Show accumulated running time (elapsed)
    // Per spec: accumulated running duration in Ready or Stopped mode
    if let Some(started) = app.orchestration_started_at {
        let elapsed = if matches!(app.mode, AppMode::Running | AppMode::Stopping) {
            // Use current running time
            started.elapsed()
        } else {
            // Use accumulated time from last run
            app.orchestration_elapsed
                .unwrap_or_else(|| started.elapsed())
        };

        if !spans.is_empty() {
            spans.push(Span::raw("  |  "));
        }
        spans.push(Span::styled(
            format!("Elapsed {}", format_duration(elapsed)),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let content = Line::from(spans);

    // Build title with app control keys based on mode
    let title = match app.mode {
        AppMode::Running => " Status (Esc: stop, Ctrl+C: quit) ".to_string(),
        AppMode::Stopping => " Status (F5: continue, Esc: force stop, Ctrl+C: quit) ".to_string(),
        AppMode::Stopped => " Status (F5: resume, Ctrl+C: quit) ".to_string(),
        AppMode::ConfirmWorktreeDelete => " Status (Y/N: confirm, Ctrl+C: quit) ".to_string(),
        _ => " Status (Ctrl+C: quit) ".to_string(),
    };

    let status = Paragraph::new(content).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(status, area);
}

/// Wrap a log message with proper indentation for continuation lines
///
/// The first line starts at column 0 (after timestamp+header prefix).
/// Continuation lines are indented by `prefix_width` spaces to align with the first line.
///
/// `available_width` is the total width available after subtracting borders and timestamp.
/// `header_width` is the width of the header (e.g., "[change-id:operation]").
/// `prefix_width` is the total indentation width for continuation lines (timestamp + header).
///
/// Returns a vector of display lines (wrapped output).
fn wrap_log_message(
    message: &str,
    available_width: usize,
    header_width: usize,
    prefix_width: usize,
) -> Vec<String> {
    if available_width == 0 {
        return vec![message.to_string()];
    }

    let mut lines = Vec::new();
    let mut remaining = message;

    // First line: available_width - header_width (since line is [header][message])
    let first_width = available_width.saturating_sub(header_width);
    if first_width == 0 {
        lines.push(remaining.to_string());
        return lines;
    }

    if remaining.len() <= first_width {
        // Entire message fits on first line
        lines.push(remaining.to_string());
        return lines;
    }

    // Split first line at character boundary
    let mut split_point = first_width;
    while split_point > 0 && !remaining.is_char_boundary(split_point) {
        split_point -= 1;
    }
    if split_point == 0 {
        split_point = first_width;
    }

    lines.push(remaining[..split_point].to_string());
    remaining = &remaining[split_point..];

    // Continuation lines: indent by prefix_width, use (available_width - header_width)
    // Note: available_width already has timestamp subtracted, so we only subtract header_width
    // to get the same message width as the first line
    let indent = " ".repeat(prefix_width);
    let continuation_width = available_width.saturating_sub(header_width);

    while !remaining.is_empty() {
        if continuation_width == 0 {
            // No space for continuation, append as-is
            lines.push(format!("{}{}", indent, remaining));
            break;
        }

        if remaining.len() <= continuation_width {
            // Remaining message fits on this line
            lines.push(format!("{}{}", indent, remaining));
            break;
        }

        // Split at character boundary
        let mut split_point = continuation_width;
        while split_point > 0 && !remaining.is_char_boundary(split_point) {
            split_point -= 1;
        }
        if split_point == 0 {
            split_point = continuation_width;
        }

        lines.push(format!("{}{}", indent, &remaining[..split_point]));
        remaining = &remaining[split_point..];
    }

    lines
}

/// Render logs panel with scroll support
fn render_logs(frame: &mut Frame, app: &AppState, area: Rect) {
    // Calculate available width for message (subtract borders, timestamp, and padding)
    // Timestamp format: "HH:MM:SS " = 9 chars, borders = 2 chars
    let timestamp_width = 9; // "HH:MM:SS "
    let border_width = 2;
    let available_width = (area.width as usize).saturating_sub(border_width + timestamp_width);

    // Calculate visible area height (subtract borders)
    let visible_height = (area.height as usize).saturating_sub(2);

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

    // Pre-render all logs to calculate total display lines
    // Each entry stores: (timestamp, header_spans, message_lines, color)
    struct RenderedLog {
        timestamp: String,
        timestamp_style: Style,
        header: String,
        header_style: Style,
        message_lines: Vec<String>,
        message_style: Style,
    }

    let rendered_logs: Vec<RenderedLog> = app
        .logs
        .iter()
        .map(|entry| {
            let timestamp = format!("{} ", entry.timestamp);
            let timestamp_style = Style::default().fg(Color::DarkGray);

            // Build header and calculate prefix width
            let (header, header_style, prefix_width) = if let Some(ref operation) = entry.operation
            {
                // Use hash of change_id (if present) to pick a consistent color
                let color_index = if let Some(ref change_id) = entry.change_id {
                    change_id
                        .bytes()
                        .fold(0usize, |acc, b| acc.wrapping_add(b as usize))
                        % change_colors.len()
                } else {
                    0
                };
                let prefix_color = change_colors[color_index];

                // Build header with change_id when present
                let header = match (&entry.change_id, entry.iteration) {
                    (Some(change_id), Some(iter)) => {
                        format!("[{}:{}:{}] ", change_id, operation, iter)
                    }
                    (Some(change_id), None) => format!("[{}:{}] ", change_id, operation),
                    (None, Some(iter)) => format!("[{}:{}] ", operation, iter),
                    (None, None) => {
                        // Analysis logs must always have iteration
                        if operation == "analysis" {
                            format!("[{}:1] ", operation)
                        } else {
                            format!("[{}] ", operation)
                        }
                    }
                };

                let prefix_width = timestamp.len() + header.len();
                let header_style = Style::default()
                    .fg(prefix_color)
                    .add_modifier(Modifier::BOLD);

                (header, header_style, prefix_width)
            } else {
                let prefix_width = timestamp.len();
                (String::new(), Style::default(), prefix_width)
            };

            // Wrap message with indentation
            // available_width is already (total_width - border - timestamp)
            // Pass header.len() separately to avoid double-subtraction in continuation lines
            let message_lines =
                wrap_log_message(&entry.message, available_width, header.len(), prefix_width);
            let message_style = Style::default().fg(entry.color);

            RenderedLog {
                timestamp,
                timestamp_style,
                header,
                header_style,
                message_lines,
                message_style,
            }
        })
        .collect();

    // Calculate total display lines (sum of all wrapped lines)
    let total_display_lines: usize = rendered_logs.iter().map(|r| r.message_lines.len()).sum();

    // Convert log_scroll_offset (log-count-based) to display-line-based offset
    // log_scroll_offset = 0 means show the most recent logs at the bottom
    // log_scroll_offset = N means skip N logs from the bottom
    let total_logs = rendered_logs.len();
    let skipped_logs = app.log_scroll_offset.min(total_logs);

    // Calculate display line offset by summing up the wrapped lines of the skipped logs
    let display_line_offset: usize = rendered_logs
        .iter()
        .rev()
        .take(skipped_logs)
        .map(|r| r.message_lines.len())
        .sum();

    // Calculate visible range based on display lines
    let end_line = total_display_lines.saturating_sub(display_line_offset);
    let start_line = end_line.saturating_sub(visible_height);

    // Convert line range to log entries and build Line widgets
    let mut log_items: Vec<Line> = Vec::new();
    let mut current_line = 0;

    for rendered in &rendered_logs {
        let entry_line_count = rendered.message_lines.len();
        let entry_end = current_line + entry_line_count;

        // Check if this entry overlaps with visible range
        if entry_end > start_line && current_line < end_line {
            // Determine which lines of this entry are visible
            let visible_start_in_entry = start_line.saturating_sub(current_line);
            let visible_end_in_entry = entry_line_count.min(end_line.saturating_sub(current_line));

            for (line_idx, message_line) in rendered.message_lines.iter().enumerate() {
                if line_idx >= visible_start_in_entry && line_idx < visible_end_in_entry {
                    let mut spans = Vec::new();

                    if line_idx == 0 {
                        // First line: include timestamp and header
                        spans.push(Span::styled(
                            rendered.timestamp.clone(),
                            rendered.timestamp_style,
                        ));
                        if !rendered.header.is_empty() {
                            spans
                                .push(Span::styled(rendered.header.clone(), rendered.header_style));
                        }
                        spans.push(Span::styled(message_line.clone(), rendered.message_style));
                    } else {
                        // Continuation line: message_line already has indentation
                        spans.push(Span::styled(message_line.clone(), rendered.message_style));
                    }

                    log_items.push(Line::from(spans));
                }
            }
        }

        current_line = entry_end;
    }

    // Build title with scroll position indicator and auto-scroll status
    let auto_scroll_indicator = if app.log_auto_scroll { "▼" } else { "⏸" };
    let title = if total_display_lines > visible_height {
        let visible_start = start_line + 1;
        let visible_end = end_line;
        format!(
            " Logs [{}-{}/{}] logs_off={} {} ",
            visible_start,
            visible_end,
            total_display_lines,
            app.log_scroll_offset,
            auto_scroll_indicator
        )
    } else {
        format!(
            " Logs logs_off={} {} ",
            app.log_scroll_offset, auto_scroll_indicator
        )
    };

    // Do NOT use Paragraph::wrap - we handle wrapping manually
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
            "Add new changes to get started",
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

/// Render worktree view
fn render_worktree_view(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(5),    // Worktree list
        Constraint::Length(3), // Footer
    ])
    .split(area);

    // Header
    render_header(frame, app, chunks[0]);

    // Worktree list
    render_worktree_list(frame, app, chunks[1]);

    // Footer
    render_footer_worktree(frame, app, chunks[2]);
}

/// Render the worktree list
fn render_worktree_list(frame: &mut Frame, app: &mut AppState, area: Rect) {
    use crate::tui::types::ViewMode;

    if app.view_mode != ViewMode::Worktrees {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Worktrees ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if app.worktrees.is_empty() {
        let empty_msg = Paragraph::new("No worktrees found")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner_area);
        return;
    }

    let items: Vec<ListItem> = app
        .worktrees
        .iter()
        .enumerate()
        .map(|(idx, wt)| {
            let is_selected = idx == app.worktree_cursor_index;

            // Build the display line
            let label = wt.display_label();
            let branch = wt.display_branch();

            // Add conflict badge if present
            let conflict_badge = if wt.has_merge_conflict() {
                format!(" ⚠{}", wt.conflict_file_count())
            } else {
                String::new()
            };

            // Main/Detached indicators
            let indicator = if wt.is_main {
                " [MAIN]"
            } else if wt.is_detached {
                " [DETACHED]"
            } else {
                ""
            };

            // Merge status indicator
            let merge_status = wt.merge_status_label();
            let merge_indicator = if !merge_status.is_empty() {
                format!(" [{}]", merge_status)
            } else {
                String::new()
            };

            let line = format!(
                "{} → {}{}{}{}",
                label, branch, indicator, merge_indicator, conflict_badge
            );

            // Style based on conflict and selection
            let mut style = Style::default();

            if wt.has_merge_conflict() {
                style = style.fg(Color::Red);
            } else if wt.is_main {
                style = style.fg(Color::Green);
            } else {
                style = style.fg(Color::White);
            }

            if is_selected {
                style = style.add_modifier(Modifier::BOLD).bg(Color::DarkGray);
            }

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    // Update list state
    app.worktree_list_state
        .select(Some(app.worktree_cursor_index));

    frame.render_stateful_widget(list, inner_area, &mut app.worktree_list_state);
}

/// Render footer for worktree view
fn render_footer_worktree(frame: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Build key hints
    let mut key_hints = vec![("Tab", "changes"), ("↑↓/jk", "navigate"), ("+", "create")];

    // Only show Delete if a non-main, non-detached worktree is selected
    if let Some(wt) = app.get_selected_worktree() {
        if !wt.is_main && !wt.is_detached {
            key_hints.push(("D", "delete"));
        }

        // Show M (merge) key only if:
        // - Not main worktree
        // - Not detached HEAD
        // - No merge conflicts
        // - Has a branch name
        // - Has commits ahead of base branch
        // - No resolve operation in progress
        if !wt.is_main
            && !wt.is_detached
            && !wt.has_merge_conflict()
            && !wt.branch.is_empty()
            && wt.has_commits_ahead
            && !app.is_resolving
            && !wt.is_merging
        {
            key_hints.push(("M", "merge"));
        }
    }

    // Show editor key if configured
    key_hints.push(("e", "editor"));

    // Show shell key if worktree_command is configured
    // Note: We'll check this in the actual implementation
    key_hints.push(("Enter", "shell"));

    key_hints.push(("Ctrl+C", "quit"));

    let hints_text = key_hints
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect::<Vec<_>>()
        .join("  ");

    // Status line
    let status = if let Some(ref msg) = app.warning_message {
        Span::styled(msg, Style::default().fg(Color::Yellow))
    } else {
        let count = app.worktrees.len();
        Span::styled(
            format!("{} worktree{}", count, if count == 1 { "" } else { "s" }),
            Style::default().fg(Color::DarkGray),
        )
    };

    let footer_line = Line::from(vec![
        status,
        Span::raw("  |  "),
        Span::styled(hints_text, Style::default().fg(Color::Cyan)),
    ]);

    let footer = Paragraph::new(footer_line).alignment(Alignment::Left);
    frame.render_widget(footer, inner_area);
}

/// Render the worktree delete confirmation modal
fn render_worktree_delete_confirm(frame: &mut Frame, app: &AppState, area: Rect) {
    use crate::tui::types::WorktreeAction;

    let Some((path, WorktreeAction::Delete)) = &app.pending_worktree_action else {
        return;
    };

    let modal_width = (area.width * 60 / 100).clamp(40, 90);
    let modal_height = (area.height * 30 / 100).clamp(7, 12);
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Delete Worktree ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let lines = vec![
        Line::from(Span::styled(
            format!("Delete worktree at '{}'?", path),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "This will remove the worktree directory permanently.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press Y to delete, N or Esc to cancel.",
            Style::default().fg(Color::White),
        )),
    ];

    let body = Paragraph::new(lines);
    frame.render_widget(body, inner_area);
}

/// Render the warning popup modal
fn render_warning_popup(frame: &mut Frame, app: &AppState, area: Rect) {
    let Some(popup) = &app.warning_popup else {
        return;
    };

    let modal_width = (area.width * 70 / 100).clamp(40, 90);
    let modal_height = (area.height * 40 / 100).clamp(8, 14);
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(format!(" {} ", popup.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let body = Paragraph::new(popup.message.clone()).style(Style::default().fg(Color::Yellow));
    frame.render_widget(body, inner_area);
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
    use crate::openspec::Change;
    use crate::tui::events::LogEntry;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;
    use std::collections::HashSet;

    fn create_test_change(id: &str, is_approved: bool) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 3,
            last_modified: "now".to_string(),
            is_approved,
            dependencies: Vec::new(),
        }
    }

    fn create_test_app(changes: Vec<Change>) -> AppState {
        let mut app = AppState::new(changes);
        app.logs.clear();
        app.parallel_available = false;
        app.parallel_mode = false;
        app.web_url = None;
        app
    }

    fn render_buffer(app: &mut AppState, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal init");
        terminal.draw(|frame| render(frame, app)).expect("draw");
        terminal.backend().buffer().clone()
    }

    fn buffer_to_string(buffer: &Buffer) -> String {
        let mut lines = Vec::new();
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            lines.push(line);
        }
        lines.join("\n")
    }

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
        // Applying state should show green when selected
        let (text, color) = get_checkbox_display(&QueueStatus::Applying, true, true);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);

        // Archiving state should show green when selected
        let (text, color) = get_checkbox_display(&QueueStatus::Archiving, true, true);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_render_shows_small_terminal_warning() {
        let mut app = create_test_app(Vec::new());
        let buffer = render_buffer(&mut app, 50, 10);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Terminal too small. Minimum: 60x15"));
    }

    #[test]
    fn test_render_shows_worktree_badge() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.changes[0].has_worktree = true;

        let buffer = render_buffer(&mut app, 80, 20);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("WT"));
    }

    #[test]
    fn test_render_resolving_status_shows_label() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.add_log(LogEntry::info("log"));

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("resolving"));
    }

    #[test]
    fn test_render_merge_wait_status_shows_label() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.changes[0].queue_status = QueueStatus::MergeWait;
        app.add_log(LogEntry::info("log"));

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("merge wait"));
    }

    #[test]
    fn test_render_merge_wait_shows_resolve_key_hint() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.changes[0].queue_status = QueueStatus::MergeWait;
        app.is_resolving = false; // Not currently resolving

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("M: resolve"),
            "Should show M key hint for MergeWait status"
        );
    }

    #[test]
    fn test_render_merge_wait_hides_resolve_key_hint_when_resolving() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.changes[0].queue_status = QueueStatus::MergeWait;
        app.is_resolving = true; // Currently resolving

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            !content.contains("M: resolve"),
            "Should NOT show M key hint when resolve is in progress"
        );
    }

    // === Tests for update-tui-error-mode-continuation ===

    #[test]
    fn test_render_uses_centralized_resolve_check_in_select_mode() {
        // Verify that render shows M: resolve in Select mode with MergeWait
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.mode = AppMode::Select;
        app.changes[0].queue_status = QueueStatus::MergeWait;
        app.is_resolving = false;
        app.cursor_index = 0;

        // Render should show M: resolve
        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("M: resolve"),
            "Should show M: resolve in Select mode with MergeWait"
        );
    }

    #[test]
    fn test_render_hides_resolve_in_error_mode() {
        // Verify that render does NOT show M: resolve in Error mode
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.mode = AppMode::Error; // Error mode
        app.changes[0].queue_status = QueueStatus::MergeWait;
        app.is_resolving = false;
        app.cursor_index = 0;
        app.add_log(LogEntry::info("log")); // Add log to show render_running_mode

        // Render should NOT show M: resolve
        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            !content.contains("M: resolve"),
            "Should NOT show M: resolve in Error mode"
        );
    }

    #[test]
    fn test_render_shows_resolve_in_running_mode() {
        // Verify that render shows M: resolve in Running mode for MergeWait
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.mode = AppMode::Running;
        app.changes[0].queue_status = QueueStatus::MergeWait;
        app.is_resolving = false;
        app.cursor_index = 0;
        app.add_log(LogEntry::info("log")); // Add log to trigger render_running_mode

        // Render should show M: resolve
        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("M: resolve"),
            "Should show M: resolve in Running mode when available"
        );
    }

    #[test]
    fn test_render_consistency_with_resolve_availability() {
        // Test that M key hint is shown correctly based on resolve state
        // - When resolve is NOT running and queue_status is MergeWait: "M: resolve"
        // - When resolve IS running and queue_status is MergeWait: "M: queue resolve"
        let test_cases = vec![
            // (mode, queue_status, is_resolving, should_show_resolve, should_show_queue_resolve)
            (AppMode::Select, QueueStatus::MergeWait, false, true, false),
            (AppMode::Select, QueueStatus::MergeWait, true, false, true),
            (AppMode::Running, QueueStatus::MergeWait, false, true, false),
            (AppMode::Running, QueueStatus::MergeWait, true, false, true),
            (AppMode::Error, QueueStatus::MergeWait, false, false, false),
            (AppMode::Select, QueueStatus::Queued, false, false, false),
        ];

        for (mode, queue_status, is_resolving, should_show_resolve, should_show_queue_resolve) in
            test_cases
        {
            let mut app = create_test_app(vec![create_test_change("change-a", true)]);
            app.mode = mode.clone();
            app.changes[0].queue_status = queue_status.clone();
            app.is_resolving = is_resolving;
            app.cursor_index = 0;
            if mode != AppMode::Select {
                app.add_log(LogEntry::info("log")); // Ensure logs exist for running mode
            }

            let buffer = render_buffer(&mut app, 100, 24);
            let content = buffer_to_string(&buffer);
            let shows_resolve = content.contains("M: resolve");
            let shows_queue_resolve = content.contains("M: queue resolve");

            assert_eq!(
                shows_resolve, should_show_resolve,
                "Render 'M: resolve' hint mismatch for mode={:?}, queue_status={:?}, is_resolving={}",
                mode, queue_status, is_resolving
            );
            assert_eq!(
                shows_queue_resolve, should_show_queue_resolve,
                "Render 'M: queue resolve' hint mismatch for mode={:?}, queue_status={:?}, is_resolving={}",
                mode, queue_status, is_resolving
            );
        }
    }

    #[test]
    fn test_render_shows_worktree_delete_confirm_modal() {
        use crate::tui::types::WorktreeAction;

        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.pending_worktree_action =
            Some(("/path/to/worktree".to_string(), WorktreeAction::Delete));
        app.mode = AppMode::ConfirmWorktreeDelete;

        let buffer = render_buffer(&mut app, 80, 20);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Delete Worktree"));
        assert!(content.contains("/path/to/worktree"));
    }

    #[test]
    fn test_render_parallel_archived_row_does_not_show_uncommited_badge() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.parallel_mode = true;
        app.changes[0].queue_status = QueueStatus::Archived;
        app.changes[0].is_parallel_eligible = false;

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(!content.contains("UNCOMMITED"));
        assert!(content.contains("[x]"));
    }

    #[test]
    fn test_render_parallel_uncommitted_queueable_row_shows_uncommited_badge() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.parallel_mode = true;
        app.changes[0].queue_status = QueueStatus::NotQueued;
        app.changes[0].is_parallel_eligible = false;

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("UNCOMMITED"));
    }

    #[test]
    fn test_render_select_mode_footer_message() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Conflux"));
        assert!(content.contains("Press F5 to start processing"));
    }

    #[test]
    fn test_render_shows_uncommitted_badge() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);
        app.parallel_available = true;
        app.parallel_mode = true;
        app.apply_parallel_eligibility(&HashSet::new(), &HashSet::new());

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("UNCOMMITED"));
    }

    #[test]
    fn test_log_header_analysis_with_iteration() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add analysis log with iteration
        let entry = LogEntry::info("Analyzing dependencies")
            .with_operation("analysis")
            .with_iteration(2);
        app.add_log(entry);

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should display [analysis:2] header
        assert!(
            content.contains("[analysis:2]"),
            "Buffer should contain '[analysis:2]' header, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_log_header_analysis_without_iteration() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add analysis log without iteration (edge case - should default to iteration 1)
        let entry = LogEntry::info("Starting analysis").with_operation("analysis");
        app.add_log(entry);

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Per spec: analysis logs must always display with iteration number
        // When iteration is missing, defaults to 1
        assert!(
            content.contains("[analysis:1]"),
            "Buffer should contain '[analysis:1]' header (analysis logs must always show iteration), but got:\n{}",
            content
        );
    }

    #[test]
    fn test_log_header_resolve_with_change_id_and_iteration() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add resolve log with change_id and iteration
        let entry = LogEntry::info("Resolving conflicts")
            .with_change_id("my-change")
            .with_operation("resolve")
            .with_iteration(1);
        app.add_log(entry);

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should display full [my-change:resolve:1] header in Logs view
        assert!(
            content.contains("[my-change:resolve:1]"),
            "Buffer should contain '[my-change:resolve:1]' header, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_log_header_with_change_id_only() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add log with only change_id (no operation or iteration)
        let entry = LogEntry::info("Processing change").with_change_id("test-change");
        app.add_log(entry);

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should display no header (change_id alone is not shown)
        assert!(
            content.contains("Processing change"),
            "Buffer should contain log message"
        );
        // No header should be shown when there's no operation
        assert!(
            !content.contains("[test-change]"),
            "Buffer should not contain header when only change_id is present"
        );
    }

    #[test]
    fn test_log_no_header_when_no_change_id_or_operation() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add plain log with no change_id or operation
        let entry = LogEntry::info("Regular log message");
        app.add_log(entry);

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should display message without header
        assert!(
            content.contains("Regular log message"),
            "Buffer should contain log message"
        );
        // Should not contain bracket headers
        let has_headers = content.contains("[analysis]")
            || content.contains("[resolve]")
            || content.contains("[test-change]");
        assert!(
            !has_headers,
            "Buffer should not contain headers for plain log messages"
        );
    }

    #[test]
    fn test_log_header_acceptance_with_iteration() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add acceptance log with change_id and iteration
        let entry = LogEntry::info("Running acceptance test")
            .with_change_id("my-change")
            .with_operation("acceptance")
            .with_iteration(3);
        app.add_log(entry);

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should display full [my-change:acceptance:3] header in Logs view
        assert!(
            content.contains("[my-change:acceptance:3]"),
            "Buffer should contain '[my-change:acceptance:3]' header, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_log_header_acceptance_without_iteration() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add acceptance log with change_id but no iteration
        let entry = LogEntry::info("Acceptance test starting")
            .with_change_id("my-change")
            .with_operation("acceptance");
        app.add_log(entry);

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should display full [my-change:acceptance] header in Logs view
        assert!(
            content.contains("[my-change:acceptance]"),
            "Buffer should contain '[my-change:acceptance]' header, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_log_header_archive_with_change_id_and_iteration() {
        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add archive log with change_id and iteration
        let entry = LogEntry::info("Archiving change")
            .with_change_id("test-change")
            .with_operation("archive")
            .with_iteration(2);
        app.add_log(entry);

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should display full [test-change:archive:2] header in Logs view
        assert!(
            content.contains("[test-change:archive:2]"),
            "Buffer should contain '[test-change:archive:2]' header for retry identification, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_running_header_counts_only_in_flight_changes() {
        // Test that Running header only counts in-flight changes (not queued)
        let mut app = create_test_app(vec![
            create_test_change("change-a", true),
            create_test_change("change-b", true),
            create_test_change("change-c", true),
            create_test_change("change-d", true),
        ]);

        // Set mode to Running
        app.mode = AppMode::Running;

        // Set up different statuses:
        // - change-a: Queued (should NOT be counted)
        // - change-b: Applying (should be counted)
        // - change-c: Archiving (should be counted)
        // - change-d: NotQueued (should NOT be counted)
        app.changes[0].queue_status = QueueStatus::Queued;
        app.changes[1].queue_status = QueueStatus::Applying;
        app.changes[2].queue_status = QueueStatus::Archiving;
        app.changes[3].queue_status = QueueStatus::NotQueued;

        // Add a log to trigger running mode display
        app.add_log(LogEntry::info("test"));

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should show "Running 2" (only Applying and Archiving)
        assert!(
            content.contains("[Running 2]"),
            "Header should show 'Running 2' (only in-flight changes), but got:\n{}",
            content
        );

        // Should NOT show "Running 3" or "Running 4"
        assert!(
            !content.contains("[Running 3]") && !content.contains("[Running 4]"),
            "Header should not count Queued changes, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_running_header_counts_resolving_as_in_flight() {
        // Test that Resolving status is counted as in-flight
        let mut app = create_test_app(vec![
            create_test_change("change-a", true),
            create_test_change("change-b", true),
        ]);

        // Set mode to Running
        app.mode = AppMode::Running;

        // Set one change to Resolving, one to Queued
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.changes[1].queue_status = QueueStatus::Queued;

        // Add a log to trigger running mode display
        app.add_log(LogEntry::info("test"));

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        // Should show "Running 1" (only Resolving)
        assert!(
            content.contains("[Running 1]"),
            "Header should show 'Running 1' (Resolving is in-flight), but got:\n{}",
            content
        );
    }

    #[test]
    fn test_select_mode_shows_running_when_resolving() {
        let mut app = create_test_app(vec![
            create_test_change("change-a", true),
            create_test_change("change-b", true),
        ]);

        app.mode = AppMode::Select;
        app.changes[0].queue_status = QueueStatus::Resolving;
        app.changes[1].queue_status = QueueStatus::Queued;

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("[Running 1]"),
            "Header should show 'Running 1' in Select mode when resolving, but got:\n{}",
            content
        );
        assert!(
            !content.contains("[Ready]"),
            "Header should not show '[Ready]' while resolving, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_japanese_log_preview_truncation_no_panic() {
        // Test that log preview with Japanese characters doesn't panic
        // when truncated at character boundaries
        use super::super::utils::truncate_to_display_width_with_suffix;

        // Test the truncation function directly with Japanese text
        let japanese_text = "日本語のログメッセージです。これは長いメッセージで切り詰められます。";

        // This should not panic even with multi-byte UTF-8 characters
        let truncated = truncate_to_display_width_with_suffix(japanese_text, 20, "…");

        // Verify result contains ellipsis (was truncated) and doesn't panic
        assert!(
            truncated.contains("…"),
            "Should be truncated with ellipsis, got: {}",
            truncated
        );

        // Verify the truncated string is valid UTF-8 and can be used safely
        assert_eq!(
            truncated.chars().count(),
            truncated.chars().count(), // This would panic if UTF-8 is broken
            "Truncated string should be valid UTF-8"
        );

        // Test with various widths to ensure no panic at character boundaries
        for width in 1..50 {
            let result = truncate_to_display_width_with_suffix(japanese_text, width, "…");
            assert!(
                !result.is_empty(),
                "Should never return empty string for width {}",
                width
            );
        }
    }

    // === Tests for fix-tui-logs-wrap ===

    #[test]
    fn test_logs_wrap_indents_continuation_lines() {
        // Test that wrapped log lines maintain proper indentation
        // First line starts at column 0 (after timestamp+header)
        // Continuation lines are indented by prefix_width

        let message = "This is a very long message that will definitely wrap across multiple lines when rendered in the logs view with a narrow width";
        let available_width = 40;
        let header_width = 0; // No header for this test
        let prefix_width = 15; // e.g., "HH:MM:SS [op] " length

        let wrapped = wrap_log_message(message, available_width, header_width, prefix_width);

        // Should have multiple lines
        assert!(wrapped.len() > 1, "Message should wrap to multiple lines");

        // First line should NOT have indentation (starts at column 0)
        assert!(
            !wrapped[0].starts_with(' '),
            "First line should not start with spaces, got: '{}'",
            wrapped[0]
        );

        // Second and subsequent lines should have indentation
        for (idx, line) in wrapped.iter().skip(1).enumerate() {
            let expected_indent = " ".repeat(prefix_width);
            assert!(
                line.starts_with(&expected_indent),
                "Continuation line {} should start with {} spaces, got: '{}'",
                idx + 2,
                prefix_width,
                line
            );
        }
    }

    #[test]
    fn test_logs_visible_range_not_broken_by_wrapped_entry() {
        // Test that visible range calculation works correctly with wrapped logs
        // When logs wrap to multiple display lines, the visible range should
        // show the correct portion based on display lines, not log count

        let mut app = create_test_app(vec![create_test_change("change-a", true)]);

        // Add a short log
        app.add_log(LogEntry::info("Short log 1"));

        // Add a very long log that will wrap (simulate 200+ char message)
        let long_message = "A".repeat(200);
        app.add_log(LogEntry::info(&long_message).with_operation("apply"));

        // Add another short log
        app.add_log(LogEntry::info("Short log 3"));

        // Render with sufficient size (meet minimum 60x15 requirement)
        // Use height=30 to give enough space for logs panel
        let buffer = render_buffer(&mut app, 80, 30);
        let content = buffer_to_string(&buffer);

        // Verify that the latest log (Short log 3) is visible
        // The bug would cause this to be scrolled off-screen due to incorrect range calculation
        assert!(
            content.contains("Short log 3"),
            "Latest log should be visible in the rendered output, but got:\n{}",
            content
        );

        // Verify that at least one continuation line from the long log is visible
        // This confirms that wrapping is working
        let a_count = content.matches('A').count();
        assert!(
            a_count > 0,
            "Wrapped log should have continuation lines visible, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_wrap_log_message_handles_empty_message() {
        let wrapped = wrap_log_message("", 40, 0, 10);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "");
    }

    #[test]
    fn test_wrap_log_message_handles_zero_width() {
        let wrapped = wrap_log_message("test message", 0, 0, 10);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "test message");
    }

    #[test]
    fn test_wrap_log_message_no_wrap_needed() {
        let message = "Short message";
        let wrapped = wrap_log_message(message, 40, 0, 10);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], message);
    }

    #[test]
    fn test_wrap_log_message_unicode_boundaries() {
        // Test with multi-byte UTF-8 characters (Japanese)
        let message = "日本語のログメッセージです。これは長いメッセージで折り返されます。";
        let wrapped = wrap_log_message(message, 30, 0, 10);

        // Should wrap without panic
        assert!(wrapped.len() > 1);

        // All lines should be valid UTF-8
        for line in &wrapped {
            assert!(line.is_char_boundary(0));
            assert!(line.is_char_boundary(line.len()));
        }

        // Continuation lines should have indentation
        for line in wrapped.iter().skip(1) {
            assert!(
                line.starts_with("          "), // 10 spaces
                "Continuation line should be indented, got: '{}'",
                line
            );
        }
    }
}
