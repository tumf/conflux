//! Rendering functions for the TUI
//!
//! Contains all render_* functions for drawing the UI.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::time::Duration;

use crate::orchestration::operator_command::{is_active_status, is_markable_status};

use super::state::{
    guards, log_logic, AppState, ChangeState, CopyFeedback, ErrorDetailsPopup,
    ERROR_DETAILS_UNAVAILABLE,
};
use super::types::{AppExecutionMode, ModalState};
use super::utils::{get_version_string, truncate_to_display_width_with_suffix};

/// Parsed parts of a remote change ID.
///
/// Remote server mode encodes the change ID as `<project_id>::<project_name>/<change_id>`.
/// This struct holds the project label (the human-friendly portion before the last `/`) and the
/// bare change id (everything after the last `/`).
#[derive(Debug)]
struct RemoteChangeId<'a> {
    /// Human-friendly project label (e.g. `myproject`).  `None` for local changes.
    project: Option<&'a str>,
    /// Bare change id without the project prefix (e.g. `add-feature`).
    change: &'a str,
}

/// Split a raw change `id` field into its project and change components.
///
/// - Local change (no `::`):  `RemoteChangeId { project: None, change: id }`
/// - Remote change (`<pid>::<pname>/<cid>`): `RemoteChangeId { project: Some(pname), change: cid }`
/// - Remote change without `/` after `::` is treated as local-like.
fn split_remote_change_id(id: &str) -> RemoteChangeId<'_> {
    if let Some((_, after_colon)) = id.split_once("::") {
        if let Some((project, change)) = after_colon.rsplit_once('/') {
            return RemoteChangeId {
                project: Some(project),
                change,
            };
        }
        // No slash after "::" – use the whole `after_colon` part as the change id.
        RemoteChangeId {
            project: None,
            change: after_colon,
        }
    } else {
        RemoteChangeId {
            project: None,
            change: id,
        }
    }
}

/// A single visual row in the Changes list.
#[derive(Debug)]
enum ChangeRow {
    /// A non-selectable project header row.
    Header(String),
    /// A selectable change row.  `change_index` is the index into `app.changes`.
    Item { change_index: usize },
}

/// Build the ordered list of visual rows for the Changes panel.
///
/// Groups changes by project (for remote-mode IDs) and inserts a header row
/// before the first change of each project.  Local changes (no project prefix)
/// are also grouped together under a single `(local)` header when the list is
/// otherwise mixed, or shown directly without any header when *all* changes are
/// local.
///
/// Returns:
/// - `rows`: the ordered visual rows
/// - `change_to_visual`: maps `change_index → visual_index` for `ListState::select`
fn build_change_rows(changes: &[ChangeState]) -> (Vec<ChangeRow>, Vec<usize>) {
    // Collect unique project names in stable order.
    let mut seen_projects: Vec<Option<String>> = Vec::new();
    for change in changes {
        let parsed = split_remote_change_id(&change.id);
        let key = parsed.project.map(|p| p.to_string());
        if !seen_projects.contains(&key) {
            seen_projects.push(key);
        }
    }

    // If all changes are local (no project prefix) we skip the header row entirely.
    let all_local = seen_projects.len() == 1 && seen_projects[0].is_none();

    let mut rows: Vec<ChangeRow> = Vec::new();
    let mut change_to_visual: Vec<usize> = vec![0; changes.len()];

    if all_local {
        // No headers needed – one row per change.
        for (ci, _) in changes.iter().enumerate() {
            change_to_visual[ci] = rows.len();
            rows.push(ChangeRow::Item { change_index: ci });
        }
    } else {
        // Insert a project header before the first change of each project group.
        for project_key in &seen_projects {
            let header_label = match project_key {
                Some(p) => p.clone(),
                None => "(local)".to_string(),
            };
            rows.push(ChangeRow::Header(header_label));

            for (ci, change) in changes.iter().enumerate() {
                let parsed = split_remote_change_id(&change.id);
                let key = parsed.project.map(|p| p.to_string());
                if key == *project_key {
                    change_to_visual[ci] = rows.len();
                    rows.push(ChangeRow::Item { change_index: ci });
                }
            }
        }
    }

    (rows, change_to_visual)
}

/// The blank that stands in for a checkbox on a post-archive row.
///
/// Exactly [`CHECKBOX_WIDTH`] columns, so substituting it keeps the
/// change ID, badges, status, progress, and preview in the same columns as
/// every non-terminal row.
const CHECKBOX_PLACEHOLDER: &str = "   ";

/// Display width of the checkbox column, shared by `[x]`, `[ ]`, and the blank.
const CHECKBOX_WIDTH: usize = CHECKBOX_PLACEHOLDER.len();

/// The one column between the checkbox area and the change ID.
///
/// It is the whole focus-independent separator: the cursor glyph that used to
/// live here is gone, and Ratatui's row highlight is the only focus indicator.
const CHECKBOX_TO_ID_SEPARATOR_WIDTH: usize = 1;

/// Terminal display columns of change-ID *content* in a Changes row.
///
/// The full ID field is [`CHANGE_ID_FIELD_WIDTH`]; this is the part the ID itself
/// may occupy, hard-truncated or space-padded to exactly this width so every
/// following field starts in the same column on every row.
const CHANGE_ID_CONTENT_WIDTH: usize = 35;

/// Terminal display columns from the change-ID start to the next field's start.
///
/// [`CHANGE_ID_CONTENT_WIDTH`] columns of ID content plus one separator column,
/// which is supplied by the *next* field's own leading space (`" WT"`,
/// `" [status]"`, …) rather than by the ID span. Nothing but this constant may
/// describe that boundary, so the two row layouts cannot drift apart.
#[cfg_attr(not(test), allow(dead_code))]
const CHANGE_ID_FIELD_WIDTH: usize = CHANGE_ID_CONTENT_WIDTH + CHECKBOX_TO_ID_SEPARATOR_WIDTH;

/// Fixed display columns a Changes row spends before its first badge.
///
/// Checkbox, the single separator column, and the ID content field. Fixed by
/// construction, which is why the preview-width calculation can use it instead of
/// measuring formatted strings with `str::len` — the byte length of a row prefix
/// is not its column count.
const CHANGE_ROW_PREFIX_WIDTH: usize =
    CHECKBOX_WIDTH + CHECKBOX_TO_ID_SEPARATOR_WIDTH + CHANGE_ID_CONTENT_WIDTH;

/// Terminal display columns a rendered fragment occupies.
///
/// Row layout is column accounting, and `str::len` is byte accounting: the two
/// agree only for ASCII, and the spinner glyph and CJK change IDs are exactly
/// where they stop agreeing.
fn display_width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

/// The single character a middle-elided string spends on its gap.
const MIDDLE_ELLIPSIS: char = '…';

/// Middle-elide `text` to at most `max_width` terminal columns.
///
/// A project path is most distinguishing at both ends — the account or checkout
/// root at the front, the repository name at the back — so a trailing-ellipsis
/// truncation would throw away exactly the part that tells two Conflux instances
/// apart. This is the conventional `ElideMiddle` behavior: reserve one column for
/// `…`, split what is left between a retained prefix and suffix, and give the
/// suffix the spare column when that remainder is odd.
///
/// Accounting is in display columns, not bytes or scalars: a CJK path component
/// is half as many characters as it is columns, and a combining mark is none.
/// Both sides are taken whole characters at a time, so the result is always
/// valid UTF-8 and never wider than `max_width`. A budget no wider than the
/// ellipsis itself yields the ellipsis alone (or nothing at zero columns) rather
/// than a panic.
fn middle_elide_to_display_width(text: &str, max_width: usize) -> String {
    if display_width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }

    // One column for the gap; the rest is split, suffix-favoring.
    let retained = max_width - 1;
    let prefix_budget = retained / 2;
    let suffix_budget = retained - prefix_budget;

    let mut prefix = String::new();
    let mut prefix_used = 0;
    for ch in text.chars() {
        let ch_width = char_display_width(ch);
        if prefix_used + ch_width > prefix_budget {
            break;
        }
        prefix.push(ch);
        prefix_used += ch_width;
    }

    // Taken in reverse and flipped back, so a combining mark is admitted before
    // the base character it belongs to and stays attached to it.
    let mut suffix_rev = String::new();
    let mut suffix_used = 0;
    for ch in text.chars().rev() {
        let ch_width = char_display_width(ch);
        if suffix_used + ch_width > suffix_budget {
            break;
        }
        suffix_rev.push(ch);
        suffix_used += ch_width;
    }
    let mut suffix: String = suffix_rev.chars().rev().collect();

    // A side that retained no visible column retained nothing worth showing:
    // keeping a bare combining mark there would only reattach it to the `…`.
    if prefix_used == 0 {
        prefix.clear();
    }
    if suffix_used == 0 {
        suffix.clear();
    }

    let mut out = prefix;
    out.push(MIDDLE_ELLIPSIS);
    out.push_str(&suffix);
    out
}

/// Terminal display columns one character occupies.
///
/// Control characters and combining marks report `None`/zero and are carried
/// along for free, which is what keeps a mark with its base character.
fn char_display_width(ch: char) -> usize {
    unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// True when the row's *display status* is a post-archive presentation.
///
/// `archived`, `merged`, and `pushed` are all post-archive presentations of the
/// same fact, and none of them can carry next-run intent. This is the status-only
/// fallback: it holds on a startup or refresh frame where no reducer
/// archive-completion snapshot has been observed yet.
fn is_post_archive_status(display_status: &str) -> bool {
    matches!(display_status, "archived" | "merged" | "pushed")
}

/// True when the row must render no checkbox text at all.
///
/// Either the reducer recorded archive completion — which is what keeps the
/// checkbox hidden while the row's display status is a live `resolving`,
/// `resolve pending`, or `merge wait` — or the display status is itself a final
/// post-archive one. One predicate for both, so the rendered checkbox and the
/// mark admission in [`guards::is_mark_target`] cannot disagree about a row.
fn hides_checkbox(change: &ChangeState) -> bool {
    change.archive_complete_cache || is_post_archive_status(&change.display_status_cache)
}

/// Determine checkbox display and color for a change item
///
/// Returns (checkbox_text, checkbox_color) based on the change's status.
/// A post-archive row renders neither `[x]` nor `[ ]`: it is not an execution
/// candidate, so a checkbox there would advertise intent it cannot hold. The
/// blank keeps the column width so nothing after it shifts left.
fn get_checkbox_display(
    display_status: &str,
    is_selected: bool,
    archive_complete: bool,
) -> (&'static str, Color) {
    if archive_complete || is_post_archive_status(display_status) {
        (CHECKBOX_PLACEHOLDER, Color::DarkGray) // Post-archive - no checkbox
    } else if is_selected {
        ("[x]", Color::Green) // Selected/In queue
    } else {
        ("[ ]", Color::Gray) // Not selected
    }
}

/// The change-ID field content: exactly [`CHANGE_ID_CONTENT_WIDTH`] columns.
///
/// Hard truncation with no ellipsis, measured in terminal display columns rather
/// than bytes or scalars: a CJK ID is half as many characters as it is columns, so
/// a byte- or char-padded field would push `WT`, the spinner, elapsed time,
/// status, and task progress out of alignment on exactly the rows that already
/// look widest.
///
/// A wide character that would straddle the boundary is dropped whole and its
/// column is filled with a space, because half a character cannot be rendered.
fn render_change_id_field(display_id: &str) -> String {
    use unicode_width::UnicodeWidthChar;

    let mut field = String::new();
    let mut width = 0;
    for ch in display_id.chars() {
        let char_width = ch.width().unwrap_or(0);
        if width + char_width > CHANGE_ID_CONTENT_WIDTH {
            break;
        }
        field.push(ch);
        width += char_width;
    }
    // No suffix: an ellipsis would consume a content column and make the visible
    // prefix depend on whether truncation happened.
    field.push_str(&" ".repeat(CHANGE_ID_CONTENT_WIDTH - width));
    field
}

/// Push the cursor row's kill and execution-mark hints.
///
/// Shared by the Select and Running/Stopped layouts so the two can never
/// describe the same row differently. Two independent controls, so two
/// independent hints:
///
/// * `K: kill` is per-change termination and belongs only to an active row;
/// * `Space: mark` / `Space: unmark` is next-run intent and belongs to every
///   visible non-terminal row, whatever its execution mode, activity, wait
///   state, Apply-limit evidence, or current worktree eligibility.
///
/// A terminal row, and a row the reducer has recorded as archive-complete, gets
/// no mark hint at all: Space on it is a silent no-op. `K: kill` is unaffected —
/// an archive-complete row that is still executing post-archive work remains
/// killable.
fn push_change_row_hints(keys: &mut Vec<String>, app: &AppState, item: &ChangeState) {
    let status = item.display_status_cache.as_str();
    if matches!(
        status,
        "preparing" | "applying" | "accepting" | "archiving" | "resolving"
    ) {
        if let Some(ModalState::ConfirmForceKill { .. }) = app.modal {
            keys.push("Y: confirm kill".to_string());
            keys.push("N: cancel".to_string());
        } else {
            keys.push("K: kill".to_string());
        }
    }

    if is_markable_status(status, item.archive_complete_cache) {
        keys.push(if item.selected {
            "Space: unmark".to_string()
        } else {
            "Space: mark".to_string()
        });
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
            worktree_view::render(frame, app, area);
        }
    }

    // Overlays render from the modal axis, after the base screen. There is no
    // fallback that rewrites an unsupported execution/modal combination: the base
    // above already painted whatever execution state is current.
    match &app.modal {
        Some(ModalState::QrPopup) => popups::render_qr(frame, app, area),
        Some(ModalState::ConfirmWorktreeDelete { .. }) => {
            worktree_view::render_delete_confirm(frame, app, area)
        }
        Some(ModalState::ConfirmDirtyDiscard { .. }) => {
            worktree_view::render_dirty_discard_confirm(frame, app, area)
        }
        Some(ModalState::ConfirmAheadDiscard { .. }) => {
            worktree_view::render_ahead_discard_confirm(frame, app, area)
        }
        // Force-kill confirmation keeps its existing in-list presentation (the
        // `Y: confirm kill` / `N: cancel` hints and the header label); it has no
        // separate popup widget, and this change does not add one.
        Some(ModalState::ConfirmForceKill { .. }) | None => {}
    }

    // The Error Details popup sits above the interaction modals and below the
    // warning popup, matching the order in which the two claim input.
    if app.error_details_popup.is_some() {
        popups::render_error_details(frame, app, area);
    }

    // Render warning popup on top if present
    if app.warning_popup.is_some() {
        popups::render_warning(frame, app, area);
    }
}

mod screens {
    use super::*;

    pub(super) fn render_select_mode(frame: &mut Frame, app: &mut AppState, area: Rect) {
        let chunks = Layout::vertical([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Changes list
            Constraint::Length(3), // Footer
        ])
        .split(area);

        super::render_header(frame, app, chunks[0]);
        super::changes_list::render_select(frame, app, chunks[1]);
        super::render_footer_select(frame, app, chunks[2]);
    }

    pub(super) fn render_running_mode(frame: &mut Frame, app: &mut AppState, area: Rect) {
        let chunks = if app.logs_panel_enabled {
            let (changes_height, logs_height) = super::running_logs_enabled_layout_heights(
                area.height,
                super::running_changes_visual_row_count(app),
            );
            Layout::vertical([
                Constraint::Length(super::TUI_HEADER_HEIGHT), // Header
                Constraint::Length(changes_height),           // Changes list
                Constraint::Length(super::TUI_STATUS_HEIGHT), // Status
                Constraint::Length(logs_height),              // Logs
            ])
            .split(area)
        } else {
            Layout::vertical([
                Constraint::Length(super::TUI_HEADER_HEIGHT),
                Constraint::Min(super::RUNNING_CHANGES_MIN_HEIGHT),
                Constraint::Length(super::TUI_STATUS_HEIGHT),
            ])
            .split(area)
        };

        super::render_header(frame, app, chunks[0]);
        super::changes_list::render_running(frame, app, chunks[1]);
        super::status_logs::render_status(frame, app, chunks[2]);

        if app.logs_panel_enabled && chunks.len() > 3 {
            super::status_logs::render_logs(frame, app, chunks[3]);
        }
    }

    pub(super) fn render_worktree_view(frame: &mut Frame, app: &mut AppState, area: Rect) {
        let chunks = Layout::vertical([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Worktree list
            Constraint::Length(3), // Footer
        ])
        .split(area);

        super::render_header(frame, app, chunks[0]);
        super::worktree_view::render_list(frame, app, chunks[1]);
        super::worktree_view::render_footer(frame, app, chunks[2]);
    }
}

fn render_select_mode(frame: &mut Frame, app: &mut AppState, area: Rect) {
    screens::render_select_mode(frame, app, area);
}

const TUI_HEADER_HEIGHT: u16 = 3;
const TUI_STATUS_HEIGHT: u16 = 3;
const RUNNING_CHANGES_MIN_HEIGHT: u16 = 5;
const RUNNING_LOGS_TARGET_HEIGHT: u16 = 20;
const PANEL_BORDER_HEIGHT: u16 = 2;

fn running_changes_visual_row_count(app: &AppState) -> u16 {
    let (rows, _) = build_change_rows(&app.changes);
    rows.len().min(u16::MAX as usize) as u16
}

fn running_logs_enabled_layout_heights(area_height: u16, changes_visual_rows: u16) -> (u16, u16) {
    let available = area_height.saturating_sub(TUI_HEADER_HEIGHT + TUI_STATUS_HEIGHT);
    let desired_changes_height = changes_visual_rows
        .saturating_add(PANEL_BORDER_HEIGHT)
        .max(RUNNING_CHANGES_MIN_HEIGHT);

    if available <= RUNNING_CHANGES_MIN_HEIGHT {
        return (available, 0);
    }

    if available <= RUNNING_CHANGES_MIN_HEIGHT + RUNNING_LOGS_TARGET_HEIGHT {
        let changes_height = RUNNING_CHANGES_MIN_HEIGHT.min(available);
        return (changes_height, available.saturating_sub(changes_height));
    }

    let max_changes_with_target_logs = available.saturating_sub(RUNNING_LOGS_TARGET_HEIGHT);
    let changes_height = desired_changes_height.min(max_changes_with_target_logs);
    let logs_height = available.saturating_sub(changes_height);
    (changes_height, logs_height)
}

fn render_running_mode(frame: &mut Frame, app: &mut AppState, area: Rect) {
    screens::render_running_mode(frame, app, area);
}

mod changes_list {
    use super::*;

    pub(super) fn render_select(frame: &mut Frame, app: &mut AppState, area: Rect) {
        super::render_changes_list_select(frame, app, area);
    }

    pub(super) fn render_running(frame: &mut Frame, app: &mut AppState, area: Rect) {
        super::render_changes_list_running(frame, app, area);
    }
}

mod status_logs {
    use super::*;

    pub(super) fn render_status(frame: &mut Frame, app: &AppState, area: Rect) {
        super::render_status(frame, app, area);
    }

    pub(super) fn render_logs(frame: &mut Frame, app: &mut AppState, area: Rect) {
        super::render_logs(frame, app, area);
    }
}

mod worktree_view {
    use super::*;

    pub(super) fn render(frame: &mut Frame, app: &mut AppState, area: Rect) {
        super::screens::render_worktree_view(frame, app, area);
    }

    pub(super) fn render_list(frame: &mut Frame, app: &mut AppState, area: Rect) {
        super::render_worktree_list(frame, app, area);
    }

    pub(super) fn render_footer(frame: &mut Frame, app: &AppState, area: Rect) {
        super::render_footer_worktree(frame, app, area);
    }

    pub(super) fn render_delete_confirm(frame: &mut Frame, app: &AppState, area: Rect) {
        super::render_worktree_delete_confirm(frame, app, area);
    }

    pub(super) fn render_dirty_discard_confirm(frame: &mut Frame, app: &AppState, area: Rect) {
        super::render_worktree_dirty_discard_confirm(frame, app, area);
    }

    pub(super) fn render_ahead_discard_confirm(frame: &mut Frame, app: &AppState, area: Rect) {
        super::render_worktree_ahead_discard_confirm(frame, app, area);
    }
}

mod popups {
    use super::*;

    pub(super) fn render_warning(frame: &mut Frame, app: &AppState, area: Rect) {
        super::render_warning_popup(frame, app, area);
    }

    pub(super) fn render_qr(frame: &mut Frame, app: &AppState, area: Rect) {
        super::render_qr_popup(frame, app, area);
    }

    pub(super) fn render_error_details(frame: &mut Frame, app: &AppState, area: Rect) {
        super::render_error_details_popup(frame, app, area);
    }
}

/// Render header
fn render_header(frame: &mut Frame, app: &AppState, area: Rect) {
    let active_count = app
        .changes
        .iter()
        .filter(|c| {
            matches!(
                c.display_status_cache.as_str(),
                "preparing" | "applying" | "accepting" | "archiving" | "resolving"
            )
        })
        .count();

    // Per spec (show-ready-header-after-stop):
    // - Select mode: Ready
    // - Running mode: Running / Running <count>
    // - Stopping mode: Stopping
    // - Stopped mode: Ready
    // - Error mode: no status label
    // The header reports current orchestration activity, not internal control
    // state. `AppExecutionMode::Stopped` is a resume-specific command-admission
    // mode, not a running condition, so it projects to the same Ready label as
    // Select. Its stop/resume semantics stay in the status panel, which keeps
    // reporting the configured start key as `resume`.
    // An active overlay may relabel the header, but that label is presentation
    // only: it never changes what the execution axis is, and the status panel
    // below still reports the execution mode's own controls.
    let (mode_text, mode_color, show_status) = match &app.modal {
        Some(modal @ ModalState::QrPopup) => (modal.title_label().to_string(), Color::Green, true),
        Some(modal @ ModalState::ConfirmWorktreeDelete { .. }) => {
            (modal.title_label().to_string(), Color::Yellow, true)
        }
        Some(
            modal @ (ModalState::ConfirmDirtyDiscard { .. }
            | ModalState::ConfirmAheadDiscard { .. }),
        ) => (modal.title_label().to_string(), Color::Red, true),
        Some(modal @ ModalState::ConfirmForceKill { .. }) => {
            (modal.title_label().to_string(), Color::Red, true)
        }
        None => match app.execution_mode {
            AppExecutionMode::Select | AppExecutionMode::Stopped => {
                ("Ready".to_string(), Color::Cyan, true)
            }
            AppExecutionMode::Running => {
                if active_count > 0 {
                    (format!("Running {}", active_count), Color::Yellow, true)
                } else {
                    ("Running".to_string(), Color::Yellow, true)
                }
            }
            AppExecutionMode::Stopping => ("Stopping".to_string(), Color::Yellow, true),
            AppExecutionMode::Error => {
                // Hide status in Error mode per spec
                (String::new(), Color::White, false)
            }
        },
    };

    let version = get_version_string();
    let version_width = version.len() as u16 + 2; // +2 for padding

    // Split area into left content and right-aligned version. The split has to
    // happen before the spans are built, because the project path's width budget
    // is whatever the left chunk has left after every other header segment.
    let chunks =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(version_width)]).split(area);

    let status_segment = if show_status && !mode_text.is_empty() {
        format!("  [{}]", mode_text)
    } else {
        String::new()
    };
    let dirty_segment = if app.workspace_dirty().shows_dirty_badge() {
        " [dirty]"
    } else {
        ""
    };

    // Build header spans
    let mut header_spans = vec![Span::styled("Conflux", Style::default().fg(Color::White))];

    // Add status label only when show_status is true
    if !status_segment.is_empty() {
        header_spans.push(Span::raw("  "));
        header_spans.push(Span::styled(
            format!("[{}]", mode_text),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ));
    }

    // The project this TUI owns, captured at startup. It takes the slot the
    // workspace concurrency/backend badge used to hold: an operator with several
    // instances open needs to know which repository is in front of them, and the
    // execution configuration stays readable from `/api/v2` and the status panel.
    //
    // The budget is the left chunk's own interior — its width less the single
    // LEFT border column — after the segments around the path are reserved. That
    // is real column accounting, so a wide-Unicode path cannot silently overrun
    // the version area.
    let project_path = app.project_path().display().to_string();
    if !project_path.is_empty() {
        let left_interior = chunks[0].width.saturating_sub(1) as usize;
        let reserved = display_width("Conflux")
            + display_width(&status_segment)
            + display_width(" ")
            + display_width(dirty_segment);
        let path_budget = left_interior.saturating_sub(reserved);
        if path_budget > 0 {
            header_spans.push(Span::raw(" "));
            header_spans.push(Span::styled(
                middle_elide_to_display_width(&project_path, path_budget),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    // Workspace dirty observation from the existing auto-refresh. Only a known
    // dirty result draws: a clean observation and a not-yet-observed workspace
    // both stay silent, so the badge's absence is never evidence of anything.
    if !dirty_segment.is_empty() {
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled(
            "[dirty]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    let header_text = Line::from(header_spans);

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

/// Minimum remaining row width a preview needs before it is worth showing.
const MIN_PREVIEW_WIDTH: usize = 10;

/// Badge for a change with observed uncommitted or untracked proposal files.
///
/// Shared by both list layouts so the rendered text and the width reserved for
/// it can never disagree.
const UNCOMMITTED_BADGE: &str = " UNCOMMITTED";

/// Preview text for one change row, before width truncation.
///
/// An `error` row explains itself from the retained final diagnostic, which is
/// independent of the bounded log buffer: a change stays `error` long after its
/// failure entry has been evicted, so the row must not depend on a surviving
/// `LogEntry` to name its failure. Every other row keeps the existing
/// latest-log preview, relative time and shortened header included.
///
/// The returned text is presentation only and carries no workflow-control meaning.
fn change_row_preview_text(app: &AppState, change: &ChangeState) -> Option<String> {
    if change.display_status_cache == "error" {
        return Some(match change.error_message_cache.as_deref() {
            Some(detail) => format!(" Error: {}", detail),
            None => format!(" {}", ERROR_DETAILS_UNAVAILABLE),
        });
    }

    let log = app.get_latest_log_for_change(&change.id)?;

    // Format relative time with parentheses
    let relative_time = format!("({})", format_relative_time(&log.created_at));

    // Build shortened header: [operation:iteration] or [operation]
    let header = match (&log.operation, log.iteration) {
        (Some(op), Some(iter)) => format!(" [{}:{}]", op, iter),
        (Some(op), None) => format!(" [{}]", op),
        (None, _) => String::new(),
    };

    Some(if !header.is_empty() {
        format!(" {}{} {}", relative_time, header, log.message)
    } else {
        format!(" {} {}", relative_time, log.message)
    })
}

/// Foreground color for a row preview.
///
/// An error preview stays readable in both row states: `LightRed` against the
/// `DarkGray` highlight of the focused row, `Red` against the ordinary
/// background. Non-error previews keep their existing dim styling.
fn change_row_preview_color(change: &ChangeState, is_focused_row: bool) -> Color {
    if change.display_status_cache == "error" {
        return if is_focused_row {
            Color::LightRed
        } else {
            Color::Red
        };
    }

    if is_focused_row {
        Color::Gray
    } else {
        Color::DarkGray
    }
}

/// Render changes list in selection mode
fn render_changes_list_select(frame: &mut Frame, app: &mut AppState, area: Rect) {
    // Build grouped visual rows (project headers + change rows).
    let (rows, change_to_visual) = build_change_rows(&app.changes);

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            // Non-selectable project header row.
            ChangeRow::Header(label) => {
                let line = Line::from(vec![
                    Span::styled(
                        format!("  {} ", label),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "─".repeat(area.width.saturating_sub(label.len() as u16 + 5) as usize),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            }
            // Selectable change row.
            ChangeRow::Item { change_index: i } => {
                let i = *i;
                let change = &app.changes[i];
                // Checkbox display (Select mode):
                // [ ] - not marked
                // [x] - marked as a next-run target
                // (blank) - post-archive: no longer a run candidate
                // Note: 'selected' field indicates the execution mark
                let is_post_archive = hides_checkbox(change);
                // Every worktree-ineligible reason still refuses a *run*; only
                // the badge narrows to the one reason that is actually a Git
                // working-tree condition. It never refuses a mark.
                let is_parallel_blocked = !change.is_parallel_eligible()
                    && !is_post_archive
                    && matches!(
                        change.display_status_cache.as_str(),
                        "not queued" | "queued"
                    );
                let show_uncommitted_badge =
                    is_parallel_blocked && change.has_uncommitted_proposal_files();
                // Determine if this is the focused/cursor row before computing colors.
                let is_selected_row = i == app.cursor_index;
                // When a blocked row is focused its foreground must remain readable against the
                // DarkGray highlight background. Use Gray (visible) instead of DarkGray (invisible).
                let blocked_fg = if is_selected_row {
                    Color::Gray
                } else {
                    Color::DarkGray
                };
                // A blocked row is dimmed, never falsified: the checkbox still
                // reports the mark the operator actually set, because worktree
                // eligibility no longer gates mark intent.
                let (checkbox, checkbox_color) = {
                    let (text, color) = get_checkbox_display(
                        &change.display_status_cache,
                        change.selected,
                        change.archive_complete_cache,
                    );
                    (
                        text,
                        if is_parallel_blocked {
                            blocked_fg
                        } else {
                            color
                        },
                    )
                };

                let worktree_badge = if change.has_worktree { " WT" } else { "" };
                let worktree_color = if is_parallel_blocked {
                    blocked_fg
                } else {
                    Color::Green
                };
                let new_badge = if change.is_new && change.display_status_cache != "rejected" {
                    " NEW"
                } else {
                    ""
                };
                let uncommitted_badge = if show_uncommitted_badge {
                    UNCOMMITTED_BADGE
                } else {
                    ""
                };

                // Use brighter colors for selected row to ensure visibility on DarkGray background
                let dim_color = if is_parallel_blocked {
                    blocked_fg
                } else if is_selected_row {
                    Color::Gray // Brighter than DarkGray for visibility on selected row
                } else {
                    Color::DarkGray
                };

                let name_color = if is_parallel_blocked {
                    blocked_fg
                } else {
                    Color::White
                };

                // In grouped mode show only the bare change id (no project prefix).
                let parsed = split_remote_change_id(&change.id);
                let display_id = parsed.change;

                let status_text = format!("[{}]", change.status_badge());

                let mut spans = vec![
                    // Checkbox plus the single separator column. Focus is the row
                    // highlight alone, so no cursor glyph and no cursor column.
                    Span::styled(format!("{checkbox} "), Style::default().fg(checkbox_color)),
                    Span::styled(
                        render_change_id_field(display_id),
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
                        format!(" {:>18}", status_text),
                        Style::default().fg(change.display_color_cache),
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

                // Add the row preview if there is anything to preview: the
                // retained final diagnostic for an error row, otherwise the
                // latest buffered log.
                if let Some(preview_text) = change_row_preview_text(app, change) {
                    // The row prefix is fixed by construction, so it is taken from
                    // the shared constant rather than measured: `str::len` on the
                    // formatted prefix reported bytes, which is not its column
                    // count once a non-ASCII glyph or ID is involved.
                    let worktree_badge_width = if change.has_worktree { 3 } else { 0 }; // " WT"
                    let new_badge_width = if change.is_new { 4 } else { 0 }; // " NEW"
                    let uncommitted_badge_width = if show_uncommitted_badge {
                        display_width(UNCOMMITTED_BADGE)
                    } else {
                        0
                    };
                    let status_text = format!("[{}]", change.display_status_cache.as_str());
                    let status_width = display_width(&format!(" {:>18}", status_text));
                    let tasks_text =
                        format!(" {}/{} tasks", change.completed_tasks, change.total_tasks);
                    let tasks_width = display_width(&tasks_text);
                    let percent_text = format!("  {:>5.1}%", change.progress_percent());
                    let percent_width = display_width(&percent_text);
                    let list_border_width = 2; // List widget border

                    let base_width = CHANGE_ROW_PREFIX_WIDTH
                        + worktree_badge_width
                        + new_badge_width
                        + uncommitted_badge_width
                        + status_width
                        + tasks_width
                        + percent_width
                        + list_border_width;

                    let available = (area.width as usize).saturating_sub(base_width);

                    // Only show preview if available width is wide enough
                    if available >= MIN_PREVIEW_WIDTH {
                        // Truncate if necessary (Unicode-safe)
                        let truncated =
                            truncate_to_display_width_with_suffix(&preview_text, available, "…");

                        spans.push(Span::styled(
                            truncated,
                            Style::default().fg(change_row_preview_color(change, is_selected_row)),
                        ));
                    }
                }

                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    // Update list_state to select the visual index corresponding to the current cursor.
    if !app.changes.is_empty() && app.cursor_index < change_to_visual.len() {
        app.list_state
            .select(Some(change_to_visual[app.cursor_index]));
    }

    // Build dynamic key hints based on current state
    let has_selection = !app.changes.is_empty();
    let has_queue = app.changes.iter().any(|c| c.selected);
    let current_item = if has_selection && app.cursor_index < app.changes.len() {
        Some(&app.changes[app.cursor_index])
    } else {
        None
    };
    let start_key_label = app.start_key_label();

    let mut keys = vec!["↑↓/jk: move".to_string()];
    if let Some(item) = current_item {
        push_change_row_hints(&mut keys, app, item);
        keys.push("e: edit".to_string());
        // Advertise the Error Details popup only on a row that can open it.
        if item.display_status_cache == "error" {
            keys.push("Enter: details".to_string());
        }
        // Show M key hint based on resolve state (only in Select, Running, Stopped modes)
        // - When resolve is NOT running and current item is MergeWait: "M: resolve"
        // - When resolve IS running and current item is MergeWait: "M: queue resolve"
        if item.display_status_cache == "merge wait"
            && matches!(
                app.execution_mode,
                AppExecutionMode::Select | AppExecutionMode::Running | AppExecutionMode::Stopped
            )
        {
            if app.is_resolving() {
                keys.push("M: queue resolve".to_string());
            } else {
                keys.push("M: resolve".to_string());
            }
        }
    }
    if has_queue {
        keys.push(format!("{start_key_label}: run"));
    }
    if app.has_bulk_toggle_targets() {
        keys.push("x: toggle all".to_string());
    }
    keys.push("Tab: worktrees".to_string());
    // Show QR code hint if web server is enabled
    if app.web_url.is_some() {
        keys.push("w: QR".to_string());
    }
    // Show log panel toggle hint
    keys.push("l: logs".to_string());

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

    // Build grouped visual rows (project headers + change rows).
    let (rows, change_to_visual) = build_change_rows(&app.changes);

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            // Non-selectable project header row.
            ChangeRow::Header(label) => {
                let line = Line::from(vec![
                    Span::styled(
                        format!("  {} ", label),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "─".repeat(area.width.saturating_sub(label.len() as u16 + 5) as usize),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            }
            // Selectable change row.
            ChangeRow::Item { change_index: i } => {
                let i = *i;
                let change = &app.changes[i];
                // Checkbox display (Running/Stopped mode):
                // [ ] - not marked
                // [x] - marked as a next-run target
                // (blank) - post-archive: no longer a run candidate
                // The checkbox is the execution mark in every mode. Queue
                // membership is a separate axis and has its own status column.
                let is_post_archive = hides_checkbox(change);
                // Every worktree-ineligible reason still refuses a *run*; only
                // the badge narrows to the one reason that is actually a Git
                // working-tree condition. It never refuses a mark.
                let is_parallel_blocked = !change.is_parallel_eligible()
                    && !is_post_archive
                    && matches!(
                        change.display_status_cache.as_str(),
                        "not queued" | "queued"
                    );
                let show_uncommitted_badge =
                    is_parallel_blocked && change.has_uncommitted_proposal_files();
                // Determine if this is the focused/cursor row before computing colors.
                let is_selected_row = i == app.cursor_index;
                // When a blocked row is focused its foreground must remain readable against the
                // DarkGray highlight background. Use Gray (visible) instead of DarkGray (invisible).
                let blocked_fg = if is_selected_row {
                    Color::Gray
                } else {
                    Color::DarkGray
                };
                // A blocked row is dimmed, never falsified: the checkbox still
                // reports the mark the operator actually set, because worktree
                // eligibility no longer gates mark intent.
                let (checkbox, checkbox_color) = {
                    let (text, color) = get_checkbox_display(
                        &change.display_status_cache,
                        change.selected,
                        change.archive_complete_cache,
                    );
                    (
                        text,
                        if is_parallel_blocked {
                            blocked_fg
                        } else {
                            color
                        },
                    )
                };

                let worktree_badge = if change.has_worktree { " WT" } else { "" };
                let worktree_color = if is_parallel_blocked {
                    blocked_fg
                } else {
                    Color::Green
                };
                let new_badge = if change.is_new && change.display_status_cache != "rejected" {
                    " NEW"
                } else {
                    ""
                };
                let uncommitted_badge = if show_uncommitted_badge {
                    UNCOMMITTED_BADGE
                } else {
                    ""
                };

                // Use brighter colors for selected row to ensure visibility on DarkGray background
                let dim_color = if is_parallel_blocked {
                    blocked_fg
                } else if is_selected_row {
                    Color::Gray // Brighter than DarkGray for visibility on selected row
                } else {
                    Color::DarkGray
                };

                let name_color = if is_parallel_blocked {
                    blocked_fg
                } else {
                    Color::White
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
                let (spinner_prefix, status_text) = match change.display_status_cache.as_str() {
                    // Preparation spins like any other active row, but never
                    // carries an iteration: no operation agent has started, so
                    // a retained apply count would describe a different phase.
                    "preparing" => (format!("{} ", spinner_char), "[preparing]".to_string()),
                    "applying" | "archiving" | "resolving" | "accepting" => {
                        let status = if let Some(iter) = change.iteration_number {
                            format!("[{}:{}]", change.display_status_cache.as_str(), iter)
                        } else {
                            format!("[{}]", change.display_status_cache.as_str())
                        };
                        (format!("{} ", spinner_char), status)
                    }
                    // A blocked row carries its blocker kind in the badge so a
                    // dependency wait and an external prerequisite wait stay
                    // distinguishable without a separate lookup.
                    _ => (String::new(), format!("[{}]", change.status_badge())),
                };

                // Pre-calculate widths before moving values into Spans
                let (spinner_elapsed_width, status_only_width) = if !spinner_prefix.is_empty() {
                    let spinner_elapsed_text =
                        format!(" {}{:>7} ", spinner_prefix.trim(), elapsed_text);
                    // Display columns, not bytes: the spinner glyph is multi-byte.
                    (
                        display_width(&spinner_elapsed_text),
                        display_width(&status_text),
                    )
                } else {
                    let status_formatted = format!(" {:>18}", status_text);
                    (0, display_width(&status_formatted))
                };

                // In grouped mode show only the bare change id (no project prefix).
                let parsed = split_remote_change_id(&change.id);
                let display_id = parsed.change;

                let mut spans = vec![
                    // Checkbox plus the single separator column. Focus is the row
                    // highlight alone, so no cursor glyph and no cursor column.
                    Span::styled(format!("{checkbox} "), Style::default().fg(checkbox_color)),
                    Span::styled(
                        render_change_id_field(display_id),
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
                        Style::default().fg(change.display_color_cache),
                    ));
                } else {
                    spans.push(Span::styled(
                        format!(" {:>18}", status_text),
                        Style::default().fg(change.display_color_cache),
                    ));
                }

                // For Applying status, show progress as "completed/total(percent%)"
                // For other statuses, show just "completed/total"
                let tasks_text = if change.display_status_cache == "applying" {
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

                // Add the row preview if there is anything to preview: the
                // retained final diagnostic for an error row, otherwise the
                // latest buffered log.
                if let Some(preview_text) = change_row_preview_text(app, change) {
                    // The row prefix is fixed by construction, so it is taken from
                    // the shared constant rather than measured: `str::len` on the
                    // formatted prefix reported bytes, which is not its column
                    // count once a non-ASCII glyph or ID is involved.
                    let worktree_badge_width = if change.has_worktree { 3 } else { 0 }; // " WT"
                    let new_badge_width = if change.is_new { 4 } else { 0 }; // " NEW"
                    let uncommitted_badge_width = if show_uncommitted_badge {
                        display_width(UNCOMMITTED_BADGE)
                    } else {
                        0
                    };

                    // Use the actual tasks_text that was already formatted above
                    let tasks_width = display_width(&tasks_text);
                    let list_border_width = 2; // List widget border

                    let base_width = CHANGE_ROW_PREFIX_WIDTH
                        + worktree_badge_width
                        + new_badge_width
                        + uncommitted_badge_width
                        + spinner_elapsed_width
                        + status_only_width
                        + tasks_width
                        + list_border_width;

                    let available = (area.width as usize).saturating_sub(base_width);

                    // Only show preview if available width is wide enough
                    if available >= MIN_PREVIEW_WIDTH {
                        // Truncate if necessary (Unicode-safe)
                        let truncated =
                            truncate_to_display_width_with_suffix(&preview_text, available, "…");

                        spans.push(Span::styled(
                            truncated,
                            Style::default().fg(change_row_preview_color(change, is_selected_row)),
                        ));
                    }
                }

                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    // Update list_state to select the visual index corresponding to the current cursor.
    if !app.changes.is_empty() && app.cursor_index < change_to_visual.len() {
        app.list_state
            .select(Some(change_to_visual[app.cursor_index]));
    }

    // Build dynamic key hints based on current state (same logic as select mode)
    let has_selection = !app.changes.is_empty();
    let current_item = if has_selection && app.cursor_index < app.changes.len() {
        Some(&app.changes[app.cursor_index])
    } else {
        None
    };

    let mut keys = vec!["↑↓/jk: move".to_string()];
    if let Some(item) = current_item {
        push_change_row_hints(&mut keys, app, item);
        keys.push("e: edit".to_string());
        // Advertise the Error Details popup only on a row that can open it.
        if item.display_status_cache == "error" {
            keys.push("Enter: details".to_string());
        }
        // Show M key hint based on resolve state (only in Select, Running, Stopped modes)
        // - When resolve is NOT running and current item is MergeWait: "M: resolve"
        // - When resolve IS running and current item is MergeWait: "M: queue resolve"
        if item.display_status_cache == "merge wait"
            && matches!(
                app.execution_mode,
                AppExecutionMode::Select | AppExecutionMode::Running | AppExecutionMode::Stopped
            )
        {
            if app.is_resolving() {
                keys.push("M: queue resolve".to_string());
            } else {
                keys.push("M: resolve".to_string());
            }
        }
    }
    if app.has_bulk_toggle_targets() {
        keys.push("x: toggle all".to_string());
    }
    keys.push("Tab: worktrees".to_string());
    // Show QR code hint if web server is enabled
    if app.web_url.is_some() {
        keys.push("w: QR".to_string());
    }
    // Show log panel toggle hint
    keys.push("l: logs".to_string());

    let new_indicator = if app.new_change_count > 0 {
        format!(" New: {} |", app.new_change_count)
    } else {
        String::new()
    };
    let title = format!(" Changes ({} {}) ", new_indicator, keys.join(", "));

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
/// Display statuses that mean the change reached final success.
///
/// Narrower than the shared final-status vocabulary on purpose: `rejected` is
/// also final, but it is a non-success outcome and never an execution target.
const OVERALL_PROGRESS_SUCCESS_STATUSES: [&str; 3] = ["archived", "merged", "pushed"];

/// Whether a row contributes its stored task counts to the Status aggregate.
///
/// The aggregate target set is the unique union of successful completed work,
/// work currently executing, and unfinished work carrying an execution mark.
/// This is one predicate rather than three passes precisely so an overlapping
/// row — active *and* marked, or completed *and* still marked — is counted once.
///
/// An execution mark is next-run intent, not a record of what this run already
/// covered, so mark presence alone can never be the sole inclusion evidence:
/// archive completion revokes the mark by design, and dropping the row then
/// would make a successful completion *reduce* the displayed progress.
fn contributes_to_overall_progress(change: &ChangeState) -> bool {
    // Rejection is final and unsuccessful, so stale presentation state still
    // claiming a mark must not pull the row back into the aggregate.
    if change.display_status_cache == "rejected" {
        return false;
    }

    // Completed: the reducer-observed archive milestone covers post-archive
    // `resolving` / `resolve pending` / `merge wait`, none of which is terminal.
    change.archive_complete_cache
        || OVERALL_PROGRESS_SUCCESS_STATUSES.contains(&change.display_status_cache.as_str())
        // In progress: the shared vocabulary, not a TUI-local phase list.
        || is_active_status(&change.display_status_cache)
        // Marked for execution: idle, queued, waiting, or retryable error.
        || change.selected
}

fn render_status(frame: &mut Frame, app: &AppState, area: Rect) {
    // Per spec (update-tui-status-display, fix-tui-overall-progress-scope):
    // Status line shows only progress bar + elapsed time. Progress covers the
    // unique union of completed, actively executing, and execution-marked work.
    let (total_tasks, completed_tasks) = app
        .changes
        .iter()
        .filter(|c| contributes_to_overall_progress(c))
        .fold((0u32, 0u32), |(total, completed), c| {
            (total + c.total_tasks, completed + c.completed_tasks)
        });

    let mut spans = vec![];

    // Show progress bar only when the included rows carry tasks at all.
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
        let elapsed = if matches!(
            app.execution_mode,
            AppExecutionMode::Running | AppExecutionMode::Stopping
        ) {
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
    let start_key_label = app.start_key_label();
    // Base controls come from the execution axis alone. An overlay adds its own
    // instructions inside the overlay itself, so it must not erase the retry or
    // resume affordance the execution state underneath still owns.
    let title = match app.execution_mode {
        // Ready backed by a live idle scheduler keeps the stop affordance: the
        // scheduler task still exists and still accepts a graceful stop, so
        // hiding Esc here would make a running process look unstoppable.
        AppExecutionMode::Select if app.persistent_scheduler_idle => {
            format!(" Status ({start_key_label}: start, Esc: stop, Ctrl+C: quit) ")
        }
        AppExecutionMode::Select => " Status (Ctrl+C: quit) ".to_string(),
        AppExecutionMode::Running => " Status (Esc: stop, Ctrl+C: quit) ".to_string(),
        AppExecutionMode::Stopping => {
            format!(" Status ({start_key_label}: continue, Esc: force stop, Ctrl+C: quit) ")
        }
        AppExecutionMode::Stopped => format!(" Status ({start_key_label}: resume, Ctrl+C: quit) "),
        AppExecutionMode::Error => format!(" Status ({start_key_label}: retry, Ctrl+C: quit) "),
    };

    let status = Paragraph::new(content).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(status, area);
}

fn log_navigation_hint() -> &'static str {
    "PgUp/PgDn: older/newer Home/End: oldest/newest l: hide"
}

/// Full selected-proposal filter hint, naming the active target when known.
fn selected_proposal_filter_hint(filter_enabled: bool, filter_target: Option<&str>) -> String {
    match (filter_enabled, filter_target) {
        (false, _) => compact_selected_proposal_filter_hint(false).to_string(),
        (true, Some(target)) => format!("f: filter={}", target),
        (true, None) => compact_selected_proposal_filter_hint(true).to_string(),
    }
}

/// Width-constrained equivalent: keeps the `f` key and the off/on meaning.
fn compact_selected_proposal_filter_hint(filter_enabled: bool) -> &'static str {
    if filter_enabled {
        "f: filter on"
    } else {
        "f: filter off"
    }
}

/// Inputs the Logs panel title is derived from.
///
/// All line/range values are already computed from the filtered, wrapped
/// display-line sequence, so the indicator describes what navigation actually
/// moves through rather than a count of skipped entries.
struct LogsPanelTitle<'a> {
    /// Display lines between the last visible line and the newest line.
    lines_below: usize,
    log_auto_scroll: bool,
    total_display_lines: usize,
    visible_height: usize,
    start_line: usize,
    end_line: usize,
    filter_enabled: bool,
    filter_target: Option<&'a str>,
    panel_width: usize,
}

fn logs_panel_title(params: LogsPanelTitle<'_>) -> String {
    let auto_scroll_indicator = if params.log_auto_scroll { "▼" } else { "⏸" };
    let help = log_navigation_hint();

    let build = |filter_hint: &str| {
        if params.total_display_lines > params.visible_height {
            let visible_start = params.start_line + 1;
            let visible_end = params.end_line;
            format!(
                " Logs [{}-{}/{} lines] lines_below={} {} ({} | {}) ",
                visible_start,
                visible_end,
                params.total_display_lines,
                params.lines_below,
                auto_scroll_indicator,
                filter_hint,
                help
            )
        } else {
            format!(
                " Logs lines_below={} {} ({} | {}) ",
                params.lines_below, auto_scroll_indicator, filter_hint, help
            )
        }
    };

    let title = build(&selected_proposal_filter_hint(
        params.filter_enabled,
        params.filter_target,
    ));

    // Fall back to the compact wording when naming the target would push the
    // title past the panel width; the key and the off/on meaning are preserved.
    if params.panel_width > 0 && title.chars().count() > params.panel_width {
        return build(compact_selected_proposal_filter_hint(params.filter_enabled));
    }

    title
}

/// Render logs panel with display-line scroll support.
///
/// Width ownership stays here: the panel wraps the retained message across as
/// many lines as its current inner width needs, and it records that geometry on
/// `AppState` so navigation moves through exactly the same display-line
/// sequence this function draws.
fn render_logs(frame: &mut Frame, app: &mut AppState, area: Rect) {
    // Publish the geometry navigation must wrap with before reading any lines.
    app.set_log_viewport(area.width as usize, area.height as usize);
    let visible_height = app.log_viewport.visible_height();

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

    /// Per-entry chrome; the wrapped text itself lives in the display-line list.
    struct RenderedLogChrome {
        timestamp: String,
        timestamp_style: Style,
        header: String,
        header_style: Style,
        message_style: Style,
    }

    // Select the visible entry set BEFORE any wrapping, line counting, visible
    // range, or scroll math so every downstream calculation is derived from the
    // filtered set (including the zero-match case). `app.logs` is never mutated.
    let entries = app.visible_log_entries();

    let chrome: Vec<RenderedLogChrome> = entries
        .iter()
        .map(|(_, entry)| {
            let timestamp = format!("{} ", entry.timestamp);
            let header = log_logic::logs_panel_header(entry);

            let header_style = if header.is_empty() {
                Style::default()
            } else {
                // Use hash of change_id (if present) to pick a consistent color
                let color_index = if let Some(ref change_id) = entry.change_id {
                    change_id
                        .bytes()
                        .fold(0usize, |acc, b| acc.wrapping_add(b as usize))
                        % change_colors.len()
                } else {
                    0
                };
                Style::default()
                    .fg(change_colors[color_index])
                    .add_modifier(Modifier::BOLD)
            };

            RenderedLogChrome {
                timestamp,
                timestamp_style: Style::default().fg(Color::DarkGray),
                header,
                header_style,
                message_style: Style::default().fg(entry.color),
            }
        })
        .collect();

    // One wrapped display-line sequence, shared with navigation.
    let lines = log_logic::build_log_display_lines(&entries, area.width as usize);
    let total_display_lines = lines.len();

    let start_line = app.log_start_line(&lines);
    let end_line = total_display_lines.min(start_line + visible_height);

    let log_items: Vec<Line> = lines[start_line..end_line]
        .iter()
        .map(|line| {
            let chrome = &chrome[line.entry_index];
            let mut spans = Vec::new();

            if line.is_first {
                // First line: timestamp and header, then the message remainder.
                spans.push(Span::styled(
                    chrome.timestamp.clone(),
                    chrome.timestamp_style,
                ));
                if !chrome.header.is_empty() {
                    spans.push(Span::styled(chrome.header.clone(), chrome.header_style));
                }
            }
            // Continuation lines start at column zero and use the full inner width.
            spans.push(Span::styled(line.text.clone(), chrome.message_style));

            Line::from(spans)
        })
        .collect();

    // Build title with display-line position indicator, auto-scroll status, and
    // compact navigation help.
    let title = logs_panel_title(LogsPanelTitle {
        lines_below: total_display_lines.saturating_sub(end_line),
        log_auto_scroll: app.log_auto_scroll,
        total_display_lines,
        visible_height,
        start_line,
        end_line,
        filter_enabled: app.selected_proposal_log_filter,
        filter_target: app.selected_proposal_log_filter_target(),
        panel_width: area.width as usize,
    });

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
    let start_key_label = app.start_key_label();

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
        // Changes exist but none selected. A settled terminal-error row — with or
        // without a retained Apply iteration-limit diagnostic — is an ordinary
        // retryable row here, because that is exactly how the shared service
        // classifies it.
        let retryable_error_rows = app
            .changes
            .iter()
            .any(|change| change.display_status_cache == "error");
        let message = if retryable_error_rows {
            "Select changes with Space to process (error rows need retry mark)"
        } else {
            "Select changes with Space to process"
        };
        spans.push(Span::styled(message, Style::default().fg(Color::Yellow)));
    } else {
        // Changes selected and ready to process
        spans.push(Span::styled(
            format!("Press {start_key_label} to start processing"),
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
            // Where the row's ahead/conflict facts came from. A checked row says
            // nothing, so only a reused or skipped observation is called out.
            let inspection_status = wt.inspection_label();
            let inspection_indicator = if inspection_status.is_empty() {
                String::new()
            } else {
                format!(" [{}]", inspection_status)
            };
            let deleting = app.is_worktree_deleting(&wt.path);
            let delete_indicator = if deleting { " [Deleting...]" } else { "" };

            let line = format!(
                "{} → {}{}{}{}{}{}",
                label,
                branch,
                indicator,
                merge_indicator,
                inspection_indicator,
                conflict_badge,
                delete_indicator
            );

            // Style based on conflict and selection
            let mut style = Style::default();

            if deleting {
                style = style.fg(Color::Yellow);
            } else if wt.has_merge_conflict() {
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

        // Show M (merge) key exactly when the request would be accepted. The
        // guard is the single source of that answer, so an uninspected row keeps
        // its affordance and is resolved by the service's fresh observation
        // rather than being hidden by a check that never ran.
        if !app.is_resolving()
            && matches!(
                guards::validate_worktree_mergeable(wt),
                guards::MergeGuardResult::Allowed
            )
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
    } else if let Some(label) = app.deleting_worktree_status_label() {
        Span::styled(
            format!("Deleting worktree: {}", label),
            Style::default().fg(Color::Yellow),
        )
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
    let Some(ModalState::ConfirmWorktreeDelete { path, .. }) = &app.modal else {
        return;
    };
    let path = path.display();

    let modal_width = (area.width * 60 / 100).clamp(40, 90);
    // Tall enough for all seven body lines plus borders. The key hints are the
    // last of them, and a confirmation that clips the keys it accepts is worse
    // than one that takes more of a short terminal.
    let modal_height = (area.height * 30 / 100).clamp(9, 12).min(area.height);
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
            "This will remove the worktree directory permanently, including",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "generated and ignored contents inside it.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Y: run teardown and delete   S: skip teardown and delete",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "N or Esc: cancel",
            Style::default().fg(Color::White),
        )),
    ];

    let body = Paragraph::new(lines);
    frame.render_widget(body, inner_area);
}

/// Render the second, explicitly destructive confirmation for a dirty worktree.
///
/// This modal exists because the first one is not enough: the operator has
/// already asked for a deletion and been told the worktree still holds
/// uncommitted work. What it must state, and what makes `X` a different decision
/// from `Y`, is that the work is discarded rather than preserved anywhere.
fn render_worktree_dirty_discard_confirm(frame: &mut Frame, app: &AppState, area: Rect) {
    let Some(ModalState::ConfirmDirtyDiscard {
        path,
        branch,
        skip_teardown,
        ..
    }) = &app.modal
    else {
        return;
    };

    let modal_width = (area.width * 70 / 100).clamp(46, 100);
    // Ten body lines plus borders: the `X` hint is the last line, and clipping
    // it would leave the operator looking at a warning with no stated way out.
    let modal_height = (area.height * 40 / 100).clamp(12, 16).min(area.height);
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Discard Uncommitted Changes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let teardown_line = if *skip_teardown {
        "Teardown will be skipped (S was pressed)."
    } else {
        "Teardown will run before removal (Y was pressed)."
    };

    let lines = vec![
        Line::from(Span::styled(
            format!("'{}' has uncommitted changes.", branch),
            Style::default().fg(Color::Red),
        )),
        Line::from(Span::styled(
            format!("{}", path.display()),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Tracked and staged edits and reported untracked files will be",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "permanently lost. Generated and ignored contents of the",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "directory go with it. Nothing is stashed, committed, or backed up.",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            teardown_line,
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press uppercase X to discard and delete, N or Esc to keep it.",
            Style::default().fg(Color::White),
        )),
    ];

    let body = Paragraph::new(lines);
    frame.render_widget(body, inner_area);
}

/// Render the destructive confirmation for a worktree ahead of base.
///
/// This is the only confirmation in the TUI that authorizes deleting an unmerged
/// branch, so it names every resource that goes: the worktree, the branch, and
/// the commits only that branch reaches. When the same observation also found
/// uncommitted work, that loss is stated here too — one keypress grants both
/// permissions, and it may only do so over a disclosure that covered both.
fn render_worktree_ahead_discard_confirm(frame: &mut Frame, app: &AppState, area: Rect) {
    let Some(ModalState::ConfirmAheadDiscard {
        path,
        branch,
        head,
        dirty,
        skip_teardown,
        ..
    }) = &app.modal
    else {
        return;
    };

    let modal_width = (area.width * 70 / 100).clamp(46, 100);
    // Thirteen body lines plus borders, one more than the dirty confirmation:
    // the branch/HEAD line and the conditional dirty line are what make this
    // disclosure complete, and a clipped modal would be an incomplete one.
    let modal_height = (area.height * 45 / 100).clamp(15, 18).min(area.height);
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Discard Unmerged Commits ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let teardown_line = if *skip_teardown {
        "Teardown will be skipped (S was pressed)."
    } else {
        "Teardown will run before removal (Y was pressed)."
    };

    let mut lines = vec![
        Line::from(Span::styled(
            format!("'{}' has commits that base does not have.", branch),
            Style::default().fg(Color::Red),
        )),
        Line::from(Span::styled(
            format!("{} at {}", path.display(), head),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "The worktree, the local branch, and every unmerged commit on it",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "will be permanently deleted. Nothing is merged, pushed, tagged,",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "stashed, or backed up first, and the commits are not recoverable.",
            Style::default().fg(Color::Yellow),
        )),
    ];

    if *dirty {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "This worktree also has uncommitted changes. They are discarded",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(Span::styled(
            "by the same keypress.",
            Style::default().fg(Color::Yellow),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        teardown_line,
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press uppercase X to discard and delete, N or Esc to keep it.",
        Style::default().fg(Color::White),
    )));

    let body = Paragraph::new(lines);
    frame.render_widget(body, inner_area);
}

fn warning_popup_modal_area(area: Rect) -> Rect {
    let modal_width = (area.width.saturating_mul(85) / 100)
        .max(40)
        .min(area.width.saturating_sub(2).max(1));
    let modal_height = (area.height.saturating_mul(70) / 100)
        .max(10)
        .min(area.height.saturating_sub(2).max(1));
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    Rect::new(modal_x, modal_y, modal_width, modal_height)
}

fn warning_popup_message_lines(message: &str) -> Vec<Line<'_>> {
    message.split('\n').map(Line::from).collect()
}

/// Render the warning popup modal
fn render_warning_popup(frame: &mut Frame, app: &AppState, area: Rect) {
    let Some(popup) = &app.warning_popup else {
        return;
    };

    let modal_area = warning_popup_modal_area(area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(format!(" {} ", popup.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner_area);

    let body = Paragraph::new(warning_popup_message_lines(&popup.message))
        .style(Style::default().fg(Color::Yellow))
        .scroll((app.warning_popup_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("↑↓/jk PgUp/PgDn", Style::default().fg(Color::Cyan)),
        Span::raw(" scroll  "),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::raw(" close"),
    ]))
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[1]);
}

/// Body lines of the Error Details popup: the change ID, then the complete
/// untruncated diagnostic.
///
/// A diagnostic that carries explicit newlines keeps them as separate lines;
/// anything wider than the popup is wrapped by the caller rather than cut, so no
/// part of the failure text is lost.
fn error_details_popup_lines(popup: &ErrorDetailsPopup) -> Vec<Line<'_>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Change: ", Style::default().fg(Color::Cyan)),
            Span::styled(popup.change_id.as_str(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
    ];
    lines.extend(
        popup
            .error
            .split('\n')
            .map(|line| Line::from(Span::styled(line, Style::default().fg(Color::LightRed)))),
    );
    lines
}

/// Render the Error Details popup for a change-level failure.
fn render_error_details_popup(frame: &mut Frame, app: &AppState, area: Rect) {
    let Some(popup) = &app.error_details_popup else {
        return;
    };

    let modal_area = warning_popup_modal_area(area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Error Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    // Body, then a single feedback line, then the always-visible key guidance.
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner_area);

    let body = Paragraph::new(error_details_popup_lines(popup))
        .scroll((popup.scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[0]);

    let feedback = match &popup.copy_feedback {
        Some(feedback) => {
            let color = match feedback {
                CopyFeedback::Copied => Color::Green,
                CopyFeedback::Failed(_) => Color::Yellow,
            };
            Paragraph::new(feedback.message()).style(Style::default().fg(color))
        }
        None => Paragraph::new(""),
    };
    frame.render_widget(feedback, chunks[1]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("↑↓/jk PgUp/PgDn", Style::default().fg(Color::Cyan)),
        Span::raw(" scroll  "),
        Span::styled("c", Style::default().fg(Color::Cyan)),
        Span::raw(": copy  "),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::raw(": close"),
    ]))
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
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
    use super::log_logic::wrap_log_message;
    use super::*;
    use crate::openspec::Change;
    use crate::openspec::ProposalMetadata;
    use crate::orchestration::operator_command::ParallelEligibility;
    use crate::tui::config::TuiConfig;
    use crate::tui::events::LogEntry;
    use crate::tui::types::{ViewMode, WorktreeInfo};
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;
    use std::collections::HashSet;

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 3,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    /// The project path every render test starts from.
    ///
    /// Fixed so header assertions never depend on where the suite happens to
    /// run, and short enough that a normal-width terminal renders it whole.
    const TEST_PROJECT_PATH: &str = "/projects/conflux";

    fn create_test_app(changes: Vec<Change>) -> AppState {
        let mut app = AppState::new(changes);
        app.logs.clear();
        app.web_url = None;
        app.set_project_path(TEST_PROJECT_PATH);
        app
    }

    fn create_test_worktree(path: &str, branch: &str) -> WorktreeInfo {
        WorktreeInfo {
            path: path.into(),
            head: "abc123".to_string(),
            branch: branch.to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
            inspection: crate::worktree_ops::InspectionState::Checked,
        }
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

    /// Worktree orchestration is the only execution model, so the TUI exposes
    /// no mode toggle and no mode badge — in any app mode.
    #[test]
    fn no_execution_mode_toggle_or_badge_is_ever_rendered() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            let mut app = create_test_app(vec![create_test_change("change-a")]);
            app.execution_mode = mode;

            let buffer = render_buffer(&mut app, 120, 30);
            let content = buffer_to_string(&buffer);

            for forbidden in [
                "=: parallel",
                "=: sequential",
                "[parallel:",
                // The concurrency/backend badge is retired: the header slot now
                // reports which project this instance owns.
                "[workspaces:",
            ] {
                assert!(
                    !content.contains(forbidden),
                    "{mode:?} must not render '{forbidden}':\n{content}"
                );
            }
            assert!(
                content.contains(TEST_PROJECT_PATH),
                "{mode:?} must identify the owned project:\n{content}"
            );
        }
    }

    /// The header row, as rendered text.
    fn header_line(buffer: &Buffer) -> String {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, 1)].symbol());
        }
        line
    }

    fn observe_workspace_dirty(app: &mut AppState, dirty: bool) {
        app.adopt_workspace_dirty_observation(
            crate::tui::events::TuiRefreshObservation::WorkspaceDirty { dirty },
        );
    }

    /// Only a known dirty observation draws the badge. Clean and unknown both
    /// omit it, and neither omission may cost the header its existing content.
    #[test]
    fn workspace_dirty_header_shows_the_badge_only_for_an_observed_dirty_workspace() {
        // (label, how the observation got there, whether the badge is expected)
        let cases: [(&str, Option<bool>, bool); 3] = [
            ("unknown", None, false),
            ("clean", Some(false), false),
            ("dirty", Some(true), true),
        ];

        for (label, observation, expects_badge) in cases {
            let mut app = create_test_app(vec![create_test_change("change-a")]);
            if let Some(dirty) = observation {
                observe_workspace_dirty(&mut app, dirty);
            }

            let buffer = render_buffer(&mut app, 120, 30);
            let header = header_line(&buffer);

            assert_eq!(
                header.contains("[dirty]"),
                expects_badge,
                "{label} observation rendered the wrong badge state:\n{header}"
            );
            assert!(
                header.contains("[Ready]"),
                "{label}: the status label must survive:\n{header}"
            );
            assert!(
                header.contains(TEST_PROJECT_PATH),
                "{label}: the project path must survive:\n{header}"
            );
            assert!(
                !header.contains("[workspaces:"),
                "{label}: the retired concurrency/backend badge must not return:\n{header}"
            );
            assert!(
                header.contains(&crate::tui::utils::get_version_string()),
                "{label}: the right-aligned version area must still render:\n{header}"
            );
        }

        // The badge sits after the project path, not in place of it.
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        observe_workspace_dirty(&mut app, true);
        let buffer = render_buffer(&mut app, 120, 30);
        let header = header_line(&buffer);
        let project_at = header.find(TEST_PROJECT_PATH).expect("project path");
        let dirty_at = header.find("[dirty]").expect("dirty badge");
        assert!(
            project_at < dirty_at,
            "the dirty badge must follow the project path:\n{header}"
        );

        // Warning styling: the badge is a caution, so it must read as one.
        let cell = buffer
            .cell((dirty_at as u16, 1))
            .expect("dirty badge cell exists");
        assert_eq!(cell.style().fg, Some(Color::Red));
        assert!(
            cell.style().add_modifier.contains(Modifier::BOLD),
            "the dirty badge must render bold"
        );
    }

    /// The badge coexists with every other header owner: the running count, an
    /// active modal's relabel, and the right-aligned version area — at both a
    /// narrow and a wide terminal.
    #[test]
    fn workspace_dirty_header_coexists_with_status_modal_and_version_content() {
        for width in [80, 120, 200] {
            let mut app = create_test_app(vec![create_test_change("change-a")]);
            app.execution_mode = AppExecutionMode::Running;
            app.changes[0].set_display_status_cache("applying");
            observe_workspace_dirty(&mut app, true);

            let buffer = render_buffer(&mut app, width, 30);
            let header = header_line(&buffer);
            assert!(
                header.contains("[Running 1]"),
                "width {width}: the running status must survive:\n{header}"
            );
            assert!(
                header.contains(TEST_PROJECT_PATH) && header.contains("[dirty]"),
                "width {width}: the project path and the dirty badge must render:\n{header}"
            );
            assert!(
                header.contains(&crate::tui::utils::get_version_string()),
                "width {width}: the version area must still render:\n{header}"
            );

            // An overlay relabels the status; it does not take the badge away.
            app.modal = Some(ModalState::QrPopup);
            let buffer = render_buffer(&mut app, width, 30);
            let header = header_line(&buffer);
            assert!(
                header.contains("[dirty]"),
                "width {width}: a modal must not suppress the badge:\n{header}"
            );
            assert!(
                !header.contains("[Running 1]"),
                "width {width}: the modal still owns the status label:\n{header}"
            );
        }
    }

    /// Every retained column of a middle-elided string still comes from the
    /// source string, on the side it came from. This is the proof that no
    /// character representation was cut in half.
    fn assert_elision_retains_source_sides(source: &str, elided: &str) {
        let (prefix, suffix) = elided
            .split_once(MIDDLE_ELLIPSIS)
            .unwrap_or_else(|| panic!("{elided:?} carries no ellipsis"));
        assert!(
            source.starts_with(prefix),
            "{elided:?} kept a prefix {prefix:?} that {source:?} does not start with"
        );
        assert!(
            source.ends_with(suffix),
            "{elided:?} kept a suffix {suffix:?} that {source:?} does not end with"
        );
    }

    /// Exact middle elision for ASCII, including the suffix-favoring split of an
    /// odd non-ellipsis budget.
    #[test]
    fn middle_elision_splits_an_ascii_path_around_one_ellipsis() {
        let path = "/projects/conflux";
        assert_eq!(display_width(path), 17);

        // Fits: returned untouched, at exact width and above it.
        assert_eq!(middle_elide_to_display_width(path, 17), path);
        assert_eq!(middle_elide_to_display_width(path, 40), path);

        // Even non-ellipsis budget: both sides keep the same column count.
        assert_eq!(middle_elide_to_display_width(path, 9), "/pro…flux");
        // Odd non-ellipsis budget: the spare column goes to the suffix, because
        // the tail of a path is what distinguishes two checkouts.
        assert_eq!(middle_elide_to_display_width(path, 10), "/pro…nflux");
        assert_eq!(middle_elide_to_display_width(path, 16), "/projec…/conflux");

        for budget in [9, 10, 16] {
            let elided = middle_elide_to_display_width(path, budget);
            assert_eq!(display_width(&elided), budget);
            assert_elision_retains_source_sides(path, &elided);
        }
    }

    /// Wide characters are accounted in columns, not scalars, and a side that
    /// cannot fit a whole wide character simply keeps fewer of them.
    #[test]
    fn middle_elision_measures_wide_unicode_in_display_columns() {
        let path = "/日本語/conflux";
        assert_eq!(display_width(path), 15);
        assert_eq!(path.chars().count(), 12);

        assert_eq!(middle_elide_to_display_width(path, 8), "/日…flux");
        // One column narrower is one *column*, not one character: the wide
        // prefix cannot give up half of 日, so the suffix pays instead.
        assert_eq!(middle_elide_to_display_width(path, 7), "/日…lux");

        for budget in 1..=display_width(path) {
            let elided = middle_elide_to_display_width(path, budget);
            assert!(
                display_width(&elided) <= budget,
                "budget {budget} produced {elided:?}, which is wider than its budget"
            );
        }
    }

    /// A combining mark carries no column of its own, so it must travel with the
    /// base character it modifies rather than being orphaned onto the ellipsis.
    #[test]
    fn middle_elision_keeps_combining_marks_with_their_base_characters() {
        let path = "/a\u{301}bcdefg/hij\u{301}";
        assert_eq!(display_width(path), 12);

        let elided = middle_elide_to_display_width(path, 7);
        assert_eq!(elided, "/a\u{301}b…hij\u{301}");
        assert_eq!(display_width(&elided), 7);
        assert_elision_retains_source_sides(path, &elided);

        // No side may start or end on a mark whose base was left behind.
        let (prefix, suffix) = elided.split_once(MIDDLE_ELLIPSIS).expect("ellipsis");
        assert!(!prefix.ends_with('\u{301}') || prefix.chars().count() > 1);
        assert!(
            !suffix.starts_with('\u{301}'),
            "{suffix:?} orphans a combining mark onto the ellipsis"
        );
    }

    /// Budgets at or below the width of the ellipsis stay bounded instead of
    /// panicking or overflowing their assigned columns.
    #[test]
    fn middle_elision_stays_bounded_for_budgets_no_wider_than_the_ellipsis() {
        let samples = [
            "/projects/conflux",
            "/日本語/conflux",
            "/a\u{301}bcdefg/hij\u{301}",
            "/",
        ];

        for source in samples {
            assert_eq!(middle_elide_to_display_width(source, 0), "");
            for budget in 0..=display_width(source) + 2 {
                let elided = middle_elide_to_display_width(source, budget);
                assert!(
                    display_width(&elided) <= budget,
                    "{source:?} at budget {budget} produced {elided:?}"
                );
            }
        }

        // A source wider than one column and a one-column budget: the gap is all
        // that is left, and it is exactly one column.
        assert_eq!(middle_elide_to_display_width("/projects/conflux", 1), "…");
        assert_eq!(middle_elide_to_display_width("/日本語/conflux", 1), "…");
    }

    /// The header identifies the owned project, and the retired concurrency /
    /// backend badge does not come back with it.
    #[test]
    fn header_shows_the_captured_project_path_instead_of_the_workspaces_badge() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        let buffer = render_buffer(&mut app, 120, 30);
        let header = header_line(&buffer);

        assert!(
            header.contains(TEST_PROJECT_PATH),
            "the captured project path must be visible in full:\n{header}"
        );
        assert!(
            !header.contains("[workspaces:"),
            "the concurrency/backend badge must be gone:\n{header}"
        );
        assert!(
            !header.contains(MIDDLE_ELLIPSIS),
            "a path that fits must not be elided:\n{header}"
        );
    }

    /// Header identity is a function of the captured startup path alone.
    ///
    /// Rendering reads `AppState::project_path`, whose only writer is
    /// `set_project_path`, and never re-resolves identity from ambient process
    /// state. Two otherwise identical apps therefore differ in the header by
    /// exactly their captured paths — which is what makes a later `chdir`
    /// unable to retarget the header, since nothing consults the current
    /// directory after startup.
    #[test]
    fn header_project_identity_comes_only_from_the_captured_startup_path() {
        let mut owned = create_test_app(vec![create_test_change("change-a")]);
        owned.set_project_path("/projects/conflux");
        let mut elsewhere = create_test_app(vec![create_test_change("change-a")]);
        elsewhere.set_project_path("/tmp");

        let owned_header = header_line(&render_buffer(&mut owned, 120, 30));
        let elsewhere_header = header_line(&render_buffer(&mut elsewhere, 120, 30));

        assert!(
            owned_header.contains("/projects/conflux"),
            "the captured project must be the one shown:\n{owned_header}"
        );
        assert!(
            !owned_header.contains("/tmp"),
            "an unrelated directory must not appear:\n{owned_header}"
        );
        assert!(
            elsewhere_header.contains("/tmp"),
            "the other app shows its own captured path:\n{elsewhere_header}"
        );

        // Rendering is a read: the capture survives repeated frames unchanged.
        let _ = render_buffer(&mut owned, 120, 30);
        assert_eq!(
            owned.project_path(),
            std::path::Path::new("/projects/conflux")
        );
    }

    /// A path wider than its header budget is middle-elided in place, without
    /// displacing the status label, the dirty badge, or the version area.
    #[test]
    fn header_middle_elides_a_project_path_wider_than_its_budget() {
        let long_path =
            "/Users/operator/very/deeply/nested/checkouts/organization/conflux-worktrees/show-project-path-in-tui-header";
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.set_project_path(long_path);
        observe_workspace_dirty(&mut app, true);

        let buffer = render_buffer(&mut app, 80, 30);
        let header = header_line(&buffer);

        assert!(
            !header.contains(long_path),
            "the full path cannot fit at width 80:\n{header}"
        );
        assert!(
            header.contains(MIDDLE_ELLIPSIS),
            "an over-wide path must be elided:\n{header}"
        );
        assert!(
            header.contains("/Users/") && header.contains("-tui-header"),
            "both the path prefix and its distinguishing tail must survive:\n{header}"
        );
        assert!(
            header.contains("[Ready]") && header.contains("[dirty]"),
            "elision must not displace the other header segments:\n{header}"
        );
        assert!(
            header.contains(&crate::tui::utils::get_version_string()),
            "the right-aligned version must stay intact:\n{header}"
        );

        // The path stays on the header row: nothing spilled into the row below.
        let second_row = {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer[(x, 2)].symbol());
            }
            line
        };
        assert!(
            !second_row.contains("-tui-header"),
            "the header must not wrap onto the next row:\n{second_row}"
        );
    }

    /// Narrow terminals stay bounded: the header renders, the version area keeps
    /// its reservation, and nothing panics down to a single column.
    #[test]
    fn header_project_path_rendering_stays_bounded_at_narrow_widths() {
        for width in [1u16, 2, 4, 8, 12, 20, 24, 30, 40, 60] {
            let mut app = create_test_app(vec![create_test_change("change-a")]);
            app.set_project_path("/Users/operator/checkouts/conflux");
            observe_workspace_dirty(&mut app, true);

            let buffer = render_buffer(&mut app, width, 20);
            let header = header_line(&buffer);
            assert_eq!(
                header.chars().count(),
                width as usize,
                "width {width}: the header row must fill exactly its terminal width"
            );
            assert!(
                header.matches(MIDDLE_ELLIPSIS).count() <= 1,
                "width {width}: at most one elision gap belongs in the header:\n{header}"
            );
        }
    }

    fn find_row_containing(buffer: &Buffer, needle: &str) -> Option<u16> {
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            if line.contains(needle) {
                return Some(y);
            }
        }
        None
    }

    #[test]
    fn running_logs_enabled_layout_expands_logs_for_few_changes() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.add_log(LogEntry::info("expanded log area"));

        let buffer = render_buffer(&mut app, 100, 50);

        assert_eq!(find_row_containing(&buffer, " Logs"), Some(11));
        assert_eq!(find_row_containing(&buffer, " Status"), Some(8));
    }

    #[test]
    fn running_logs_enabled_layout_keeps_target_logs_height_for_many_changes() {
        let changes = (0..30)
            .map(|index| create_test_change(&format!("change-{index:02}")))
            .collect();
        let mut app = create_test_app(changes);
        app.execution_mode = AppExecutionMode::Running;
        app.add_log(LogEntry::info("target log area"));

        let buffer = render_buffer(&mut app, 100, 50);

        assert_eq!(find_row_containing(&buffer, " Logs"), Some(30));
        assert_eq!(find_row_containing(&buffer, " Status"), Some(27));
    }

    #[test]
    fn running_mode_shows_new_change_indicator_with_logs_and_many_changes() {
        let changes = (0..30)
            .map(|index| create_test_change(&format!("change-{index:02}")))
            .collect();
        let mut app = create_test_app(changes);
        app.execution_mode = AppExecutionMode::Running;
        app.logs_panel_enabled = true;
        app.new_change_count = 1;
        app.changes[29].is_new = true;
        app.add_log(LogEntry::info("log area"));

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("New: 1"));
        assert!(content.contains(" Logs"));
    }

    #[test]
    fn running_logs_disabled_layout_does_not_render_logs_panel() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.logs_panel_enabled = false;
        app.add_log(LogEntry::info("hidden log area"));

        let buffer = render_buffer(&mut app, 100, 50);
        let content = buffer_to_string(&buffer);

        assert!(content.contains(" Status"));
        assert!(!content.contains(" Logs"));
        assert!(!content.contains("hidden log area"));
    }

    #[test]
    fn worktree_view_renders_deleting_badge_and_footer_status() {
        let mut app = create_test_app(vec![]);
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a")];
        app.mark_worktree_deleting("/tmp/worktree-a");

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("[Deleting...]"));
        assert!(content.contains("Deleting worktree: worktree-a"));
    }

    #[test]
    fn warning_popup_message_lines_preserve_explicit_newlines() {
        let lines = warning_popup_message_lines("first\nsecond\nthird");

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].spans[0].content, "first");
        assert_eq!(lines[1].spans[0].content, "second");
        assert_eq!(lines[2].spans[0].content, "third");
    }

    #[test]
    fn warning_popup_render_shows_footer_hint_and_multiline_content() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.show_warning_popup(
            "Hook failed",
            "first diagnostic line\nsecond diagnostic line",
        );

        let buffer = render_buffer(&mut app, 100, 30);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("Hook failed"));
        assert!(content.contains("first diagnostic line"));
        assert!(content.contains("second diagnostic line"));
        assert!(content.contains("PgUp/PgDn"));
        assert!(content.contains("Esc"));
    }

    #[test]
    fn warning_popup_uses_diagnostics_sized_modal_area() {
        let area = Rect::new(0, 0, 100, 30);
        let modal = warning_popup_modal_area(area);

        assert_eq!(modal.width, 85);
        assert_eq!(modal.height, 21);
    }

    #[test]
    fn qr_popup_render_shows_url_and_close_hint() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.web_url = Some("http://127.0.0.1:8080".to_string());
        app.show_qr_popup();

        let buffer = render_buffer(&mut app, 100, 40);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("Web UI QR Code"));
        assert!(content.contains("http://127.0.0.1:8080"));
    }

    #[test]
    fn remote_grouping_characterization_shows_project_headers_and_bare_change_ids() {
        let mut app = create_test_app(vec![
            create_test_change("p1::alpha/change-one"),
            create_test_change("p2::beta/change-two"),
        ]);
        app.cursor_index = 1;

        let buffer = render_buffer(&mut app, 120, 24);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("alpha"));
        assert!(content.contains("beta"));
        assert!(content.contains("change-one"));
        assert!(content.contains("change-two"));
        assert!(!content.contains("p1::alpha/change-one"));
    }

    #[test]
    fn status_logs_characterization_shows_progress_elapsed_and_log_header() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].selected = true;
        app.changes[0].completed_tasks = 1;
        app.changes[0].total_tasks = 4;
        app.orchestration_started_at = Some(std::time::Instant::now());
        app.add_log(
            LogEntry::info("Applying patch")
                .with_change_id("change-a")
                .with_operation("apply")
                .with_iteration(2),
        );

        let buffer = render_buffer(&mut app, 120, 24);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("25.0%"));
        assert!(content.contains("Elapsed"));
        assert!(content.contains("[change-a:apply:2]"));
        assert!(content.contains("Applying patch"));
    }

    #[test]
    fn worktree_view_characterization_shows_key_hints_and_delete_confirmation() {
        let mut app = create_test_app(vec![]);
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a")];
        app.modal = Some(ModalState::ConfirmWorktreeDelete {
            path: std::path::PathBuf::from("/tmp/worktree-a"),
            branch: "feature-a".to_string(),
        });

        let buffer = render_buffer(&mut app, 120, 30);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("Worktrees"));
        assert!(content.contains("feature-a"));
        assert!(content.contains("Tab: changes"));
        assert!(content.contains("D: delete"));
        assert!(content.contains("Delete Worktree"));
        // Both teardown choices are stated, because the operator picks between
        // them here and the choice is carried into any later confirmation.
        assert!(content.contains("Y: run teardown and delete"));
        assert!(content.contains("S: skip teardown and delete"));
        assert!(content.contains("N or Esc: cancel"));
        assert!(
            content.contains("generated and ignored contents"),
            "the ordinary warning must still say the directory goes with it: {content}"
        );
    }

    #[test]
    fn tui_dirty_worktree_delete_destructive_modal_states_the_loss_and_the_key() {
        let mut app = create_test_app(vec![]);
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a")];
        app.modal = Some(ModalState::ConfirmDirtyDiscard {
            path: std::path::PathBuf::from("/tmp/worktree-a"),
            identity: "gitdir: /tmp/worktree-a/.git".to_string(),
            branch: "feature-a".to_string(),
            head: "abc1234".to_string(),
            skip_teardown: false,
        });

        let buffer = render_buffer(&mut app, 120, 30);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("Discard Uncommitted Changes"));
        assert!(content.contains("has uncommitted changes"));
        assert!(content.contains("permanently lost"));
        assert!(
            content.contains("Nothing is stashed, committed, or backed up"),
            "the modal must not imply the work is recoverable: {content}"
        );
        assert!(content.contains("Press uppercase X to discard and delete"));
        assert!(content.contains("Teardown will run before removal"));
    }

    #[test]
    fn tui_dirty_worktree_delete_destructive_modal_reports_the_captured_teardown_choice() {
        let mut app = create_test_app(vec![]);
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a")];
        app.modal = Some(ModalState::ConfirmDirtyDiscard {
            path: std::path::PathBuf::from("/tmp/worktree-a"),
            identity: "gitdir: /tmp/worktree-a/.git".to_string(),
            branch: "feature-a".to_string(),
            head: "abc1234".to_string(),
            skip_teardown: true,
        });

        let buffer = render_buffer(&mut app, 120, 30);
        let content = buffer_to_string(&buffer);

        // The `S` the operator pressed is still in force here; `X` grants the
        // other permission, not this one.
        assert!(content.contains("Teardown will be skipped"));
    }

    fn ahead_discard_modal(dirty: bool, skip_teardown: bool) -> ModalState {
        ModalState::ConfirmAheadDiscard {
            path: std::path::PathBuf::from("/tmp/worktree-a"),
            identity: "gitdir: /tmp/worktree-a/.git".to_string(),
            branch: "feature-a".to_string(),
            head: "abc1234".to_string(),
            dirty,
            skip_teardown,
        }
    }

    #[test]
    fn tui_ahead_worktree_delete_destructive_modal_states_every_resource_it_deletes() {
        let mut app = create_test_app(vec![]);
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a")];
        app.modal = Some(ahead_discard_modal(false, false));

        let buffer = render_buffer(&mut app, 120, 30);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("Discard Unmerged Commits"));
        assert!(content.contains("feature-a"));
        assert!(content.contains("/tmp/worktree-a"));
        assert!(
            content.contains("abc1234"),
            "the confirmed commit is what the branch is deleted at: {content}"
        );
        assert!(
            content.contains("the local branch, and every unmerged commit"),
            "the modal must name the branch and the commits, not only the worktree: {content}"
        );
        assert!(
            content.contains("Nothing is merged, pushed, tagged"),
            "the modal must not imply the commits are preserved somewhere: {content}"
        );
        assert!(content.contains("not recoverable"));
        assert!(content.contains("Press uppercase X to discard and delete"));
        assert!(content.contains("Teardown will run before removal"));
        assert!(
            !content.contains("uncommitted changes"),
            "a clean worktree must not be told it is losing uncommitted work: {content}"
        );
    }

    #[test]
    fn tui_ahead_worktree_delete_destructive_modal_discloses_both_losses_when_dirty() {
        let mut app = create_test_app(vec![]);
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![create_test_worktree("/tmp/worktree-a", "feature-a")];
        app.modal = Some(ahead_discard_modal(true, true));

        let buffer = render_buffer(&mut app, 120, 30);
        let content = buffer_to_string(&buffer);

        // One keypress, two permissions — so one modal, both disclosures.
        assert!(content.contains("every unmerged commit"));
        assert!(content.contains("also has uncommitted changes"));
        assert!(
            content.contains("by the same keypress"),
            "the operator must be told X covers both losses: {content}"
        );
        assert!(content.contains("Teardown will be skipped"));
    }

    #[test]
    fn ahead_discard_overlay_renders_above_every_execution_mode() {
        for mode in ALL_EXECUTION_MODES {
            let mut app = overlay_app();
            app.execution_mode = mode;
            app.modal = Some(ahead_discard_modal(true, false));

            let content = buffer_to_string(&render_buffer(&mut app, 100, 40));

            assert!(
                content.contains("Discard Unmerged Commits"),
                "the ahead confirmation must render above {mode:?}"
            );
            assert!(content.contains("[Discard Commits]"));
        }
    }

    // ========================================================================
    // Post-archive checkbox placeholder
    //
    // A post-archive row cannot be a next-run target, so it shows neither
    // `[x]` nor `[ ]`. What it does keep is the column: the blank is exactly
    // checkbox-wide, so the cursor, ID, badges, status, progress, and preview
    // all stay where every other row puts them.
    // ========================================================================

    /// Columns between the start of the checkbox and the start of the change
    /// ID: the checkbox itself plus the single separator column after it.
    const CHECKBOX_TO_ID_COLUMNS: usize = CHECKBOX_WIDTH + CHECKBOX_TO_ID_SEPARATOR_WIDTH;

    /// The rendered list row that displays `change_id`, as characters.
    ///
    /// Character cells, not bytes: a wide-character ID is multi-byte, so byte
    /// offsets would not describe columns.
    fn change_row_cells(buffer: &Buffer, change_id: &str) -> Vec<char> {
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .flat_map(|x| buffer[(x, y)].symbol().chars())
                    .collect::<Vec<char>>()
            })
            .find(|cells| find_columns(cells, change_id).is_some())
            .unwrap_or_else(|| panic!("no rendered row contains `{change_id}`"))
    }

    /// Column at which `needle` starts within a rendered row.
    fn find_columns(cells: &[char], needle: &str) -> Option<usize> {
        let needle: Vec<char> = needle.chars().collect();
        cells.windows(needle.len()).position(|w| w == needle)
    }

    /// The checkbox columns of a rendered row, plus the column its ID starts at.
    fn checkbox_and_id_column(cells: &[char], change_id: &str) -> (String, usize) {
        let id_column = find_columns(cells, change_id)
            .unwrap_or_else(|| panic!("row does not contain `{change_id}`"));
        let checkbox_start = id_column
            .checked_sub(CHECKBOX_TO_ID_COLUMNS)
            .expect("the checkbox column precedes the ID column");
        (
            cells[checkbox_start..checkbox_start + CHECKBOX_WIDTH]
                .iter()
                .collect(),
            id_column,
        )
    }

    /// Two rows so both the cursor row and a non-cursor row are covered.
    ///
    /// `with_logs` picks the layout: the Changes view renders the Select list
    /// while no log is buffered and the Running/Stopped list once one is.
    fn checkbox_placeholder_app(
        mode: AppExecutionMode,
        with_logs: bool,
        cursor_index: usize,
        status: &str,
        marked: bool,
    ) -> AppState {
        let mut app = create_test_app(vec![
            create_test_change("change-a"),
            create_test_change("change-b"),
        ]);
        app.execution_mode = mode;
        app.cursor_index = cursor_index;
        app.changes[0].display_status_cache = status.to_string();
        app.changes[0].selected = marked;
        if with_logs {
            app.add_log(LogEntry::info("log"));
        }
        app
    }

    /// The placeholder must measure exactly what it replaces: the row's preview
    /// width is computed from the checkbox text's own width, so a placeholder
    /// of any other size would silently resize the preview column too.
    #[test]
    fn archived_checkbox_placeholder_is_exactly_checkbox_wide() {
        assert_eq!(CHECKBOX_PLACEHOLDER.len(), CHECKBOX_WIDTH);
        assert_eq!(CHECKBOX_WIDTH, "[x]".len());
        assert_eq!(CHECKBOX_WIDTH, "[ ]".len());
        assert!(
            CHECKBOX_PLACEHOLDER.chars().all(|c| c == ' '),
            "the placeholder must be blank, not a third checkbox state"
        );
    }

    /// Every post-archive presentation drops the checkbox, and it drops it
    /// whether or not a mark survived into that state.
    #[test]
    fn archived_checkbox_placeholder_replaces_both_checkbox_states() {
        for status in ["archived", "merged", "pushed"] {
            for marked in [true, false] {
                let (text, color) = get_checkbox_display(status, marked, false);
                assert_eq!(
                    text, CHECKBOX_PLACEHOLDER,
                    "`{status}` (marked={marked}) must render no checkbox"
                );
                assert_eq!(color, Color::DarkGray);
            }
        }

        // A non-terminal row still reports the mark it actually holds.
        assert_eq!(get_checkbox_display("not queued", true, false).0, "[x]");
        assert_eq!(get_checkbox_display("not queued", false, false).0, "[ ]");
    }

    /// The column regression: in both list layouts, on the cursor row and off
    /// it, a post-archive row renders blank checkbox cells and leaves the ID
    /// column — and therefore everything after it — exactly where a
    /// non-terminal row puts it.
    #[test]
    fn archived_checkbox_placeholder_holds_the_column_in_both_layouts() {
        for (layout, with_logs) in [("select", false), ("running", true)] {
            for mode in [
                AppExecutionMode::Select,
                AppExecutionMode::Running,
                AppExecutionMode::Stopped,
            ] {
                for cursor_index in [0, 1] {
                    let mut baseline = checkbox_placeholder_app(
                        mode,
                        with_logs,
                        cursor_index,
                        "not queued",
                        false,
                    );
                    let baseline_buffer = render_buffer(&mut baseline, 120, 30);
                    let (baseline_checkbox, baseline_id_column) = checkbox_and_id_column(
                        &change_row_cells(&baseline_buffer, "change-a"),
                        "change-a",
                    );
                    assert_eq!(
                        baseline_checkbox, "[ ]",
                        "{layout}/{mode:?}/cursor={cursor_index}: the baseline row must own a checkbox"
                    );

                    for status in ["archived", "merged", "pushed"] {
                        for marked in [true, false] {
                            let mut app = checkbox_placeholder_app(
                                mode,
                                with_logs,
                                cursor_index,
                                status,
                                marked,
                            );
                            let buffer = render_buffer(&mut app, 120, 30);
                            let cells = change_row_cells(&buffer, "change-a");
                            let (checkbox, id_column) = checkbox_and_id_column(&cells, "change-a");

                            assert_eq!(
                                checkbox, CHECKBOX_PLACEHOLDER,
                                "{layout}/{mode:?}/cursor={cursor_index}: `{status}` \
                                 (marked={marked}) must render blank checkbox cells"
                            );
                            assert_eq!(
                                id_column, baseline_id_column,
                                "{layout}/{mode:?}/cursor={cursor_index}: `{status}` \
                                 (marked={marked}) must not shift the columns after the checkbox"
                            );
                        }
                    }
                }
            }
        }
    }

    // ========================================================================
    // Changes row layout: reducer-recorded archive completion hides the checkbox
    //
    // The checkbox and the mark hint answer the same question, so they are
    // asserted together against one predicate. `resolving` appears on both sides
    // of every case here: with an archive record it is post-archive, without one
    // it is an ordinary active row.
    // ========================================================================

    /// Live post-archive statuses plus the terminal ones a finished post-archive
    /// lane reaches.
    const ARCHIVE_COMPLETE_ROW_STATUSES: [&str; 5] = [
        "resolving",
        "resolve pending",
        "merge wait",
        "merged",
        "pushed",
    ];

    /// One archive-complete row and one ordinary row, for either list layout.
    fn archive_complete_app(
        mode: AppExecutionMode,
        with_logs: bool,
        status: &str,
        marked: bool,
        archive_complete: bool,
    ) -> AppState {
        let mut app = create_test_app(vec![
            create_test_change("change-a"),
            create_test_change("change-b"),
        ]);
        app.execution_mode = mode;
        app.cursor_index = 0;
        app.changes[0].set_display_status_cache(status);
        app.changes[0].selected = marked;
        app.changes[0].archive_complete_cache = archive_complete;
        if with_logs {
            app.add_log(LogEntry::info("log"));
        }
        app
    }

    /// A reducer-archived row renders the blank three-column placeholder even
    /// while its display status is a live post-archive one, and it holds the
    /// columns after it.
    #[test]
    fn tui_change_row_layout_mark_contract_archive_complete_row_hides_the_checkbox() {
        for (layout, with_logs) in [("select", false), ("running", true)] {
            for mode in [
                AppExecutionMode::Select,
                AppExecutionMode::Running,
                AppExecutionMode::Stopped,
            ] {
                let mut baseline =
                    archive_complete_app(mode, with_logs, "not queued", false, false);
                let baseline_buffer = render_buffer(&mut baseline, 120, 30);
                let (baseline_checkbox, baseline_id_column) = checkbox_and_id_column(
                    &change_row_cells(&baseline_buffer, "change-a"),
                    "change-a",
                );
                assert_eq!(baseline_checkbox, "[ ]");

                for status in ARCHIVE_COMPLETE_ROW_STATUSES {
                    for marked in [true, false] {
                        let mut app = archive_complete_app(mode, with_logs, status, marked, true);
                        let buffer = render_buffer(&mut app, 120, 30);
                        let (checkbox, id_column) = checkbox_and_id_column(
                            &change_row_cells(&buffer, "change-a"),
                            "change-a",
                        );

                        assert_eq!(
                            checkbox, CHECKBOX_PLACEHOLDER,
                            "{layout}/{mode:?}: archive-complete `{status}` \
                             (marked={marked}) must render neither [x] nor [ ]"
                        );
                        assert_eq!(
                            checkbox.chars().count(),
                            CHECKBOX_WIDTH,
                            "{layout}/{mode:?}: the placeholder stays three columns"
                        );
                        assert_eq!(
                            id_column, baseline_id_column,
                            "{layout}/{mode:?}: archive-complete `{status}` must not \
                             shift the ID or the fields after it"
                        );
                    }
                }
            }
        }
    }

    /// The control: the same statuses with no reducer archive record still show
    /// the mark the operator actually holds.
    #[test]
    fn tui_change_row_layout_mark_contract_row_without_an_archive_record_keeps_its_checkbox() {
        for (layout, with_logs) in [("select", false), ("running", true)] {
            // Only the non-terminal statuses: `merged` and `pushed` are terminal
            // and hide the checkbox on their status alone.
            for status in ["resolving", "resolve pending", "merge wait"] {
                for (marked, expected) in [(true, "[x]"), (false, "[ ]")] {
                    let mut app = archive_complete_app(
                        AppExecutionMode::Running,
                        with_logs,
                        status,
                        marked,
                        false,
                    );
                    let buffer = render_buffer(&mut app, 120, 30);
                    let (checkbox, _) =
                        checkbox_and_id_column(&change_row_cells(&buffer, "change-a"), "change-a");

                    assert_eq!(
                        checkbox, expected,
                        "{layout}/{status} (marked={marked}) has no archive record and \
                         must still report its own mark"
                    );
                }
            }
        }
    }

    /// The status-only fallback: on a frame with no reducer archive snapshot, a
    /// terminal post-archive display status still suppresses the checkbox.
    #[test]
    fn tui_change_row_layout_mark_contract_terminal_status_alone_hides_the_checkbox() {
        for status in ["archived", "merged", "pushed"] {
            for marked in [true, false] {
                let (text, color) = get_checkbox_display(status, marked, false);
                assert_eq!(
                    text, CHECKBOX_PLACEHOLDER,
                    "`{status}` must suppress the checkbox without an archive snapshot"
                );
                assert_eq!(color, Color::DarkGray);
            }
        }
    }

    /// An archive-complete cursor row offers no Space mark hint, while `K: kill`
    /// stays available for the post-archive work that is still running.
    #[test]
    fn tui_change_row_layout_mark_contract_archive_complete_row_omits_the_mark_hint() {
        for (layout, with_logs) in [("select", false), ("running", true)] {
            for status in ARCHIVE_COMPLETE_ROW_STATUSES {
                for marked in [true, false] {
                    let mut app = archive_complete_app(
                        AppExecutionMode::Running,
                        with_logs,
                        status,
                        marked,
                        true,
                    );
                    let content = buffer_to_string(&render_buffer(&mut app, 200, 24));

                    assert!(
                        !content.contains("Space: mark") && !content.contains("Space: unmark"),
                        "{layout}/{status} (marked={marked}): an archive-complete row must \
                         advertise no mark control:\n{content}"
                    );
                    if status == "resolving" {
                        assert!(
                            content.contains("K: kill"),
                            "{layout}/{status}: per-change termination is independent of \
                             markability:\n{content}"
                        );
                    }
                }
            }
        }
    }

    /// The paired control for the hint: no archive record, so the mark hint is
    /// offered exactly as before.
    #[test]
    fn tui_change_row_layout_mark_contract_row_without_an_archive_record_keeps_the_mark_hint() {
        for with_logs in [false, true] {
            for status in ["resolving", "resolve pending", "merge wait"] {
                for (marked, expected) in [(false, "Space: mark"), (true, "Space: unmark")] {
                    let mut app = archive_complete_app(
                        AppExecutionMode::Running,
                        with_logs,
                        status,
                        marked,
                        false,
                    );
                    let content = buffer_to_string(&render_buffer(&mut app, 200, 24));

                    assert!(
                        content.contains(expected),
                        "{status} (marked={marked}) must still advertise `{expected}`:\n{content}"
                    );
                    assert!(
                        content.contains("x: toggle all"),
                        "{status}: a markable row keeps the bulk hint:\n{content}"
                    );
                }
            }
        }
    }

    // ========================================================================
    // Changes row layout: highlight-only focus, fixed 36-column ID field
    //
    // The row prefix is a column contract, so these assert columns rather than
    // substrings. `render_change_id_field` is the only thing allowed to decide
    // where the ID field ends, and both list layouts have to agree with it.
    // ========================================================================

    /// The two representative IDs from the change: one shorter than the content
    /// field and one longer than it.
    const SHORT_REPRESENTATIVE_ID: &str = "fix-stale-resolve-terminal-status";
    const LONG_REPRESENTATIVE_ID: &str = "preserve-archiving-during-tui-refresh";
    /// The long ID after hard truncation, with no ellipsis.
    const LONG_REPRESENTATIVE_ID_TRUNCATED: &str = "preserve-archiving-during-tui-refre";

    /// Unit: the field constants describe one 36-column boundary.
    #[test]
    fn tui_change_row_layout_render_field_constants_are_coherent() {
        assert_eq!(CHANGE_ID_CONTENT_WIDTH, 35);
        assert_eq!(CHECKBOX_TO_ID_SEPARATOR_WIDTH, 1);
        assert_eq!(CHANGE_ID_FIELD_WIDTH, 36);
        assert_eq!(
            CHANGE_ROW_PREFIX_WIDTH,
            CHECKBOX_WIDTH + CHECKBOX_TO_ID_SEPARATOR_WIDTH + CHANGE_ID_CONTENT_WIDTH,
            "the preview base width must be built from the same constants the row is"
        );
    }

    /// Unit: short IDs pad, long IDs hard-truncate, and both end up exactly
    /// [`CHANGE_ID_CONTENT_WIDTH`] display columns wide.
    #[test]
    fn tui_change_row_layout_render_id_field_pads_and_truncates_ascii() {
        let short = render_change_id_field(SHORT_REPRESENTATIVE_ID);
        assert_eq!(display_width(SHORT_REPRESENTATIVE_ID), 33);
        assert_eq!(short, format!("{SHORT_REPRESENTATIVE_ID}  "));
        assert_eq!(display_width(&short), CHANGE_ID_CONTENT_WIDTH);

        let long = render_change_id_field(LONG_REPRESENTATIVE_ID);
        assert_eq!(long, LONG_REPRESENTATIVE_ID_TRUNCATED);
        assert_eq!(display_width(&long), CHANGE_ID_CONTENT_WIDTH);
        assert!(
            !long.contains('…') && !long.contains("..."),
            "hard truncation adds no suffix: {long}"
        );

        // An ID that lands exactly on the boundary is neither padded nor cut.
        let exact = "a".repeat(CHANGE_ID_CONTENT_WIDTH);
        assert_eq!(render_change_id_field(&exact), exact);

        assert_eq!(
            display_width(&render_change_id_field("")),
            CHANGE_ID_CONTENT_WIDTH,
            "even an empty ID owns the whole field"
        );
    }

    /// Unit: the field is measured in terminal columns, so a wide character is
    /// never split and the remainder is padded instead.
    #[test]
    fn tui_change_row_layout_render_id_field_uses_unicode_display_width() {
        // 18 wide characters = 36 columns, so the 18th straddles the boundary:
        // 17 of them fill 34 columns and the last column becomes a space.
        let wide = "変".repeat(18);
        let field = render_change_id_field(&wide);
        assert_eq!(field.chars().filter(|c| *c == '変').count(), 17);
        assert_eq!(
            display_width(&field),
            CHANGE_ID_CONTENT_WIDTH,
            "the straddling character is dropped whole and its column padded"
        );
        assert!(field.ends_with(' '));

        // A mixed ID is cut at the same column, not at the same character count.
        let mixed = format!("{}変変変変変", "ascii-".repeat(4)); // 24 + 10 columns
        let mixed_field = render_change_id_field(&mixed);
        assert_eq!(display_width(&mixed_field), CHANGE_ID_CONTENT_WIDTH);
        assert!(mixed_field.starts_with("ascii-ascii-ascii-ascii-"));
    }

    /// One row per representative ID, arranged for either list layout.
    fn representative_row_app(with_logs: bool, cursor_index: usize) -> AppState {
        let mut app = create_test_app(vec![
            create_test_change(SHORT_REPRESENTATIVE_ID),
            create_test_change(LONG_REPRESENTATIVE_ID),
        ]);
        app.execution_mode = if with_logs {
            AppExecutionMode::Running
        } else {
            AppExecutionMode::Select
        };
        app.cursor_index = cursor_index;
        if with_logs {
            app.add_log(LogEntry::info("log"));
        }
        app
    }

    /// The rendered row containing `needle`, as one string.
    fn rendered_row(buffer: &Buffer, needle: &str) -> String {
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .find(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no rendered row contains `{needle}`"))
    }

    /// The regression the cursor column used to cause: no row may render `►`,
    /// and the focused row is still identifiable from its highlight alone.
    #[test]
    fn tui_change_row_layout_render_drops_the_cursor_glyph_and_keeps_the_highlight() {
        for with_logs in [false, true] {
            for mode in [
                AppExecutionMode::Select,
                AppExecutionMode::Running,
                AppExecutionMode::Stopping,
                AppExecutionMode::Stopped,
                AppExecutionMode::Error,
            ] {
                for cursor_index in [0, 1] {
                    let mut app = representative_row_app(with_logs, cursor_index);
                    app.execution_mode = mode;

                    let buffer = render_buffer(&mut app, 160, 30);
                    let content = buffer_to_string(&buffer);
                    assert!(
                        !content.contains('►'),
                        "{mode:?}/logs={with_logs}/cursor={cursor_index}: \
                         no row may render a cursor glyph:\n{content}"
                    );

                    // Focus is the row highlight: the focused row's ID column
                    // carries the highlight background, the other row does not.
                    let focused_id = if cursor_index == 0 {
                        SHORT_REPRESENTATIVE_ID
                    } else {
                        LONG_REPRESENTATIVE_ID_TRUNCATED
                    };
                    let other_id = if cursor_index == 0 {
                        LONG_REPRESENTATIVE_ID_TRUNCATED
                    } else {
                        SHORT_REPRESENTATIVE_ID
                    };
                    assert_eq!(
                        bg_at(&buffer, focused_id),
                        Color::DarkGray,
                        "{mode:?}/logs={with_logs}/cursor={cursor_index}: \
                         the focused row keeps its highlight"
                    );
                    assert_ne!(
                        bg_at(&buffer, other_id),
                        Color::DarkGray,
                        "{mode:?}/logs={with_logs}/cursor={cursor_index}: \
                         only the focused row is highlighted"
                    );
                }
            }
        }
    }

    /// Background color of the first cell of `needle` in the rendered buffer.
    fn bg_at(buffer: &Buffer, needle: &str) -> Color {
        for y in 0..buffer.area.height {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            if let Some(byte_index) = line.find(needle) {
                let column = line[..byte_index].chars().count() as u16;
                return buffer[(column, y)].bg;
            }
        }
        panic!("{needle:?} not found in rendered buffer");
    }

    /// The user's two representative rows, asserted as exact rendered text.
    ///
    /// This is the alignment claim itself: the short ID pads, the long one is cut
    /// at 35 columns with no ellipsis, and `WT`, the spinner, elapsed time,
    /// status, and task progress therefore start in the same column on both rows.
    #[test]
    fn tui_change_row_layout_render_representative_rows_align_every_field() {
        // Running layout, because that is the one that also carries the spinner
        // and elapsed-time fields.
        let mut app = representative_row_app(true, 0);
        for change in &mut app.changes {
            change.set_display_status_cache("applying");
            change.has_worktree = true;
            change.iteration_number = Some(2);
            change.elapsed_time = Some(Duration::from_secs(75));
            change.completed_tasks = 1;
            change.total_tasks = 4;
        }
        app.spinner_frame = 0;
        let spinner = SPINNER_CHARS[0];

        let buffer = render_buffer(&mut app, 120, 30);

        let short_row = rendered_row(&buffer, SHORT_REPRESENTATIVE_ID);
        let long_row = rendered_row(&buffer, LONG_REPRESENTATIVE_ID_TRUNCATED);

        // ` WT`, then ` {spinner}{elapsed:>7} `, then the status badge, then task
        // progress — every one of them supplying its own leading separator.
        let expected_tail = format!(" WT {spinner} 1m 15s [applying:2]  1/4(25%)");
        assert!(
            // 33 columns of ID plus two columns of padding.
            short_row.contains(&format!("[ ] {SHORT_REPRESENTATIVE_ID}  {expected_tail}")),
            "short row must pad the ID to the field width before `WT`:\n\
             {short_row}\nexpected tail: {expected_tail}"
        );
        assert!(
            // 35 columns of hard-truncated ID and no padding at all.
            long_row.contains(&format!(
                "[ ] {LONG_REPRESENTATIVE_ID_TRUNCATED}{expected_tail}"
            )),
            "long row must hard-truncate at the field width:\n\
             {long_row}\nexpected tail: {expected_tail}"
        );
        assert!(
            !long_row.contains('…'),
            "no truncation suffix is rendered:\n{long_row}"
        );

        // Every field after the ID starts in the same column on both rows.
        for field in [" WT", "[applying:2]", "1/4(25%)"] {
            assert_eq!(
                short_row
                    .find(field)
                    .map(|i| short_row[..i].chars().count()),
                long_row.find(field).map(|i| long_row[..i].chars().count()),
                "`{field}` must start in the same column on both rows:\n\
                 {short_row}\n{long_row}"
            );
        }
    }

    /// Both layouts start the ID exactly one column after the checkbox area, for
    /// short IDs, truncated IDs, and wide-character IDs alike.
    #[test]
    fn tui_change_row_layout_render_id_starts_one_column_after_the_checkbox() {
        for with_logs in [false, true] {
            for cursor_index in [0, 1] {
                let mut app = representative_row_app(with_logs, cursor_index);
                let buffer = render_buffer(&mut app, 160, 30);

                let (_, short_column) = checkbox_and_id_column(
                    &change_row_cells(&buffer, SHORT_REPRESENTATIVE_ID),
                    SHORT_REPRESENTATIVE_ID,
                );
                let (_, long_column) = checkbox_and_id_column(
                    &change_row_cells(&buffer, LONG_REPRESENTATIVE_ID_TRUNCATED),
                    LONG_REPRESENTATIVE_ID_TRUNCATED,
                );
                assert_eq!(short_column, CHANGE_ID_X as usize);
                assert_eq!(long_column, CHANGE_ID_X as usize);
            }
        }
    }

    /// A CJK ID leaves the following field in the same column an ASCII ID does.
    #[test]
    fn tui_change_row_layout_render_wide_character_id_preserves_following_columns() {
        for with_logs in [false, true] {
            let mut app = create_test_app(vec![
                create_test_change(SHORT_REPRESENTATIVE_ID),
                // 18 wide characters: the last one straddles the 35-column
                // boundary and must be dropped rather than split.
                create_test_change(&"変".repeat(18)),
            ]);
            app.execution_mode = if with_logs {
                AppExecutionMode::Running
            } else {
                AppExecutionMode::Select
            };
            for change in &mut app.changes {
                change.has_worktree = true;
            }
            if with_logs {
                app.add_log(LogEntry::info("log"));
            }

            let buffer = render_buffer(&mut app, 160, 30);
            let ascii_row = rendered_row(&buffer, SHORT_REPRESENTATIVE_ID);
            // A wide cell is followed by a blank spacer cell in the buffer, so
            // consecutive wide characters are not adjacent in the joined row.
            let wide_row = rendered_row(&buffer, "変");

            let wide_cells: Vec<char> = wide_row.chars().collect();
            assert_eq!(
                wide_cells.iter().filter(|c| **c == '変').count(),
                17,
                "logs={with_logs}: the straddling wide character is dropped whole:\n{wide_row}"
            );

            // `WT` is the first field after the ID, and a display-width-padded
            // field puts it in the same column on both rows.
            //
            // Column, not byte offset: a wide character is one cell pair in the
            // buffer but three bytes in the string.
            let column_of = |row: &str, needle: &str| {
                row.find(needle)
                    .map(|byte_index| row[..byte_index].chars().count())
                    .unwrap_or_else(|| panic!("`{needle}` missing from `{row}`"))
            };
            assert_eq!(
                column_of(&wide_row, " WT"),
                column_of(&ascii_row, " WT"),
                "logs={with_logs}: a wide-character ID must not shift the next field:\n\
                 {ascii_row}\n{wide_row}"
            );
        }
    }

    /// The widened fixed prefix must degrade by dropping the preview, never by
    /// letting it overlap a truncated field.
    #[test]
    fn tui_change_row_layout_render_narrow_terminal_omits_preview_safely() {
        for with_logs in [false, true] {
            let mut app = create_test_app(vec![create_test_change(LONG_REPRESENTATIVE_ID)]);
            app.execution_mode = if with_logs {
                AppExecutionMode::Running
            } else {
                AppExecutionMode::Select
            };
            app.changes[0].set_error_message_cache("a diagnostic nobody has room for".to_string());
            if with_logs {
                app.add_log(LogEntry::info("log"));
            }

            // Wide enough for the fixed fields, too narrow to leave
            // `MIN_PREVIEW_WIDTH` beside them.
            let buffer = render_buffer(&mut app, 76, 20);
            let content = buffer_to_string(&buffer);

            assert!(
                content.contains(LONG_REPRESENTATIVE_ID_TRUNCATED),
                "logs={with_logs}: the fixed ID field is still rendered in full:\n{content}"
            );
            assert!(
                !content.contains("Error: a diagnostic"),
                "logs={with_logs}: the preview is omitted rather than overlapped:\n{content}"
            );
        }
    }

    // ========================================================================
    // Key hints under the pure-mark contract
    // ========================================================================

    /// The cursor row's mark hint follows the mark, in every lifecycle mode.
    ///
    /// Rendered through the real widget tree rather than by calling the hint
    /// builder, so the assertion covers the routing that picks a layout as well
    /// as the hint text itself.
    #[test]
    fn run_mark_intent_hint_follows_mark_state_in_every_mode() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            for status in [
                "not queued",
                "queued",
                "applying",
                "error",
                "merge wait",
                "stalled",
            ] {
                for marked in [false, true] {
                    let mut app = create_test_app(vec![create_test_change("change-a")]);
                    app.execution_mode = mode;
                    app.changes[0].display_status_cache = status.to_string();
                    app.changes[0].selected = marked;
                    app.cursor_index = 0;

                    let content = buffer_to_string(&render_buffer(&mut app, 120, 24));
                    let expected = if marked {
                        "Space: unmark"
                    } else {
                        "Space: mark"
                    };
                    assert!(
                        content.contains(expected),
                        "{mode:?}/{status}/marked={marked}: expected `{expected}`:\n{content}"
                    );
                }
            }
        }
    }

    /// An active row advertises kill *and* mark: two independent controls.
    #[test]
    fn run_mark_intent_active_row_keeps_kill_alongside_the_mark_hint() {
        for status in [
            "preparing",
            "applying",
            "accepting",
            "archiving",
            "resolving",
        ] {
            let mut app = create_test_app(vec![create_test_change("change-a")]);
            app.execution_mode = AppExecutionMode::Running;
            app.changes[0].display_status_cache = status.to_string();
            app.cursor_index = 0;

            let content = buffer_to_string(&render_buffer(&mut app, 120, 24));
            assert!(
                content.contains("K: kill"),
                "{status}: per-change termination stays its own control:\n{content}"
            );
            assert!(
                content.contains("Space: mark"),
                "{status}: and mark intent stays available beside it:\n{content}"
            );
            assert!(
                !content.contains("Space: stop"),
                "{status}: Space must never be described as a stop:\n{content}"
            );
        }
    }

    /// A terminal row advertises no mark hint at all.
    #[test]
    fn run_mark_intent_terminal_row_omits_the_mark_hint() {
        for status in ["archived", "merged", "pushed", "rejected"] {
            for marked in [false, true] {
                let mut app = create_test_app(vec![create_test_change("change-a")]);
                app.changes[0].display_status_cache = status.to_string();
                app.changes[0].selected = marked;
                app.cursor_index = 0;

                let content = buffer_to_string(&render_buffer(&mut app, 120, 24));
                assert!(
                    !content.contains("Space: mark") && !content.contains("Space: unmark"),
                    "{status}/marked={marked}: a terminal row has no mark to advertise:\n{content}"
                );
            }
        }
    }

    /// The bulk hint depends on the visible target set, never on the mode.
    #[test]
    fn run_mark_intent_bulk_hint_depends_on_targets_not_mode() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            let mut app = create_test_app(vec![
                create_test_change("terminal"),
                create_test_change("live"),
            ]);
            app.execution_mode = mode;
            app.changes[0].display_status_cache = "archived".to_string();
            app.changes[1].display_status_cache = "applying".to_string();

            let content = buffer_to_string(&render_buffer(&mut app, 120, 24));
            assert!(
                content.contains("x: toggle all"),
                "{mode:?}: one non-terminal row is enough to offer the bulk toggle:\n{content}"
            );

            // Take the only non-terminal row away and the hint goes with it.
            app.changes[1].display_status_cache = "merged".to_string();
            let content = buffer_to_string(&render_buffer(&mut app, 120, 24));
            assert!(
                !content.contains("x: toggle all"),
                "{mode:?}: a terminal-only list offers no bulk toggle:\n{content}"
            );
        }
    }

    #[test]
    fn test_get_checkbox_display_not_selected() {
        let (text, color) = get_checkbox_display("not queued", false, false);
        assert_eq!(text, "[ ]");
        assert_eq!(color, Color::Gray);
    }

    #[test]
    fn test_get_checkbox_display_selected() {
        let (text, color) = get_checkbox_display("not queued", true, false);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);

        let (text, color) = get_checkbox_display("queued", true, false);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_get_checkbox_display_marked_not_queued() {
        // When selected but not queued, show [@] marker
        let (text, color) = get_checkbox_display("not queued", true, false);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_get_checkbox_display_processing_states() {
        // Applying state should show green when selected
        let (text, color) = get_checkbox_display("applying", true, false);
        assert_eq!(text, "[x]");
        assert_eq!(color, Color::Green);

        // Archiving state should show green when selected
        let (text, color) = get_checkbox_display("archiving", true, false);
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
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].has_worktree = true;

        let buffer = render_buffer(&mut app, 80, 20);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("WT"));
    }

    #[test]
    fn test_render_hides_new_badge_for_rejected_row_in_select_mode() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Select;
        app.changes[0].display_status_cache = "rejected".to_string();
        app.changes[0].is_new = true;

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("[rejected]"));
        assert!(
            !content.contains(" NEW"),
            "rejected row must never render NEW badge in Select mode"
        );
    }

    #[test]
    fn test_render_hides_new_badge_for_rejected_row_in_running_mode() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "rejected".to_string();
        app.changes[0].is_new = true;
        app.add_log(LogEntry::info("log"));

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("rejected"));
        assert!(
            !content.contains(" NEW"),
            "rejected row must never render NEW badge in Running mode"
        );
    }

    #[test]
    fn test_render_resolving_status_shows_label() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].display_status_cache = "resolving".to_string();
        app.add_log(LogEntry::info("log"));

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("resolving"));
    }

    #[test]
    fn test_render_merge_wait_status_shows_label() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.add_log(LogEntry::info("log"));

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("merge wait"));
    }

    #[test]
    fn test_render_merge_wait_shows_resolve_key_hint() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.clear_resolving(); // Not currently resolving

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("M: resolve"),
            "Should show M key hint for MergeWait status"
        );
    }

    #[test]
    fn test_render_merge_wait_shows_queue_resolve_key_hint_when_resolving() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.set_resolving("__active__"); // Currently resolving

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("M: queue resolve"),
            "Should show M queue-resolve intent hint while resolve is in progress"
        );
    }

    #[test]
    fn test_render_keeps_start_run_hint_while_resolving() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Select;
        app.set_resolving("__active__");
        app.cursor_index = 0;
        app.changes[0].selected = true;

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("F5/!: run"));

        let mut configured_app = create_test_app(vec![create_test_change("change-a")]);
        configured_app.set_tui_config(
            TuiConfig::parse_jsonc(
                r#"{"keybindings":{"start":["F5","!"]}}"#,
                std::path::Path::new("/tmp/tui.jsonc"),
            )
            .unwrap(),
        );
        configured_app.execution_mode = AppExecutionMode::Select;
        configured_app.set_resolving("__active__");
        configured_app.cursor_index = 0;
        configured_app.changes[0].selected = true;

        let buffer = render_buffer(&mut configured_app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("F5/!: run"));
    }

    // === Tests for update-tui-error-mode-continuation ===

    #[test]
    fn test_render_uses_centralized_resolve_check_in_select_mode() {
        // Verify that render shows M: resolve in Select mode with MergeWait
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Select;
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.clear_resolving();
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
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Error; // Error mode
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.clear_resolving();
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
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.clear_resolving();
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
        // - When resolve is NOT running and display_status_cache is MergeWait: "M: resolve"
        // - When resolve IS running and display_status_cache is MergeWait: "M: queue resolve"
        let test_cases = vec![
            // (mode, display_status_cache, is_resolving, should_show_resolve, should_show_queue_resolve)
            (
                AppExecutionMode::Select,
                "merge wait".to_string(),
                false,
                true,
                false,
            ),
            (
                AppExecutionMode::Select,
                "merge wait".to_string(),
                true,
                false,
                true,
            ),
            (
                AppExecutionMode::Running,
                "merge wait".to_string(),
                false,
                true,
                false,
            ),
            (
                AppExecutionMode::Running,
                "merge wait".to_string(),
                true,
                false,
                true,
            ),
            (
                AppExecutionMode::Error,
                "merge wait".to_string(),
                false,
                false,
                false,
            ),
            (
                AppExecutionMode::Select,
                "queued".to_string(),
                false,
                false,
                false,
            ),
        ];

        for (
            mode,
            display_status_cache,
            is_resolving,
            should_show_resolve,
            should_show_queue_resolve,
        ) in test_cases
        {
            let mut app = create_test_app(vec![create_test_change("change-a")]);
            app.execution_mode = mode;
            app.changes[0].display_status_cache = display_status_cache.clone();
            if is_resolving {
                app.set_resolving("__active__");
            }
            app.cursor_index = 0;
            if mode != AppExecutionMode::Select {
                app.add_log(LogEntry::info("log")); // Ensure logs exist for running mode
            }

            let buffer = render_buffer(&mut app, 100, 24);
            let content = buffer_to_string(&buffer);
            let shows_resolve = content.contains("M: resolve");
            let shows_queue_resolve = content.contains("M: queue resolve");

            assert_eq!(
                shows_resolve, should_show_resolve,
                "Render 'M: resolve' hint mismatch for mode={:?}, display_status_cache={:?}, is_resolving={}",
                mode, display_status_cache, is_resolving
            );
            assert_eq!(
                shows_queue_resolve, should_show_queue_resolve,
                "Render 'M: queue resolve' hint mismatch for mode={:?}, display_status_cache={:?}, is_resolving={}",
                mode, display_status_cache, is_resolving
            );
        }
    }

    #[test]
    fn test_render_shows_worktree_delete_confirm_modal() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.modal = Some(ModalState::ConfirmWorktreeDelete {
            path: std::path::PathBuf::from("/path/to/worktree"),
            branch: "feature-a".to_string(),
        });

        let buffer = render_buffer(&mut app, 80, 20);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Delete Worktree"));
        assert!(content.contains("/path/to/worktree"));
    }

    #[test]
    fn test_render_parallel_archived_row_does_not_show_uncommitted_badge() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].display_status_cache = "archived".to_string();
        app.changes[0].parallel_eligibility = ParallelEligibility::UncommittedProposalFiles;

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(!content.contains("UNCOMMITTED"));
        // The row is still rendered — it just carries no checkbox any more.
        let (checkbox, _) =
            checkbox_and_id_column(&change_row_cells(&buffer, "change-a"), "change-a");
        assert_eq!(checkbox, CHECKBOX_PLACEHOLDER);
    }

    #[test]
    fn test_render_parallel_uncommitted_queueable_row_shows_uncommitted_badge() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].display_status_cache = "not queued".to_string();
        app.changes[0].parallel_eligibility = ParallelEligibility::UncommittedProposalFiles;

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("UNCOMMITTED"));
    }

    #[test]
    fn the_badge_uses_the_correct_spelling_and_reserves_its_own_width() {
        assert_eq!(UNCOMMITTED_BADGE, " UNCOMMITTED");
        assert_eq!(
            UNCOMMITTED_BADGE.len(),
            12,
            "the reserved width is taken from the badge itself, so the two cannot drift"
        );
    }

    // ========================================================================
    // Parallel-ineligibility reasons are rendered apart
    //
    // Every case below is parallel-ineligible and must stay non-actionable.
    // What differs is whether the row may claim a Git working-tree condition.
    // ========================================================================

    /// One parallel-ineligible row, arranged for both list layouts.
    fn parallel_ineligible_app(
        mode: AppExecutionMode,
        display_status: &str,
        eligibility: ParallelEligibility,
        has_worktree: bool,
    ) -> AppState {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = mode;
        app.changes[0].display_status_cache = display_status.to_string();
        app.changes[0].parallel_eligibility = eligibility;
        app.changes[0].has_worktree = has_worktree;
        app.cursor_index = 0;
        app
    }

    #[test]
    fn a_clean_proposal_absent_from_head_is_blocked_without_claiming_dirty_state() {
        for mode in [AppExecutionMode::Select, AppExecutionMode::Running] {
            for status in ["not queued", "queued"] {
                let mut app = parallel_ineligible_app(
                    mode,
                    status,
                    ParallelEligibility::ProposalAbsentFromHead,
                    false,
                );

                let buffer = render_buffer(&mut app, 120, 24);
                let content = buffer_to_string(&buffer);

                assert!(
                    !content.contains("UNCOMMITTED"),
                    "{mode:?}/{status}: an absent proposal has no uncommitted files to report"
                );
                assert!(
                    !content.contains("Space: queue") && !content.contains("Space: unqueue"),
                    "{mode:?}/{status}: Space is never described as a queue control"
                );
                assert!(
                    content.contains("[ ]"),
                    "{mode:?}/{status}: an unmarked non-terminal row still renders its checkbox"
                );
                assert_eq!(
                    buffer
                        .cell((CHANGE_ID_X, SELECT_FIRST_ROW_Y))
                        .unwrap()
                        .style()
                        .fg,
                    Some(Color::Gray),
                    "{mode:?}/{status}: the focused blocked row stays grayed out"
                );

                // Eligibility is still observed and still refuses a *run*; it no
                // longer refuses the intent. Space marks the row silently and
                // the start key is where the condition is reported.
                assert!(!app.changes[0].is_parallel_eligible());
                app.toggle_selection();
                assert!(
                    app.changes[0].selected,
                    "{mode:?}/{status}: a worktree-ineligible row still accepts mark intent"
                );
                assert!(
                    app.warning_message.is_none(),
                    "{mode:?}/{status}: marking refuses nothing: {:?}",
                    app.warning_message
                );
            }
        }
    }

    /// An archived or failed-merge change whose managed worktree is still around
    /// keeps `WT` but is not dirty: the worktree marker and the badge are
    /// independent observations.
    #[test]
    fn a_retained_clean_worktree_keeps_wt_without_the_uncommitted_badge() {
        for mode in [AppExecutionMode::Select, AppExecutionMode::Running] {
            for status in ["not queued", "queued"] {
                let mut app = parallel_ineligible_app(
                    mode,
                    status,
                    ParallelEligibility::ProposalAbsentFromHead,
                    true,
                );

                let buffer = render_buffer(&mut app, 120, 24);
                let content = buffer_to_string(&buffer);

                assert!(
                    content.contains("WT"),
                    "{mode:?}/{status}: a retained worktree is still reported"
                );
                assert!(
                    !content.contains("UNCOMMITTED"),
                    "{mode:?}/{status}: a retained clean worktree is not dirty proposal content"
                );
                assert!(
                    content.contains("[ ]"),
                    "{mode:?}/{status}: the row stays non-actionable"
                );
                assert!(!app.changes[0].is_parallel_eligible());
            }
        }
    }

    #[test]
    fn a_dirty_proposal_is_the_only_row_that_claims_uncommitted_state() {
        for mode in [AppExecutionMode::Select, AppExecutionMode::Running] {
            for status in ["not queued", "queued"] {
                let mut app = parallel_ineligible_app(
                    mode,
                    status,
                    ParallelEligibility::UncommittedProposalFiles,
                    false,
                );

                let buffer = render_buffer(&mut app, 120, 24);
                let content = buffer_to_string(&buffer);

                assert!(
                    content.contains("UNCOMMITTED"),
                    "{mode:?}/{status}: observed dirty proposal files must be reported"
                );
                assert!(
                    content.contains("[ ]") && !content.contains("Space: queue"),
                    "{mode:?}/{status}: the dirty row renders a checkbox, not a queue affordance"
                );

                // Same rule as the absent-proposal row: the badge reports the
                // observation, the mark is accepted, and start admission owns
                // the refusal.
                app.toggle_selection();
                assert!(
                    app.changes[0].selected,
                    "{mode:?}/{status}: a dirty row still accepts mark intent"
                );
                assert!(
                    app.warning_message.is_none(),
                    "{mode:?}/{status}: marking refuses nothing: {:?}",
                    app.warning_message
                );
            }
        }
    }

    // Select-mode layout: header=3 rows, list starts at y=3 (border), first item at y=4.
    // List left border is at x=0, spans start at x=1.
    // Checkbox "[ ]" = 3 columns plus one separator column → display_id starts at x=5.
    // There is no cursor column: focus is the row highlight alone.
    const SELECT_FIRST_ROW_Y: u16 = 4;
    const CHANGE_ID_X: u16 = 1 + CHECKBOX_WIDTH as u16 + CHECKBOX_TO_ID_SEPARATOR_WIDTH as u16;

    #[test]
    fn test_focused_blocked_row_has_readable_fg_select_mode() {
        // Focused blocked row should use Gray (not DarkGray) so it's readable on the
        // DarkGray highlight background.
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].display_status_cache = "not queued".to_string();
        app.changes[0].parallel_eligibility = ParallelEligibility::UncommittedProposalFiles;
        app.cursor_index = 0; // cursor on the blocked row

        let buffer = render_buffer(&mut app, 80, 24);
        let cell = buffer.cell((CHANGE_ID_X, SELECT_FIRST_ROW_Y)).unwrap();
        assert_eq!(
            cell.style().fg,
            Some(Color::Gray),
            "Focused blocked row name should use Gray fg for readability on DarkGray highlight"
        );
    }

    #[test]
    fn test_unfocused_blocked_row_remains_dimmed_select_mode() {
        // Unfocused blocked row should keep DarkGray to remain visually de-emphasized.
        let mut app = create_test_app(vec![
            create_test_change("change-a"),
            create_test_change("change-b"),
        ]);
        app.changes[0].display_status_cache = "not queued".to_string();
        app.changes[0].parallel_eligibility = ParallelEligibility::UncommittedProposalFiles;
        app.cursor_index = 1; // cursor on change-b, not on the blocked row

        let buffer = render_buffer(&mut app, 80, 24);
        let cell = buffer.cell((CHANGE_ID_X, SELECT_FIRST_ROW_Y)).unwrap();
        assert_eq!(
            cell.style().fg,
            Some(Color::DarkGray),
            "Unfocused blocked row name should stay DarkGray to remain de-emphasized"
        );
    }

    #[test]
    fn test_focused_blocked_row_has_readable_fg_running_mode() {
        // Same contrast rule applies in Running view.
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "not queued".to_string();
        app.changes[0].parallel_eligibility = ParallelEligibility::UncommittedProposalFiles;
        app.cursor_index = 0;

        let buffer = render_buffer(&mut app, 80, 24);
        let cell = buffer.cell((CHANGE_ID_X, SELECT_FIRST_ROW_Y)).unwrap();
        assert_eq!(
            cell.style().fg,
            Some(Color::Gray),
            "Focused blocked row name should use Gray fg in Running view too"
        );
    }

    #[test]
    fn test_unfocused_blocked_row_remains_dimmed_running_mode() {
        let mut app = create_test_app(vec![
            create_test_change("change-a"),
            create_test_change("change-b"),
        ]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "not queued".to_string();
        app.changes[0].parallel_eligibility = ParallelEligibility::UncommittedProposalFiles;
        app.cursor_index = 1;

        let buffer = render_buffer(&mut app, 80, 24);
        let cell = buffer.cell((CHANGE_ID_X, SELECT_FIRST_ROW_Y)).unwrap();
        assert_eq!(
            cell.style().fg,
            Some(Color::DarkGray),
            "Unfocused blocked row name should stay DarkGray in Running view"
        );
    }

    #[test]
    fn test_render_select_mode_footer_message() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        // Select the change to trigger start processing guidance.
        app.changes[0].selected = true;
        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Conflux"));
        assert!(content.contains("Press F5/! to start processing"));

        app.set_tui_config(
            TuiConfig::parse_jsonc(
                r#"{"keybindings":{"start":["F5","!"]}}"#,
                std::path::Path::new("/tmp/tui.jsonc"),
            )
            .unwrap(),
        );
        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Press F5/! to start processing"));
    }

    // ========================================================================
    // Settled Apply iteration limit: TUI guidance (integration — rendering)
    // ========================================================================
    //
    // These render the real widget tree over an in-memory buffer. No terminal,
    // process, or repository is involved. The retained diagnostic is evidence,
    // not an action gate, so every one of them asserts that a settled limited
    // row is presented exactly as an ordinary terminal-error row.

    /// One settled terminal-error row whose Apply ceiling refused the invocation.
    ///
    /// The retained diagnostic reaches the TUI as the row's error detail, which
    /// is the only place it lives now: there is no per-row limit cache to set.
    fn app_with_limited_error_row() -> AppState {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].set_error_message_cache(
            "reached maximum iterations (13/13) without completion".to_string(),
        );
        app
    }

    #[test]
    fn settled_iteration_limit_tui_row_hint_offers_the_ordinary_mark() {
        let mut app = app_with_limited_error_row();
        app.cursor_index = 0;

        let content = buffer_to_string(&render_buffer(&mut app, 120, 24));

        assert!(
            content.contains("Space: mark"),
            "a settled limited row is markable like any other error row:\n{content}"
        );
        assert!(
            !content.contains("Apply limit reached"),
            "and no retired active-limit condition replaces that guidance:\n{content}"
        );
        assert!(
            content.contains("Enter: details"),
            "the retained diagnostic stays inspectable:\n{content}"
        );
    }

    #[test]
    fn settled_iteration_limit_tui_select_footer_gives_ordinary_retry_guidance() {
        let mut app = app_with_limited_error_row();

        let content = buffer_to_string(&render_buffer(&mut app, 120, 24));
        assert!(
            content.contains("Select changes with Space to process"),
            "the footer offers the same Space guidance as any error row:\n{content}"
        );
        assert!(
            content.contains("error rows need retry mark"),
            "including the ordinary error-row retry hint:\n{content}"
        );
        assert!(
            !content.contains("Apply iteration limit reached"),
            "and no stable active-limit condition replaces it:\n{content}"
        );
    }

    /// A settled limited row must not suppress guidance for unrelated rows.
    #[test]
    fn settled_iteration_limit_tui_select_footer_keeps_guidance_for_eligible_rows() {
        let mut app = create_test_app(vec![
            create_test_change("limited"),
            create_test_change("eligible"),
        ]);
        app.changes[0].set_error_message_cache(
            "reached maximum iterations (13/13) without completion".to_string(),
        );

        let content = buffer_to_string(&render_buffer(&mut app, 120, 24));
        assert!(
            content.contains("Select changes with Space to process"),
            "an eligible row keeps its ordinary guidance:\n{content}"
        );
    }

    #[test]
    fn settled_iteration_limit_tui_error_header_offers_the_retry_key() {
        let mut app = app_with_limited_error_row();
        app.execution_mode = AppExecutionMode::Error;
        app.error_change_id = Some("change-a".to_string());
        // The Status panel is part of the logs-bearing layout, which is what a
        // real Error-mode session always has by the time a change has failed.
        app.add_log(LogEntry::error("apply failed"));

        // The taller layout the other base-control tests use; at 24 rows the
        // Status panel is not laid out at all.
        let content = buffer_to_string(&render_buffer(&mut app, 120, 40));
        assert!(
            content.contains(&format!("{}: retry", app.start_key_label())),
            "the header offers the retry the shared service now admits:\n{content}"
        );
        assert!(
            !content.contains("Apply iteration limit reached"),
            "and states no retired active-limit condition:\n{content}"
        );
    }

    /// The retained diagnostic is what makes the failure inspectable, so it must
    /// still reach the operator through the row's error detail.
    #[test]
    fn settled_iteration_limit_tui_keeps_the_attempts_and_ceiling_visible() {
        let mut app = app_with_limited_error_row();
        app.cursor_index = 0;
        app.execution_mode = AppExecutionMode::Error;
        app.error_change_id = Some("change-a".to_string());
        app.add_log(LogEntry::error("apply failed"));

        let content = buffer_to_string(&render_buffer(&mut app, 120, 40));
        assert!(
            content.contains("13/13"),
            "the exact attempts/max evidence stays observable:\n{content}"
        );
    }

    #[test]
    fn test_render_status_uses_configured_start_label() {
        let mut stopped_app = create_test_app(vec![create_test_change("change-a")]);
        stopped_app.set_tui_config(
            TuiConfig::parse_jsonc(
                r#"{"keybindings":{"start":["F5","!"]}}"#,
                std::path::Path::new("/tmp/tui.jsonc"),
            )
            .unwrap(),
        );
        stopped_app.execution_mode = AppExecutionMode::Stopped;
        stopped_app.logs.push(LogEntry::info("show status"));
        let buffer = render_buffer(&mut stopped_app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("F5/!: resume"));

        let mut stopping_app = create_test_app(vec![create_test_change("change-a")]);
        stopping_app.set_tui_config(
            TuiConfig::parse_jsonc(
                r#"{"keybindings":{"start":["F5","!"]}}"#,
                std::path::Path::new("/tmp/tui.jsonc"),
            )
            .unwrap(),
        );
        stopping_app.execution_mode = AppExecutionMode::Stopping;
        stopping_app.logs.push(LogEntry::info("show status"));
        let buffer = render_buffer(&mut stopping_app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("F5/!: continue"));
    }

    // === Tests for fix-tui-overall-progress-scope ===

    /// One Status-aggregate input row: display status, execution mark, and the
    /// last known task counts.
    type ProgressRow = (&'static str, bool, u32, u32);

    /// Render only the Status panel, so the asserted text is the real widget
    /// output rather than a test-local recalculation of the same rule.
    fn render_status_panel(app: &AppState) -> String {
        let backend = TestBackend::new(100, 3);
        let mut terminal = Terminal::new(backend).expect("terminal init");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_status(frame, app, area);
            })
            .expect("draw");
        buffer_to_string(terminal.backend().buffer())
    }

    fn overall_progress_app(rows: &[ProgressRow]) -> AppState {
        let changes = (0..rows.len())
            .map(|i| create_test_change(&format!("change-{i}")))
            .collect();
        let mut app = create_test_app(changes);
        for (state, (status, marked, completed, total)) in app.changes.iter_mut().zip(rows) {
            state.display_status_cache = (*status).to_string();
            state.selected = *marked;
            state.completed_tasks = *completed;
            state.total_tasks = *total;
        }
        app
    }

    fn assert_status_aggregate(app: &AppState, expected: &str, context: &str) {
        let content = render_status_panel(app);
        assert!(
            content.contains(expected),
            "{context}: expected Status aggregate {expected} in:\n{content}"
        );
    }

    #[test]
    fn tui_status_overall_progress_mixed_lifecycle_and_mark_states() {
        let app = overall_progress_app(&[
            ("merged", false, 3, 3),
            ("applying", false, 1, 4),
            ("not queued", true, 0, 2),
            ("not queued", false, 0, 5),
        ]);

        let content = render_status_panel(&app);
        assert!(
            content.contains("(4/9)"),
            "completed, active, and marked rows form the aggregate:\n{content}"
        );
        assert!(
            content.contains("44.4%"),
            "the percentage follows the same aggregate:\n{content}"
        );
    }

    #[test]
    fn tui_status_overall_progress_retains_completed_work_after_mark_revocation() {
        for final_success in ["archived", "merged", "pushed"] {
            let mut app =
                overall_progress_app(&[(final_success, true, 3, 3), ("applying", true, 1, 4)]);
            assert_status_aggregate(&app, "(4/7)", final_success);

            // Archive completion revokes the process-local mark by design.
            app.changes[0].selected = false;
            assert_status_aggregate(
                &app,
                "(4/7)",
                &format!("{final_success} after mark revocation"),
            );
        }
    }

    #[test]
    fn tui_status_overall_progress_includes_archive_complete_post_archive_rows() {
        for post_archive in ["resolving", "resolve pending", "merge wait"] {
            let mut app = overall_progress_app(&[(post_archive, false, 3, 3)]);
            app.changes[0].archive_complete_cache = true;

            assert_status_aggregate(&app, "(3/3)", post_archive);
        }
    }

    #[test]
    fn tui_status_overall_progress_includes_every_active_status_without_a_mark() {
        for status in crate::orchestration::operator_command::ACTIVE_STATUSES {
            let app = overall_progress_app(&[(status, false, 1, 4)]);

            assert_status_aggregate(&app, "(1/4)", status);
        }
    }

    #[test]
    fn tui_status_overall_progress_includes_marked_unfinished_rows() {
        for status in [
            "not queued",
            "queued",
            "merge wait",
            "resolve pending",
            "error",
        ] {
            let app = overall_progress_app(&[(status, true, 1, 2)]);

            assert_status_aggregate(&app, "(1/2)", status);
        }
    }

    #[test]
    fn tui_status_overall_progress_excludes_unmarked_idle_and_rejected_rows() {
        // An `error` row whose own failure revoked its mark, an idle unmarked
        // row, and a rejected row that stale presentation state still marks.
        let app = overall_progress_app(&[
            ("applying", false, 1, 4),
            ("error", false, 2, 6),
            ("not queued", false, 0, 5),
            ("rejected", true, 3, 3),
        ]);

        assert_status_aggregate(&app, "(1/4)", "only the active row contributes");
    }

    #[test]
    fn tui_status_overall_progress_counts_overlapping_rows_once() {
        let mut app = overall_progress_app(&[
            // active + marked
            ("applying", true, 1, 4),
            // completed + marked
            ("merged", true, 3, 3),
            // archive-complete + active + marked
            ("resolving", true, 2, 2),
        ]);
        app.changes[2].archive_complete_cache = true;

        assert_status_aggregate(&app, "(6/9)", "each overlapping row counts once");
    }

    #[test]
    fn tui_status_overall_progress_zero_total_rows_render_no_bar() {
        let app = overall_progress_app(&[
            ("merged", false, 0, 0),
            ("applying", false, 0, 0),
            ("not queued", true, 0, 0),
        ]);

        let content = render_status_panel(&app);
        assert!(
            !content.contains('%') && !content.contains('█') && !content.contains('░'),
            "included rows with no tasks keep the existing no-task rendering:\n{content}"
        );
        assert!(
            content.contains("Status"),
            "the Status panel still renders:\n{content}"
        );
    }

    #[test]
    fn test_render_shows_uncommitted_badge() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.apply_parallel_eligibility(
            &HashSet::from(["change-a".to_string()]),
            &HashSet::from(["change-a".to_string()]),
        );

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);
        assert!(content.contains("UNCOMMITTED"));
    }

    #[test]
    fn test_log_header_analysis_with_iteration() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);

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
        let mut app = create_test_app(vec![create_test_change("change-a")]);

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
        let mut app = create_test_app(vec![create_test_change("change-a")]);

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
        let mut app = create_test_app(vec![create_test_change("change-a")]);

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
        let mut app = create_test_app(vec![create_test_change("change-a")]);

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
        let mut app = create_test_app(vec![create_test_change("change-a")]);

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
        let mut app = create_test_app(vec![create_test_change("change-a")]);

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
        let mut app = create_test_app(vec![create_test_change("change-a")]);

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
            create_test_change("change-a"),
            create_test_change("change-b"),
            create_test_change("change-c"),
            create_test_change("change-d"),
        ]);

        // Set mode to Running
        app.execution_mode = AppExecutionMode::Running;

        // Set up different statuses:
        // - change-a: Queued (should NOT be counted)
        // - change-b: Applying (should be counted)
        // - change-c: Archiving (should be counted)
        // - change-d: NotQueued (should NOT be counted)
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[1].display_status_cache = "applying".to_string();
        app.changes[2].display_status_cache = "archiving".to_string();
        app.changes[3].display_status_cache = "not queued".to_string();

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
            create_test_change("change-a"),
            create_test_change("change-b"),
        ]);

        // Set mode to Running
        app.execution_mode = AppExecutionMode::Running;

        // Set one change to Resolving, one to Queued
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "queued".to_string();

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
    fn running_header_count_reflects_reducer_synced_active_status_after_refresh() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[0].selected = true;
        app.apply_display_statuses_from_reducer(&std::collections::HashMap::from([(
            "change-a".to_string(),
            "accepting",
        )]));

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("[Running 1]"),
            "header should count reducer-synced accepting row, but got:\n{}",
            content
        );
        assert!(
            !content.contains("[Running 2]"),
            "header should not count queued rows in addition to active row, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_select_mode_shows_ready_even_when_resolving_exists() {
        let mut app = create_test_app(vec![
            create_test_change("change-a"),
            create_test_change("change-b"),
        ]);

        app.execution_mode = AppExecutionMode::Select;
        app.changes[0].display_status_cache = "resolving".to_string();
        app.changes[1].display_status_cache = "queued".to_string();

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("[Ready]"),
            "Header should show 'Ready' in Select mode, but got:\n{}",
            content
        );
        assert!(
            !content.contains("[Running 1]"),
            "Header should not show '[Running 1]' in Select mode, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_running_mode_shows_running_without_count_when_no_in_flight() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);

        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "queued".to_string();

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("[Running]"),
            "Header should show '[Running]' in Running mode with zero in-flight, but got:\n{}",
            content
        );
        assert!(
            !content.contains("[Running 1]"),
            "Header should not show count when in-flight is zero, but got:\n{}",
            content
        );
        assert!(
            !content.contains("[Ready]"),
            "Header should not show '[Ready]' in Running mode, but got:\n{}",
            content
        );
    }

    #[test]
    fn test_stopping_mode_header_shows_stopping() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Stopping;

        let buffer = render_buffer(&mut app, 80, 24);
        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("[Stopping]"),
            "Header should show '[Stopping]' in Stopping mode, but got:\n{}",
            content
        );
        assert!(
            !content.contains("[Ready]"),
            "Header should not show '[Ready]' in Stopping mode, but got:\n{}",
            content
        );
    }

    #[test]
    fn stopped_mode_header_shows_ready_with_resume_controls() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Stopped;
        // The status panel only renders on the running-mode screen, which is
        // what carries the stopped-mode resume control.
        app.add_log(LogEntry::info("stop requested"));

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("[Ready]"),
            "Stopped mode must project the Ready header, but got:\n{}",
            content
        );
        assert_eq!(
            fg_at(&buffer, "[Ready]"),
            Color::Cyan,
            "the stopped Ready label must use the existing cyan Ready presentation"
        );
        assert!(
            content.contains(&format!("{}: resume", app.start_key_label())),
            "Stopped mode must keep its resume control alongside the Ready header, but got:\n{}",
            content
        );
        assert!(
            !content.contains("[Stopped]"),
            "the header must never expose a Stopped execution status, but got:\n{}",
            content
        );
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Stopped,
            "rendering must not mutate the internal execution mode"
        );
    }

    #[test]
    fn error_mode_header_remains_unlabeled_without_modal() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Error;
        app.modal = None;
        app.add_log(LogEntry::info("apply failed"));

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);

        assert!(
            !content.contains("[Ready]") && !content.contains("[Stopped]"),
            "Error mode without a modal must render no status label, but got:\n{}",
            content
        );
        assert!(
            content.contains(&format!("{}: retry", app.start_key_label())),
            "Error mode must keep its retry control, but got:\n{}",
            content
        );
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Error,
            "rendering must not mutate the internal execution mode"
        );
    }

    #[test]
    fn test_log_panel_toggle_hides_logs() {
        // Test that logs can be hidden when logs_panel_enabled is false
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.add_log(LogEntry::info("Test log message"));

        // Logs panel should be visible by default
        assert!(app.logs_panel_enabled);

        // Disable logs panel
        app.logs_panel_enabled = false;

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);

        // The log message should not be visible when panel is disabled
        assert!(
            !content.contains("Test log message"),
            "Log message should not be visible when logs panel is disabled"
        );

        // Status panel should still be visible
        assert!(
            content.contains("Status"),
            "Status panel should be visible even when logs are hidden"
        );
    }

    #[test]
    fn test_log_panel_toggle_shows_logs_when_enabled() {
        // Test that logs are shown when logs_panel_enabled is true
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.add_log(LogEntry::info("Test log message"));

        // Logs panel is enabled by default
        assert!(app.logs_panel_enabled);

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);

        // The log message should be visible
        assert!(
            content.contains("Test log message"),
            "Log message should be visible when logs panel is enabled"
        );
    }

    /// Extract only the Logs panel rows from a rendered frame.
    ///
    /// Change rows carry their own per-change log preview, so whole-buffer
    /// assertions cannot distinguish Logs-panel content from list previews.
    fn logs_panel_content(buffer: &Buffer) -> String {
        let title_row = find_row_containing(buffer, "f: filter").expect("logs panel title row");
        let mut lines = Vec::new();
        for y in title_row..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            lines.push(line);
        }
        lines.join("\n")
    }

    /// Changes-view app with alpha/beta proposal logs plus one unscoped entry.
    fn mixed_proposal_log_app() -> AppState {
        let mut app = create_test_app(vec![
            create_test_change("alpha"),
            create_test_change("beta"),
        ]);
        app.execution_mode = AppExecutionMode::Running;
        app.add_log(LogEntry::info("alpha-apply-output").with_change_id("alpha"));
        app.add_log(LogEntry::info("beta-apply-output").with_change_id("beta"));
        app.add_log(LogEntry::info("global-orchestration-output"));
        app
    }

    #[test]
    fn logs_panel_shows_all_entries_while_filter_is_disabled() {
        let mut app = mixed_proposal_log_app();

        let content = logs_panel_content(&render_buffer(&mut app, 140, 24));

        assert!(content.contains("alpha-apply-output"));
        assert!(content.contains("beta-apply-output"));
        assert!(content.contains("global-orchestration-output"));
    }

    #[test]
    fn logs_panel_shows_only_cursor_proposal_entries_while_filter_is_enabled() {
        let mut app = mixed_proposal_log_app();
        app.toggle_selected_proposal_log_filter();

        let content = logs_panel_content(&render_buffer(&mut app, 140, 24));

        assert!(content.contains("alpha-apply-output"));
        assert!(!content.contains("beta-apply-output"));
        assert!(!content.contains("global-orchestration-output"));
        // The buffer is untouched; only the visible set changed.
        assert_eq!(app.logs.len(), 3);
    }

    #[test]
    fn logs_panel_filter_follows_cursor_movement() {
        let mut app = mixed_proposal_log_app();
        app.toggle_selected_proposal_log_filter();
        app.cursor_down();

        let content = logs_panel_content(&render_buffer(&mut app, 140, 24));

        assert!(content.contains("beta-apply-output"));
        assert!(!content.contains("alpha-apply-output"));
    }

    #[test]
    fn logs_panel_filter_hides_global_only_buffers_without_panicking() {
        let mut app = create_test_app(vec![create_test_change("alpha")]);
        app.execution_mode = AppExecutionMode::Running;
        app.add_log(LogEntry::info("global-orchestration-output"));
        app.toggle_selected_proposal_log_filter();

        let content = logs_panel_content(&render_buffer(&mut app, 140, 24));

        assert!(!content.contains("global-orchestration-output"));
        assert!(content.contains("f: filter"));
        assert_eq!(app.logs.len(), 1);
    }

    #[test]
    fn logs_panel_filter_renders_zero_matches_from_the_filtered_line_counts() {
        let mut app = mixed_proposal_log_app();
        // Many unrelated entries would overflow the panel if counted.
        for index in 0..80 {
            app.add_log(LogEntry::info(format!("beta-noise-{index}")).with_change_id("beta"));
        }
        app.logs
            .retain(|entry| entry.change_id.as_deref() != Some("alpha"));
        app.toggle_selected_proposal_log_filter();
        app.scroll_logs_up(40);

        let content = logs_panel_content(&render_buffer(&mut app, 140, 24));

        assert!(!content.contains("beta-noise-"));
        assert!(!content.contains("global-orchestration-output"));
        // Ranges are computed from the empty filtered set, so no range is shown.
        assert!(!content.contains("Logs ["));
        assert!(content.contains("f: filter=alpha"));
    }

    #[test]
    fn logs_panel_filter_scroll_bounds_use_the_filtered_entry_set() {
        let mut app = create_test_app(vec![
            create_test_change("alpha"),
            create_test_change("beta"),
        ]);
        app.execution_mode = AppExecutionMode::Running;
        for index in 0..40 {
            app.add_log(LogEntry::info(format!("beta-noise-{index}")).with_change_id("beta"));
        }
        for index in 0..6 {
            app.add_log(LogEntry::info(format!("alpha-line-{index}")).with_change_id("alpha"));
        }
        app.toggle_selected_proposal_log_filter();

        let content = logs_panel_content(&render_buffer(&mut app, 140, 24));

        // Six matching single-line entries fit, so no scroll range is rendered
        // even though the unfiltered buffer is far larger than the panel.
        assert!(!content.contains("Logs ["));
        assert!(content.contains("alpha-line-0"));
        assert!(content.contains("alpha-line-5"));
        assert!(!content.contains("beta-noise-"));
    }

    #[test]
    fn logs_panel_buffer_shows_filter_key_and_state() {
        let mut app = mixed_proposal_log_app();

        let off_content = logs_panel_content(&render_buffer(&mut app, 140, 24));
        assert!(off_content.contains("f: filter off"));

        app.toggle_selected_proposal_log_filter();
        let on_content = logs_panel_content(&render_buffer(&mut app, 140, 24));
        assert!(on_content.contains("f: filter=alpha"));
        // Existing navigation guidance is not regressed by the new hint.
        assert!(on_content.contains("PgUp/PgDn"));
        assert!(on_content.contains("Home/End"));
        assert!(on_content.contains("l: hide"));
    }

    /// Title params for a panel that fits its content on screen.
    fn title_params(filter_enabled: bool, filter_target: Option<&str>) -> LogsPanelTitle<'_> {
        LogsPanelTitle {
            lines_below: 0,
            log_auto_scroll: true,
            total_display_lines: 4,
            visible_height: 10,
            start_line: 0,
            end_line: 4,
            filter_enabled,
            filter_target,
            panel_width: 200,
        }
    }

    #[test]
    fn logs_panel_title_preserves_position_and_shows_navigation_guidance() {
        let title = logs_panel_title(LogsPanelTitle {
            lines_below: 12,
            log_auto_scroll: false,
            total_display_lines: 42,
            visible_height: 10,
            start_line: 20,
            end_line: 30,
            filter_enabled: false,
            filter_target: Some("alpha"),
            panel_width: 200,
        });

        assert!(title.contains("Logs [21-30/42 lines]"));
        assert!(title.contains("lines_below=12"));
        assert!(title.contains("⏸"));
        assert!(title.contains("PgUp/PgDn: older/newer"));
        assert!(title.contains("Home/End: oldest/newest"));
        assert!(title.contains("l: hide"));
    }

    #[test]
    fn logs_panel_title_reports_disabled_selected_proposal_filter() {
        let title = logs_panel_title(title_params(false, Some("alpha")));

        assert!(title.contains("f: filter off"));
        assert!(!title.contains("filter=alpha"));
        assert!(title.contains("l: hide"));
    }

    #[test]
    fn logs_panel_title_reports_active_filter_target() {
        let title = logs_panel_title(title_params(true, Some("alpha")));

        assert!(title.contains("f: filter=alpha"));
    }

    #[test]
    fn logs_panel_title_falls_back_to_compact_filter_hint_for_narrow_panels() {
        let long_id = "a-very-long-proposal-identifier-that-will-not-fit";
        let title = logs_panel_title(LogsPanelTitle {
            panel_width: 80,
            ..title_params(true, Some(long_id))
        });

        assert!(title.contains("f: filter on"));
        assert!(!title.contains(long_id));
    }

    #[test]
    fn logs_panel_title_reports_active_filter_without_a_cursor_proposal() {
        let title = logs_panel_title(title_params(true, None));

        assert!(title.contains("f: filter on"));
    }

    #[test]
    fn logs_panel_visible_buffer_shows_navigation_guidance() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.add_log(LogEntry::info("Test log message"));

        let buffer = render_buffer(&mut app, 120, 24);
        let content = buffer_to_string(&buffer);

        assert!(content.contains("PgUp/PgDn"));
        assert!(content.contains("Home/End"));
        assert!(content.contains("l: hide"));
    }

    #[test]
    fn test_log_panel_key_hint_always_shows() {
        // Test that 'l: logs' key hint is always shown in Changes view
        let mut app = create_test_app(vec![create_test_change("change-a")]);

        // Test in select mode (no logs)
        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("l: logs"),
            "Key hint 'l: logs' should be visible in select mode"
        );

        // Test in running mode (with logs)
        app.add_log(LogEntry::info("Test log"));
        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("l: logs"),
            "Key hint 'l: logs' should be visible in running mode"
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

    // === Tests for fix-width-aware-tui-log-display ===

    /// Render only the Logs panel, so width and height are exactly the values
    /// under test instead of whatever the running-mode layout happens to give.
    fn render_logs_panel(app: &mut AppState, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal init");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_logs(frame, app, area);
            })
            .expect("draw");
        terminal.backend().buffer().clone()
    }

    /// Text of one buffer row between `[from, to)` columns.
    ///
    /// A wide symbol occupies one cell and blanks the cells it spans, so those
    /// filler cells must be skipped or the reconstructed text would gain a space
    /// after every CJK character.
    fn buffer_row_text(buffer: &Buffer, y: u16, from: u16, to: u16) -> String {
        let mut text = String::new();
        let mut x = from;
        while x < to {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            let width = unicode_width::UnicodeWidthStr::width(symbol).max(1) as u16;
            x += width;
        }
        text
    }

    /// Inner rows of a Logs panel buffer, borders excluded, right padding trimmed.
    fn logs_panel_rows(buffer: &Buffer) -> Vec<String> {
        (1..buffer.area.height.saturating_sub(1))
            .map(|y| {
                buffer_row_text(buffer, y, 1, buffer.area.width.saturating_sub(1))
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    /// Deterministic long message: 26-character cycles make every wrapped
    /// segment identifiable without depending on word boundaries.
    fn long_ascii_message(len: usize) -> String {
        (0..len)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect()
    }

    fn app_with_single_log(message: &str) -> AppState {
        let mut app = create_test_app(vec![create_test_change("alpha")]);
        app.execution_mode = AppExecutionMode::Running;
        app.add_log(LogEntry::info(message));
        app
    }

    /// The message part of the first Logs row, i.e. everything after the
    /// `HH:MM:SS ` timestamp column.
    fn first_row_message(rows: &[String]) -> String {
        rows[0].chars().skip(9).collect()
    }

    #[test]
    fn wider_logs_panel_shows_more_retained_content_per_line() {
        let message = long_ascii_message(400);

        let narrow = logs_panel_rows(&render_logs_panel(
            &mut app_with_single_log(&message),
            60,
            12,
        ));
        let wide = logs_panel_rows(&render_logs_panel(
            &mut app_with_single_log(&message),
            120,
            12,
        ));

        let narrow_first = first_row_message(&narrow);
        let wide_first = first_row_message(&wide);

        // 60 - 2 borders - 9 timestamp = 49; 120 - 2 - 9 = 109.
        assert_eq!(narrow_first.chars().count(), 49);
        assert_eq!(wide_first.chars().count(), 109);
        assert!(wide_first.starts_with(&narrow_first));
        // Nothing was cut at a producer-fixed position.
        assert!(!wide_first.contains("..."));
    }

    #[test]
    fn logs_panel_renders_the_whole_retained_message_when_it_fits() {
        let message = long_ascii_message(400);
        let rows = logs_panel_rows(&render_logs_panel(
            &mut app_with_single_log(&message),
            120,
            12,
        ));

        let rebuilt: String = std::iter::once(first_row_message(&rows))
            .chain(rows[1..].iter().cloned())
            .collect();
        assert_eq!(rebuilt, message);
    }

    #[test]
    fn navigation_reaches_every_wrapped_segment_of_an_oversized_entry() {
        // 60 columns => 49 first-line columns and 58 continuation columns, so
        // 1,000 characters wrap to 18 lines against a 5-row viewport.
        let message = long_ascii_message(1000);
        let mut app = app_with_single_log(&message);

        // Establish the geometry the key handlers navigate with.
        let mut seen: Vec<String> = Vec::new();
        // The top rendered row only carries a timestamp when it is an entry's
        // first line; a continuation row starts at column zero.
        let record = |app: &mut AppState, seen: &mut Vec<String>| {
            let rows = logs_panel_rows(&render_logs_panel(app, 60, 7));
            let lines = app.log_display_lines();
            let top_is_entry_head = lines
                .get(app.log_start_line(&lines))
                .is_some_and(|line| line.is_first);

            let mut merged = String::new();
            for (index, row) in rows.iter().enumerate() {
                if index == 0 && top_is_entry_head {
                    merged.extend(row.chars().skip(9));
                } else {
                    merged.push_str(row);
                }
            }
            seen.push(merged);
        };

        record(&mut app, &mut seen);
        assert!(
            !seen[0].starts_with(&message[..49]),
            "auto-scroll starts at the tail of the entry"
        );

        app.scroll_logs_to_top();
        record(&mut app, &mut seen);
        // Home shows the first five wrapped segments: 49 + 4 * 58 = 281 chars,
        // which covers at least the first 200 source characters.
        assert_eq!(seen[1], message[..281]);
        assert!(seen[1].contains(&message[..200]));

        // Walk forward one page at a time until auto-scroll resumes.
        for _ in 0..5 {
            app.scroll_logs_down(5);
            record(&mut app, &mut seen);
        }
        assert!(
            app.log_auto_scroll,
            "PgDn eventually reaches the newest line"
        );

        // Every wrapped segment appeared in some rendered buffer.
        let lines = {
            app.set_log_viewport(60, 7);
            app.log_display_lines()
        };
        assert_eq!(lines.len(), 18);
        for line in &lines {
            assert!(
                seen.iter().any(|frame| frame.contains(&line.text)),
                "wrapped segment at offset {} never reached a rendered buffer",
                line.source_byte_offset
            );
        }

        // PgUp walks back to the head again.
        app.scroll_logs_to_top();
        record(&mut app, &mut seen);
        assert_eq!(seen.last().unwrap(), &message[..281]);
    }

    #[test]
    fn logs_panel_continuation_rows_use_the_full_inner_width_without_indent() {
        let message = long_ascii_message(400);
        let mut app = app_with_single_log(&message);
        app.scroll_logs_to_top();

        let rows = logs_panel_rows(&render_logs_panel(&mut app, 60, 7));

        assert_eq!(first_row_message(&rows), message[..49]);
        for (index, row) in rows[1..].iter().enumerate() {
            assert!(
                !row.starts_with(' '),
                "continuation row {index} must not be indented: {row:?}"
            );
            let start = 49 + index * 58;
            assert_eq!(row, &message[start..start + 58]);
        }
    }

    #[test]
    fn auto_scroll_keeps_the_newest_line_visible_behind_a_tall_entry() {
        let mut app = app_with_single_log(&long_ascii_message(1000));
        app.add_log(LogEntry::info("newest-line-marker"));

        let rows = logs_panel_rows(&render_logs_panel(&mut app, 60, 7));

        assert!(
            rows.last().unwrap().contains("newest-line-marker"),
            "auto-scroll must keep the newest line in view: {rows:?}"
        );
    }

    #[test]
    fn resizing_keeps_the_anchor_on_the_same_source_content() {
        let message = long_ascii_message(1000);
        let mut app = app_with_single_log(&message);

        // Navigate inside the entry at the narrow width.
        let _ = render_logs_panel(&mut app, 60, 7);
        app.scroll_logs_to_top();
        app.scroll_logs_down(3);
        let anchored = app.log_anchor.unwrap().source_byte_offset;
        assert_eq!(anchored, 49 + 2 * 58);

        // A resize re-wraps everything; the top rendered row must still contain
        // the anchored source position, and auto-scroll must stay off.
        let rows = logs_panel_rows(&render_logs_panel(&mut app, 120, 7));
        assert!(!app.log_auto_scroll);

        let lines = app.log_display_lines();
        let top = &lines[app.log_start_line(&lines)];
        assert!(top.source_byte_offset <= anchored);
        assert!(anchored < top.source_byte_offset + top.text.len());
        assert_eq!(rows[0], top.text, "the anchored line is the one drawn");
    }

    #[test]
    fn logs_panel_title_reports_display_lines_not_entry_offsets() {
        let mut app = app_with_single_log(&long_ascii_message(1000));
        app.scroll_logs_to_top();

        let buffer = render_logs_panel(&mut app, 120, 7);
        let title: String = (0..buffer.area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect();

        assert!(
            title.contains("lines"),
            "title must count display lines: {title}"
        );
        assert!(!title.contains("logs_off="));
    }

    #[test]
    fn logs_panel_wraps_cjk_and_emoji_within_the_inner_display_width() {
        let message = "漢字テスト🎉".repeat(40);
        let mut app = app_with_single_log(&message);
        app.scroll_logs_to_top();

        for width in [41u16, 60, 121] {
            let buffer = render_logs_panel(&mut app, width, 10);
            let rows = logs_panel_rows(&buffer);
            let inner_width = (width - 2) as usize;

            for row in &rows {
                assert!(
                    unicode_width::UnicodeWidthStr::width(row.as_str()) <= inner_width,
                    "row exceeds inner width {inner_width}: {row:?}"
                );
            }

            let rebuilt: String = std::iter::once(first_row_message(&rows))
                .chain(rows[1..].iter().cloned())
                .collect();
            assert!(
                message.starts_with(&rebuilt),
                "width {width} lost retained content: {rebuilt:?}"
            );
        }
    }

    #[test]
    fn filtering_returns_the_logs_panel_to_the_newest_matching_line() {
        let message = long_ascii_message(1000);
        let mut app = create_test_app(vec![
            create_test_change("alpha"),
            create_test_change("beta"),
        ]);
        app.execution_mode = AppExecutionMode::Running;
        app.add_log(LogEntry::info(message.clone()).with_change_id("alpha"));
        app.add_log(LogEntry::info("beta-only-line").with_change_id("beta"));

        let _ = render_logs_panel(&mut app, 60, 7);
        app.scroll_logs_to_top();
        assert!(!app.log_auto_scroll);

        // Toggling the filter re-targets the visible set, so the anchor is
        // discarded rather than left pointing at a hidden entry.
        app.toggle_selected_proposal_log_filter();
        assert!(app.log_auto_scroll);
        assert_eq!(app.log_anchor, None);

        let rows = logs_panel_rows(&render_logs_panel(&mut app, 60, 7));
        assert!(
            !rows.iter().any(|row| row.contains("beta-only-line")),
            "filtered-out entries must not render: {rows:?}"
        );
        // The newest matching line is the tail of the alpha entry.
        assert!(rows
            .last()
            .unwrap()
            .ends_with(&message[message.len() - 5..]));
    }

    #[test]
    fn buffer_trimming_keeps_the_anchored_view_on_a_rendered_line() {
        let mut app = create_test_app(vec![create_test_change("alpha")]);
        app.execution_mode = AppExecutionMode::Running;
        for index in 0..crate::tui::state::MAX_LOG_ENTRIES {
            app.add_log(LogEntry::info(format!("log-{index}")));
        }

        let _ = render_logs_panel(&mut app, 60, 7);
        app.scroll_logs_to_top();

        // Overflow evicts the anchored entry.
        for index in 0..5 {
            app.add_log(LogEntry::info(format!("overflow-{index}")));
        }

        let rows = logs_panel_rows(&render_logs_panel(&mut app, 60, 7));
        assert!(
            rows[0].contains("log-5"),
            "the view must clamp to the oldest surviving line: {rows:?}"
        );
        assert!(!app.log_auto_scroll, "trimming must not resume auto-scroll");
    }

    /// Changes-row preview policy: one line, current remaining width, no wrap.
    fn preview_app(mode: AppExecutionMode, message: &str) -> AppState {
        let mut app = create_test_app(vec![
            create_test_change("alpha"),
            create_test_change("beta"),
        ]);
        app.execution_mode = mode;
        app.add_log(
            LogEntry::info(message)
                .with_change_id("alpha")
                .with_operation("apply")
                .with_iteration(1),
        );
        app
    }

    fn row_index_containing(buffer: &Buffer, needle: &str) -> u16 {
        find_row_containing(buffer, needle).expect("row present")
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        buffer_row_text(buffer, y, 0, buffer.area.width)
    }

    #[test]
    fn wider_change_row_reveals_more_retained_preview_content() {
        for mode in [AppExecutionMode::Select, AppExecutionMode::Running] {
            let message = long_ascii_message(300);

            let mut narrow_app = preview_app(mode, &message);
            let narrow = render_buffer(&mut narrow_app, 110, 24);
            let narrow_row = row_text(&narrow, row_index_containing(&narrow, "alpha"));

            let mut wide_app = preview_app(mode, &message);
            let wide = render_buffer(&mut wide_app, 200, 24);
            let wide_row = row_text(&wide, row_index_containing(&wide, "alpha"));

            let narrow_preview = narrow_row.matches('a').count()
                + narrow_row.matches('b').count()
                + narrow_row.matches('c').count();
            let wide_preview = wide_row.matches('a').count()
                + wide_row.matches('b').count()
                + wide_row.matches('c').count();

            assert!(
                wide_preview > narrow_preview,
                "{mode:?}: a wider row must reveal more retained preview content"
            );
            assert!(
                narrow_row.contains('…'),
                "{mode:?}: a narrow row truncates at its actual width"
            );
            assert!(
                !wide_row.contains("..."),
                "{mode:?}: no producer-fixed cutoff may reach the preview"
            );
        }
    }

    #[test]
    fn change_row_preview_never_wraps_or_shifts_the_following_row() {
        for mode in [AppExecutionMode::Select, AppExecutionMode::Running] {
            let mut without = preview_app(mode, "short");
            without.logs.clear();
            let baseline = render_buffer(&mut without, 110, 24);
            let beta_without = row_index_containing(&baseline, "beta");

            let mut with = preview_app(mode, &long_ascii_message(400));
            let buffer = render_buffer(&mut with, 110, 24);
            let alpha = row_index_containing(&buffer, "alpha");
            let beta = row_index_containing(&buffer, "beta");

            assert_eq!(
                beta, beta_without,
                "{mode:?}: an oversized preview must not move the next row"
            );
            assert_eq!(beta, alpha + 1, "{mode:?}: no continuation row is created");
        }
    }

    #[test]
    fn change_row_preview_truncates_cjk_and_emoji_within_the_row_width() {
        for mode in [AppExecutionMode::Select, AppExecutionMode::Running] {
            for width in [90u16, 110, 160] {
                let mut app = preview_app(mode, &"日本語ログ🎉".repeat(40));
                let buffer = render_buffer(&mut app, width, 24);
                let alpha = row_index_containing(&buffer, "alpha");
                let row = row_text(&buffer, alpha);

                assert!(
                    unicode_width::UnicodeWidthStr::width(row.trim_end()) <= width as usize,
                    "{mode:?} at {width}: preview overflowed the row"
                );
                // Still exactly one row: the next change keeps its position.
                assert_eq!(row_index_containing(&buffer, "beta"), alpha + 1);
            }
        }
    }

    // === Tests for update-tui-log-wrap-no-indent ===

    #[test]
    fn test_logs_wrap_no_indent_continuation_lines() {
        // Continuation lines must NOT be indented; they start at column 0.
        // First line: timestamp + header + message start.
        // Continuation lines: message continuation from column 0 (no leading spaces).

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

        // Continuation lines should NOT be indented
        for (idx, line) in wrapped.iter().skip(1).enumerate() {
            assert!(
                !line.starts_with(' '),
                "Continuation line {} should NOT be indented, got: '{}'",
                idx + 2,
                line
            );
        }
    }

    #[test]
    fn test_wrap_log_message_continuation_uses_full_width() {
        // Continuation lines use available_width + timestamp_width (no indent),
        // so they can fit more characters than the first line.
        let message = "A".repeat(200);
        let available_width = 60; // area_width - border - timestamp
        let header_width = 10; // "[op:iter]" length
        let prefix_width = 19; // timestamp(9) + header(10)

        let wrapped = wrap_log_message(&message, available_width, header_width, prefix_width);

        // First line width: available_width - header_width = 50
        assert_eq!(wrapped[0].len(), 50, "First line should be 50 chars");

        // Continuation lines width: available_width + (prefix_width - header_width) = 60 + 9 = 69
        let continuation_width = available_width + (prefix_width - header_width);
        for (idx, line) in wrapped.iter().skip(1).enumerate() {
            assert!(
                line.len() <= continuation_width,
                "Continuation line {} len {} exceeds expected continuation_width {}",
                idx + 2,
                line.len(),
                continuation_width
            );
            // Lines that are not the last should be exactly continuation_width
            if idx + 2 < wrapped.len() {
                assert_eq!(
                    line.len(),
                    continuation_width,
                    "Non-last continuation line {} should be exactly {} chars",
                    idx + 2,
                    continuation_width
                );
            }
        }
    }

    #[test]
    fn test_logs_visible_range_not_broken_by_wrapped_entry() {
        // Test that visible range calculation works correctly with wrapped logs
        // When logs wrap to multiple display lines, the visible range should
        // show the correct portion based on display lines, not log count

        let mut app = create_test_app(vec![create_test_change("change-a")]);

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

        // Continuation lines should NOT be indented (no-indent policy)
        for line in wrapped.iter().skip(1) {
            assert!(
                !line.starts_with(' '),
                "Continuation line should NOT be indented, got: '{}'",
                line
            );
        }
    }

    // === Regression tests for fix-tui-log-wrap-unicode-boundary ===

    #[test]
    fn test_wrap_log_message_no_panic_arrow_unicode_prefix() {
        // Regression: panicked when message starts with \u{2192} (→, 3-byte UTF-8)
        // and available_width caused split_point to land inside the multi-byte char.
        //
        // Original panic:
        //   byte index 1 is not a char boundary; it is inside '\u{2192}' (bytes 0..3)
        //   of `\u{2192} Skill "cflx-workflow"`
        let message = "\u{2192} Skill \"cflx-workflow\"";
        // Narrow widths exercise the boundary condition
        for width in 1..=30 {
            let wrapped = wrap_log_message(message, width, 0, 0);
            // All characters must be preserved (no data loss)
            let reconstructed: String = wrapped.join("");
            assert_eq!(
                reconstructed, message,
                "Content must be preserved for width={}",
                width
            );
        }
    }

    #[test]
    fn test_wrap_log_message_available_width_1_no_panic() {
        // Regression: available_width=1 must not panic for any message content
        let messages = ["hello", "\u{2192} arrow", "日本語", "abc\u{2192}def"];
        for message in &messages {
            let wrapped = wrap_log_message(message, 1, 0, 0);
            let reconstructed: String = wrapped.join("");
            assert_eq!(
                reconstructed, *message,
                "Content must be preserved for message={:?} at width=1",
                message
            );
        }
    }

    /// An uncommitted row keeps its badge *and* its mark hint.
    ///
    /// Worktree eligibility is a start-admission fact, so it no longer removes
    /// the affordance: the operator may express intent now and the start key
    /// refuses later, naming the condition.
    #[test]
    fn test_uncommitted_change_still_advertises_its_mark_hint() {
        use crate::openspec::Change;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create a mock backend with sufficient size
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        // Create app state with an uncommitted change
        let changes = vec![Change {
            id: "test-change".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "2024-01-01".to_string(),
            dependencies: vec![],
            metadata: ProposalMetadata::default(),
        }];
        let mut app = AppState::new(changes);

        // Mark the change as uncommitted (not parallel eligible)
        app.changes[0].parallel_eligibility = ParallelEligibility::UncommittedProposalFiles;
        app.changes[0].selected = false;
        app.changes[0].display_status_cache = "not queued".to_string();

        // Render the frame
        terminal
            .draw(|f| {
                super::render(f, &mut app);
            })
            .unwrap();

        // Get the rendered buffer content
        let buffer = terminal.backend().buffer().clone();
        let content = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");

        // The mark hint is offered: the row is a visible non-terminal target.
        assert!(
            content.contains("Space: mark"),
            "an uncommitted row is still a mark target:\n{content}"
        );
        // And Space never described a queue mutation again.
        assert!(
            !content.contains("Space: queue") && !content.contains("Space: unqueue"),
            "Space must not be described as a queue control:\n{content}"
        );

        // Verify that UNCOMMITTED badge is shown
        assert!(
            content.contains("UNCOMMITTED"),
            "UNCOMMITTED badge should be shown"
        );
    }

    #[test]
    fn test_committed_change_shows_space_hint() {
        use crate::openspec::Change;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create a mock backend with sufficient size
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        // Create app state with a committed change
        let changes = vec![Change {
            id: "test-change".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "2024-01-01".to_string(),
            dependencies: vec![],
            metadata: ProposalMetadata::default(),
        }];
        let mut app = AppState::new(changes);

        // Mark the change as committed (parallel eligible) - this is the default
        app.changes[0].parallel_eligibility = ParallelEligibility::Eligible;
        app.changes[0].selected = false;
        app.changes[0].display_status_cache = "not queued".to_string();

        // Render the frame
        terminal
            .draw(|f| {
                super::render(f, &mut app);
            })
            .unwrap();

        // Get the rendered buffer content
        let buffer = terminal.backend().buffer().clone();
        let content = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");

        // Verify that the mark hint IS shown for committed changes
        assert!(
            content.contains("Space: mark"),
            "an unmarked row advertises `Space: mark`:\n{content}"
        );

        // And that it follows the mark state rather than the queue.
        app.changes[0].selected = true;
        let content = buffer_to_string(&render_buffer(&mut app, 120, 30));
        assert!(
            content.contains("Space: unmark"),
            "a marked row advertises `Space: unmark`:\n{content}"
        );
    }

    #[test]
    fn test_toggle_all_hint_shown_in_select_mode() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Select;

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("x: toggle all"),
            "Should show 'x: toggle all' hint in Select mode"
        );
    }

    #[test]
    fn test_toggle_all_hint_shown_in_stopped_mode() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Stopped;

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("x: toggle all"),
            "Should show 'x: toggle all' hint in Stopped mode"
        );
    }

    #[test]
    fn test_toggle_all_hint_shown_in_running_mode_with_non_active_target() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "not queued".to_string();

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("x: toggle all"),
            "Should show 'x: toggle all' hint in Running mode when non-active target exists"
        );
    }

    /// An active row is still a bulk-mark target: the mark it would receive is
    /// next-run intent, which an in-flight run neither owns nor answers.
    #[test]
    fn test_toggle_all_hint_shown_in_running_mode_for_an_active_row() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].display_status_cache = "resolving".to_string();

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("x: toggle all"),
            "an active row is a bulk-mark target like any other non-terminal row"
        );
    }

    /// The hint depends on the visible target set, not on the execution mode.
    #[test]
    fn test_toggle_all_hint_shown_in_stopping_and_error_modes() {
        for mode in [AppExecutionMode::Stopping, AppExecutionMode::Error] {
            let mut app = create_test_app(vec![create_test_change("change-a")]);
            app.execution_mode = mode;

            let buffer = render_buffer(&mut app, 100, 24);
            let content = buffer_to_string(&buffer);
            assert!(
                content.contains("x: toggle all"),
                "{mode:?} must offer the bulk toggle for a visible non-terminal row"
            );
        }
    }

    /// Terminal rows only: there is no run candidate left to mark.
    #[test]
    fn test_toggle_all_hint_not_shown_for_terminal_rows_only() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            let mut app = create_test_app(vec![create_test_change("change-a")]);
            app.execution_mode = mode;
            app.changes[0].display_status_cache = "archived".to_string();

            let buffer = render_buffer(&mut app, 100, 24);
            let content = buffer_to_string(&buffer);
            assert!(
                !content.contains("x: toggle all"),
                "{mode:?} must not offer the bulk toggle when every row is terminal"
            );
        }
    }

    #[test]
    fn test_toggle_all_hint_shown_in_select_mode_with_logs() {
        // Regression test: verify that toggle all hint is shown in Select mode
        // when logs are present (i.e., when render_changes_list_running is called)
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Select;
        app.add_log(LogEntry::info("Test log")); // Add log to trigger running mode rendering

        let buffer = render_buffer(&mut app, 100, 24);
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("x: toggle all"),
            "Should show 'x: toggle all' hint in Select mode with logs present"
        );
    }

    // =========================================================================
    // Tests for keep-TUI-running-after-recoverable-analysis-fallback
    // =========================================================================

    #[test]
    fn analysis_fallback_running_status_header_keeps_running_controls() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.current_change = Some("change-a".to_string());
        app.changes[0].display_status_cache = "applying".to_string();
        app.orchestration_started_at = Some(std::time::Instant::now());
        app.add_log(LogEntry::info("log"));
        let fallback = format!(
            "{}: error=Missing change IDs in response, queued=[\"change-a\"], in_flight=[]",
            crate::events::RECOVERABLE_ANALYSIS_FALLBACK_MARKER
        );

        // Production path: the producer emits the successful fallback as a warning.
        app.handle_orchestrator_event(crate::tui::events::OrchestratorEvent::Log(LogEntry::warn(
            fallback,
        )));

        let content = buffer_to_string(&render_buffer(&mut app, 100, 24));
        assert!(
            content.contains("Esc: stop"),
            "status header must retain running controls after fallback:\n{content}"
        );
        assert!(
            !content.contains(": retry, Ctrl+C: quit"),
            "status header must not offer error-mode retry controls after fallback:\n{content}"
        );
        assert!(
            content.contains("Elapsed"),
            "status header must retain elapsed orchestration display:\n{content}"
        );
    }

    #[test]
    fn fatal_global_error_status_header_still_shows_retry_controls() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.current_change = Some("change-a".to_string());
        app.orchestration_started_at = Some(std::time::Instant::now());
        app.add_log(LogEntry::info("log"));

        app.handle_orchestrator_event(crate::tui::events::OrchestratorEvent::Error {
            message: "Parallel execution failed: base worktree is unusable".to_string(),
        });

        let content = buffer_to_string(&render_buffer(&mut app, 100, 24));
        assert!(
            content.contains(": retry, Ctrl+C: quit"),
            "fatal errors must still expose retry controls:\n{content}"
        );
        assert!(
            !content.contains("Esc: stop"),
            "fatal errors must not keep running controls:\n{content}"
        );
    }

    #[test]
    fn fatal_global_error_status_header_quoting_fallback_shows_retry_controls() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        app.current_change = Some("change-a".to_string());
        app.orchestration_started_at = Some(std::time::Instant::now());
        app.add_log(LogEntry::info("log"));
        // A fatal diagnostic that wraps the recoverable fallback wording must not be
        // downgraded by message content.
        let fatal = format!(
            "Parallel execution failed: base worktree is unusable (last diagnostic: \"{}: error=Missing change IDs in response, queued=[\"change-a\"], in_flight=[]\")",
            crate::events::RECOVERABLE_ANALYSIS_FALLBACK_MARKER
        );

        app.handle_orchestrator_event(crate::tui::events::OrchestratorEvent::Error {
            message: fatal,
        });

        let content = buffer_to_string(&render_buffer(&mut app, 100, 24));
        assert!(
            content.contains(": retry, Ctrl+C: quit"),
            "a fatal error quoting fallback text must still expose retry controls:\n{content}"
        );
        assert!(
            !content.contains("Esc: stop"),
            "a fatal error quoting fallback text must not keep running controls:\n{content}"
        );
    }

    // =========================================================================
    // Tests for split_remote_change_id
    // =========================================================================

    #[test]
    fn test_split_remote_change_id_local() {
        let parsed = split_remote_change_id("my-change");
        assert_eq!(parsed.project, None);
        assert_eq!(parsed.change, "my-change");
    }

    #[test]
    fn test_split_remote_change_id_remote() {
        // Format: <project_id>::<project_name>/<change_id>
        let parsed = split_remote_change_id("abc123::myproject/add-feature");
        assert_eq!(parsed.project, Some("myproject"));
        assert_eq!(parsed.change, "add-feature");
    }

    #[test]
    fn test_split_remote_change_id_remote_nested_path() {
        // rsplit_once('/') means we split at the LAST slash
        let parsed = split_remote_change_id("abc123::org/project/fix-bug");
        assert_eq!(parsed.project, Some("org/project"));
        assert_eq!(parsed.change, "fix-bug");
    }

    #[test]
    fn test_split_remote_change_id_no_slash_after_colon() {
        // "::" present but no "/" after it
        let parsed = split_remote_change_id("abc123::mychange");
        assert_eq!(parsed.project, None);
        assert_eq!(parsed.change, "mychange");
    }

    // =========================================================================
    // Tests for build_change_rows
    // =========================================================================

    fn make_change_state(id: &str) -> ChangeState {
        ChangeState {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 3,
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
            archive_complete_cache: false,
        }
    }

    #[test]
    fn test_build_change_rows_all_local() {
        let changes = vec![make_change_state("change-a"), make_change_state("change-b")];
        let (rows, c2v) = build_change_rows(&changes);
        // No project grouping: 2 rows, no headers
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], ChangeRow::Item { change_index: 0 }));
        assert!(matches!(rows[1], ChangeRow::Item { change_index: 1 }));
        assert_eq!(c2v[0], 0);
        assert_eq!(c2v[1], 1);
    }

    #[test]
    fn test_build_change_rows_remote_grouping() {
        let changes = vec![
            make_change_state("p1::proj-a/change-x"),
            make_change_state("p1::proj-a/change-y"),
            make_change_state("p2::proj-b/change-z"),
        ];
        let (rows, c2v) = build_change_rows(&changes);
        // 2 project groups → 2 headers + 3 change rows = 5 visual rows
        assert_eq!(rows.len(), 5);
        // Row 0: header "proj-a"
        assert!(matches!(&rows[0], ChangeRow::Header(h) if h == "proj-a"));
        // Row 1: change-x (change_index=0)
        assert!(matches!(rows[1], ChangeRow::Item { change_index: 0 }));
        // Row 2: change-y (change_index=1)
        assert!(matches!(rows[2], ChangeRow::Item { change_index: 1 }));
        // Row 3: header "proj-b"
        assert!(matches!(&rows[3], ChangeRow::Header(h) if h == "proj-b"));
        // Row 4: change-z (change_index=2)
        assert!(matches!(rows[4], ChangeRow::Item { change_index: 2 }));
        // Mapping: change 0 → visual 1, change 1 → visual 2, change 2 → visual 4
        assert_eq!(c2v[0], 1);
        assert_eq!(c2v[1], 2);
        assert_eq!(c2v[2], 4);
    }

    #[test]
    fn test_build_change_rows_mixed_local_and_remote() {
        let changes = vec![
            make_change_state("local-change"),
            make_change_state("pid::remote-proj/remote-change"),
        ];
        let (rows, c2v) = build_change_rows(&changes);
        // 2 project groups (None for local, Some("remote-proj") for remote) → 2 headers + 2 items
        assert_eq!(rows.len(), 4);
        assert!(matches!(&rows[0], ChangeRow::Header(h) if h == "(local)"));
        assert!(matches!(rows[1], ChangeRow::Item { change_index: 0 }));
        assert!(matches!(&rows[2], ChangeRow::Header(h) if h == "remote-proj"));
        assert!(matches!(rows[3], ChangeRow::Item { change_index: 1 }));
        assert_eq!(c2v[0], 1);
        assert_eq!(c2v[1], 3);
    }

    #[test]
    fn test_grouped_display_shows_project_header() {
        // Render with two changes from the same remote project
        let app_changes = vec![
            create_test_change("pid::myproject/feat-a"),
            create_test_change("pid::myproject/feat-b"),
        ];
        let mut app = create_test_app(app_changes);

        let buffer = render_buffer(&mut app, 120, 30);
        let content = buffer_to_string(&buffer);

        // Project header should appear
        assert!(
            content.contains("myproject"),
            "Should show project name as header in grouped display"
        );
        // Bare change ids should appear (not the full path)
        assert!(
            content.contains("feat-a"),
            "Should show bare change id feat-a"
        );
        assert!(
            content.contains("feat-b"),
            "Should show bare change id feat-b"
        );
    }

    // ========================================================================
    // Base execution rendering with independent modal overlays
    // ========================================================================

    const ALL_EXECUTION_MODES: [AppExecutionMode; 5] = [
        AppExecutionMode::Select,
        AppExecutionMode::Running,
        AppExecutionMode::Stopping,
        AppExecutionMode::Stopped,
        AppExecutionMode::Error,
    ];

    fn overlay_app() -> AppState {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.web_url = Some("http://127.0.0.1:8080".to_string());
        app.add_log(LogEntry::info("orchestration log"));
        app
    }

    fn delete_overlay() -> ModalState {
        ModalState::ConfirmWorktreeDelete {
            path: std::path::PathBuf::from("/tmp/worktree-a"),
            branch: "change-a".to_string(),
        }
    }

    #[test]
    fn qr_overlay_renders_above_every_execution_mode() {
        for mode in ALL_EXECUTION_MODES {
            let mut app = overlay_app();
            app.execution_mode = mode;
            app.show_qr_popup();

            let content = buffer_to_string(&render_buffer(&mut app, 100, 40));

            assert!(
                content.contains("Web UI QR Code"),
                "QR overlay must render above {mode:?}"
            );
            assert!(content.contains("http://127.0.0.1:8080"));
        }
    }

    #[test]
    fn worktree_confirmation_renders_above_every_execution_mode() {
        for mode in ALL_EXECUTION_MODES {
            let mut app = overlay_app();
            app.execution_mode = mode;
            app.modal = Some(delete_overlay());

            let content = buffer_to_string(&render_buffer(&mut app, 100, 40));

            assert!(
                content.contains("Delete Worktree"),
                "worktree confirmation must render above {mode:?}"
            );
            assert!(content.contains("/tmp/worktree-a"));
        }
    }

    #[test]
    fn worktree_confirmation_over_error_keeps_the_error_base_and_retry_control() {
        let mut app = overlay_app();
        app.execution_mode = AppExecutionMode::Error;
        app.modal = Some(delete_overlay());

        let content = buffer_to_string(&render_buffer(&mut app, 100, 40));

        assert!(
            content.contains(&format!("{}: retry", app.start_key_label())),
            "the Error base must keep its retry control under an overlay: {content}"
        );
        assert!(content.contains("Delete Worktree"));
        assert!(
            !content.contains("[Ready]") && !content.contains("[Running]"),
            "an overlay must not rewrite the base to Select or Running"
        );
    }

    #[test]
    fn force_kill_over_stopping_keeps_the_stopping_base_and_shows_confirm_hints() {
        let mut app = overlay_app();
        app.execution_mode = AppExecutionMode::Stopping;
        app.changes[0].set_display_status_cache("applying");
        app.modal = Some(ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        });

        let content = buffer_to_string(&render_buffer(&mut app, 120, 40));

        assert!(
            content.contains("force stop"),
            "the Stopping base must keep its own controls: {content}"
        );
        assert!(content.contains("Y: confirm kill"));
        assert!(content.contains("N: cancel"));
        assert!(
            !content.contains("K: kill"),
            "the confirmation replaces the kill hint while it owns input"
        );
    }

    #[test]
    fn invalidated_force_kill_reveals_the_current_base_without_conversion() {
        let mut app = overlay_app();
        app.execution_mode = AppExecutionMode::Stopped;
        app.changes[0].set_display_status_cache("applying");
        // The confirmation was invalidated; only the modal axis changed.
        app.modal = None;

        let content = buffer_to_string(&render_buffer(&mut app, 120, 40));

        assert!(!content.contains("Y: confirm kill"));
        assert!(content.contains("K: kill"));
        assert!(
            content.contains(&format!("{}: resume", app.start_key_label())),
            "the Stopped base renders as itself, not as Running: {content}"
        );
    }

    #[test]
    fn base_status_controls_come_from_execution_state_only() {
        let expectations = [
            (AppExecutionMode::Select, "Status (Ctrl+C: quit)"),
            (AppExecutionMode::Running, "Status (Esc: stop"),
            (AppExecutionMode::Stopping, "force stop"),
            (AppExecutionMode::Stopped, ": resume"),
            (AppExecutionMode::Error, ": retry"),
        ];

        for (mode, expected) in expectations {
            // The QR popup is a large centered overlay that legitimately covers the
            // status panel, so it is exercised by its own test above.
            for modal in [
                None,
                Some(delete_overlay()),
                Some(ModalState::ConfirmForceKill {
                    change_id: "change-a".to_string(),
                }),
            ] {
                let mut app = overlay_app();
                app.execution_mode = mode;
                app.modal = modal.clone();

                let content = buffer_to_string(&render_buffer(&mut app, 120, 40));

                assert!(
                    content.contains(expected),
                    "{mode:?} with {modal:?} must keep base control {expected:?}: {content}"
                );
            }
        }
    }

    #[test]
    fn overlay_header_label_is_presentation_only() {
        let mut app = overlay_app();
        app.execution_mode = AppExecutionMode::Running;

        let running = buffer_to_string(&render_buffer(&mut app, 120, 40));
        assert!(running.contains("[Running]"));

        app.show_qr_popup();
        let qr = buffer_to_string(&render_buffer(&mut app, 120, 40));
        assert!(qr.contains("[QR Code]"));
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Running,
            "rendering must never mutate the execution axis"
        );

        app.modal = Some(delete_overlay());
        let delete = buffer_to_string(&render_buffer(&mut app, 120, 40));
        assert!(delete.contains("[Confirm Delete]"));

        app.modal = Some(ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        });
        let kill = buffer_to_string(&render_buffer(&mut app, 120, 40));
        assert!(kill.contains("[Confirm Kill]"));
    }

    #[test]
    fn warning_popup_renders_above_an_interaction_modal() {
        let mut app = overlay_app();
        app.execution_mode = AppExecutionMode::Running;
        app.show_qr_popup();
        app.show_warning_popup("Merge warning", "conflict in file.rs");

        let content = buffer_to_string(&render_buffer(&mut app, 120, 40));

        assert!(content.contains("Merge warning"));
        assert!(content.contains("conflict in file.rs"));
    }

    // ------------------------------------------------------------------
    // Error row previews and the Error Details popup
    // ------------------------------------------------------------------

    const STALLED: &str = "Apply failed: stalled after 5 empty WIP commits";

    /// A Select-view app with one `error` row whose failure log is gone.
    ///
    /// The buffer is left empty on purpose: it is the state a change reaches
    /// once its failure entry has been pushed out of the bounded log buffer.
    fn error_row_app(detail: Option<&str>) -> AppState {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        match detail {
            Some(detail) => app.changes[0].set_error_message_cache(detail.to_string()),
            None => app.changes[0].set_display_status_cache("error"),
        }
        app
    }

    /// Foreground color of the first cell of `needle` in the rendered buffer.
    fn fg_at(buffer: &Buffer, needle: &str) -> Color {
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            if let Some(byte_index) = line.find(needle) {
                let column = line[..byte_index].chars().count() as u16;
                return buffer[(column, y)].fg;
            }
        }
        panic!("{needle:?} not found in rendered buffer");
    }

    #[test]
    fn error_row_preview_survives_log_eviction() {
        let mut app = error_row_app(Some(STALLED));
        assert!(
            app.get_latest_log_for_change("change-a").is_none(),
            "the failure entry is gone from the bounded buffer"
        );

        let content = buffer_to_string(&render_buffer(&mut app, 160, 20));

        assert!(
            content.contains(&format!("Error: {STALLED}")),
            "an error row must name its failure without any retained log: {content}"
        );
    }

    #[test]
    fn error_row_preview_takes_precedence_over_the_latest_log() {
        let mut app = error_row_app(Some(STALLED));
        app.add_log(LogEntry::info("refreshed workspace status").with_change_id("change-a"));

        let content = buffer_to_string(&render_buffer(&mut app, 160, 20));
        let changes_panel = content
            .lines()
            .find(|line| line.contains("change-a") && line.contains("[error]"))
            .expect("the error row is rendered")
            .to_string();

        assert!(
            changes_panel.contains(&format!("Error: {STALLED}")),
            "the retained diagnostic must win: {changes_panel}"
        );
        assert!(
            !changes_panel.contains("refreshed workspace status"),
            "an unrelated log must not be presented as the failure reason: {changes_panel}"
        );
    }

    #[test]
    fn error_row_without_a_diagnostic_states_that_details_are_unavailable() {
        let mut app = error_row_app(None);
        app.add_log(LogEntry::info("refreshed workspace status").with_change_id("change-a"));

        let content = buffer_to_string(&render_buffer(&mut app, 160, 20));
        let changes_panel = content
            .lines()
            .find(|line| line.contains("change-a") && line.contains("[error]"))
            .expect("the error row is rendered")
            .to_string();

        assert!(
            changes_panel.contains(ERROR_DETAILS_UNAVAILABLE),
            "a missing diagnostic must be explicit: {changes_panel}"
        );
        assert!(
            !changes_panel.contains("refreshed workspace status"),
            "no error reason may be inferred from an ordinary log: {changes_panel}"
        );
    }

    #[test]
    fn non_error_rows_keep_the_latest_log_preview() {
        let mut app = create_test_app(vec![create_test_change("change-a")]);
        app.changes[0].set_display_status_cache("applying");
        app.add_log(LogEntry::info("applying tasks").with_change_id("change-a"));

        let content = buffer_to_string(&render_buffer(&mut app, 160, 20));

        assert!(
            content.contains("applying tasks"),
            "non-error preview behavior is unchanged: {content}"
        );
        assert!(!content.contains("Error: "));
    }

    #[test]
    fn error_preview_is_omitted_when_the_remaining_width_is_too_small() {
        let mut app = error_row_app(Some(STALLED));

        let content = buffer_to_string(&render_buffer(&mut app, 72, 20));

        assert!(
            content.contains("change-a"),
            "the row itself is still rendered: {content}"
        );
        assert!(
            !content.contains("Error: "),
            "no preview fits below the minimum preview width: {content}"
        );
    }

    #[test]
    fn error_preview_truncation_is_unicode_safe() {
        // A wide character occupies two buffer cells, so assertions here look at
        // the characters that survived rather than at a raw substring match.
        let diagnostic = "適用に失敗しました🚀 スタックしています 追記済みです。";

        // The narrowest width here is the first one that leaves the fixed
        // 36-column ID field enough room for the minimum preview *and* one wide
        // character beside the `" Error: "` prefix and the ellipsis.
        for width in [90u16, 100, 110, 120, 140, 160] {
            let mut app = error_row_app(Some(diagnostic));

            // Rendering must complete at every width without panicking on a
            // split character, and the preview must never wrap onto a second line.
            let content = buffer_to_string(&render_buffer(&mut app, width, 20));
            let preview_lines: Vec<&str> =
                content.lines().filter(|line| line.contains('適')).collect();
            assert_eq!(
                preview_lines.len(),
                1,
                "the error preview must be rendered on exactly one line at width {width}: {content}"
            );
            let preview = preview_lines[0];

            // The diagnostic needs 54 display columns plus the 8-column
            // `" Error: "` prefix, so it only fits once the row is wide enough
            // to leave that much space beside the fixed columns.
            if width >= 140 {
                assert!(
                    preview.contains('。') && !preview.contains('…'),
                    "the whole diagnostic fits at width {width}: {preview}"
                );
            } else {
                assert!(
                    preview.contains('…'),
                    "a diagnostic wider than the row must be truncated at width {width}: {preview}"
                );
            }
        }
    }

    #[test]
    fn error_preview_uses_error_styling_on_focused_and_unfocused_rows() {
        let mut app = create_test_app(vec![
            create_test_change("change-a"),
            create_test_change("change-b"),
        ]);
        app.changes[0].set_error_message_cache("first failure".to_string());
        app.changes[1].set_error_message_cache("second failure".to_string());
        app.cursor_index = 0;

        let buffer = render_buffer(&mut app, 160, 20);

        assert_eq!(
            fg_at(&buffer, "Error: first failure"),
            Color::LightRed,
            "the focused row needs a color readable on the highlight background"
        );
        assert_eq!(fg_at(&buffer, "Error: second failure"), Color::Red);
    }

    #[test]
    fn changes_panel_advertises_details_only_on_an_error_row() {
        for mode in [AppExecutionMode::Select, AppExecutionMode::Running] {
            let mut app = create_test_app(vec![
                create_test_change("change-a"),
                create_test_change("change-b"),
            ]);
            app.execution_mode = mode;
            if mode == AppExecutionMode::Running {
                app.add_log(LogEntry::info("orchestration log"));
            }
            app.changes[0].set_error_message_cache(STALLED.to_string());
            app.cursor_index = 0;

            let on_error = buffer_to_string(&render_buffer(&mut app, 200, 24));
            assert!(
                on_error.contains("Enter: details"),
                "{mode:?} must advertise the details action on an error row: {on_error}"
            );

            app.cursor_index = 1;
            let off_error = buffer_to_string(&render_buffer(&mut app, 200, 24));
            assert!(
                !off_error.contains("Enter: details"),
                "{mode:?} must not advertise it on a non-error row: {off_error}"
            );
        }
    }

    #[test]
    fn error_details_popup_shows_the_change_id_and_complete_diagnostic() {
        let mut app = error_row_app(Some(STALLED));
        assert!(app.open_error_details_popup());

        let content = buffer_to_string(&render_buffer(&mut app, 160, 30));

        assert!(content.contains("Error Details"), "{content}");
        assert!(content.contains("Change: change-a"), "{content}");
        assert!(content.contains(STALLED), "{content}");
    }

    #[test]
    fn error_details_popup_advertises_scroll_copy_and_close() {
        let mut app = error_row_app(Some(STALLED));
        assert!(app.open_error_details_popup());

        let content = buffer_to_string(&render_buffer(&mut app, 160, 30));

        assert!(content.contains("scroll"), "{content}");
        assert!(content.contains("c: copy"), "{content}");
        assert!(content.contains("Esc"), "{content}");
    }

    #[test]
    fn error_details_popup_reports_copy_outcomes_without_losing_the_diagnostic() {
        let mut app = error_row_app(Some(STALLED));
        assert!(app.open_error_details_popup());

        app.set_clipboard(std::sync::Arc::new(
            crate::tui::clipboard::test_doubles::RecordingClipboard::default(),
        ));
        app.copy_error_details();
        let success = buffer_to_string(&render_buffer(&mut app, 160, 30));
        assert!(success.contains("Copied to clipboard"), "{success}");
        assert!(success.contains(STALLED), "{success}");

        app.set_clipboard(std::sync::Arc::new(
            crate::tui::clipboard::test_doubles::FailingClipboard::new("no clipboard provider"),
        ));
        app.copy_error_details();
        let failure = buffer_to_string(&render_buffer(&mut app, 160, 30));
        assert!(
            failure.contains("Copy failed: no clipboard provider"),
            "{failure}"
        );
        assert!(
            failure.contains("manually"),
            "failure feedback must be actionable: {failure}"
        );
        assert!(failure.contains(STALLED), "{failure}");
    }

    #[test]
    fn error_details_popup_renders_above_an_interaction_modal_and_below_a_warning() {
        let mut app = error_row_app(Some(STALLED));
        app.web_url = Some("http://127.0.0.1:8080".to_string());
        assert!(app.open_error_details_popup());
        app.show_qr_popup();

        let over_modal = buffer_to_string(&render_buffer(&mut app, 160, 30));
        assert!(over_modal.contains("Error Details"), "{over_modal}");
        assert!(over_modal.contains(STALLED), "{over_modal}");

        app.show_warning_popup("Merge warning", "conflict in file.rs");
        let under_warning = buffer_to_string(&render_buffer(&mut app, 160, 30));
        assert!(under_warning.contains("Merge warning"), "{under_warning}");
        assert!(
            under_warning.contains("conflict in file.rs"),
            "{under_warning}"
        );
        assert!(
            !under_warning.contains("Error Details"),
            "the warning popup owns the top of the stack: {under_warning}"
        );
    }
}
