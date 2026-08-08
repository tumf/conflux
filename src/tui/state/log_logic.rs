//! Logs-panel presentation logic shared by state, key handling, and rendering.
//!
//! The Logs panel navigates *display lines*, not entries: one retained entry can
//! wrap into more lines than the viewport is tall, and every one of those lines
//! must be reachable. The single contract lives here so `state.rs` (navigation),
//! `key_handlers.rs` (key assignments), and `render.rs` (drawing) can never
//! disagree about how many lines exist or which one sits at the top.
//!
//! All of it is process-local presentation state: it is recomputed from
//! `AppState::logs` plus the last rendered geometry and is never a
//! workflow-control input.

use crate::tui::events::LogEntry;
use unicode_width::UnicodeWidthChar;

/// Width of the rendered `HH:MM:SS ` timestamp column.
pub(crate) const LOG_TIMESTAMP_WIDTH: usize = 9;

/// Left + right Logs-panel border columns.
pub(crate) const LOG_BORDER_WIDTH: usize = 2;

/// Top + bottom Logs-panel border rows.
pub(crate) const LOG_BORDER_HEIGHT: usize = 2;

/// Last Logs-panel geometry the renderer observed.
///
/// Navigation needs a width to know where wrapped lines break, and the renderer
/// is the only place that knows it. The defaults only matter before the first
/// draw; every draw overwrites them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LogViewport {
    pub width: usize,
    pub height: usize,
}

impl Default for LogViewport {
    fn default() -> Self {
        Self {
            width: 80,
            height: 12,
        }
    }
}

impl LogViewport {
    /// Rows available for log lines, borders excluded.
    pub fn visible_height(&self) -> usize {
        self.height.saturating_sub(LOG_BORDER_HEIGHT)
    }
}

/// Anchor identifying the top visible display line in *source* coordinates.
///
/// Storing the entry plus a byte offset into its message — rather than a display
/// line index — is what makes the anchor survive a width change, a filter
/// toggle, an append, and a buffer trim: display lines are recomputed, the
/// anchored source position is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LogViewAnchor {
    /// Process-local monotonic sequence number of the anchored entry.
    pub entry_seq: u64,
    /// Byte offset into that entry's message where the anchored line starts.
    pub source_byte_offset: usize,
}

/// One wrapped display line of one log entry.
pub(crate) struct LogDisplayLine {
    /// Index into the filtered entry slice this line belongs to.
    pub entry_index: usize,
    /// Process-local sequence number of the owning entry.
    pub entry_seq: u64,
    /// Byte offset of this line's first character within the entry message.
    pub source_byte_offset: usize,
    /// Whether this is the entry's first line (the one carrying the header).
    pub is_first: bool,
    pub text: String,
}

/// Message columns available on a continuation line of a panel `panel_width` wide.
pub(crate) fn logs_available_width(panel_width: usize) -> usize {
    panel_width.saturating_sub(LOG_BORDER_WIDTH + LOG_TIMESTAMP_WIDTH)
}

/// Contextual header for one entry, matching the Logs-view header spec.
///
/// Returned with its trailing space so the caller can measure the exact first
/// line prefix.
pub(crate) fn logs_panel_header(entry: &LogEntry) -> String {
    let Some(operation) = entry.operation.as_ref() else {
        return String::new();
    };

    match (&entry.change_id, entry.iteration) {
        (Some(change_id), Some(iter)) => format!("[{}:{}:{}] ", change_id, operation, iter),
        (Some(change_id), None) => format!("[{}:{}] ", change_id, operation),
        (None, Some(iter)) => format!("[{}:{}] ", operation, iter),
        // Analysis logs must always carry an iteration.
        (None, None) if operation == "analysis" => format!("[{}:1] ", operation),
        (None, None) => format!("[{}] ", operation),
    }
}

/// Flatten the filtered entries into the display-line sequence for `panel_width`.
///
/// `entries` pairs each visible entry with its process-local sequence number so
/// the resulting lines stay addressable after the buffer is trimmed.
pub(crate) fn build_log_display_lines(
    entries: &[(u64, &LogEntry)],
    panel_width: usize,
) -> Vec<LogDisplayLine> {
    let available_width = logs_available_width(panel_width);
    let mut lines = Vec::new();

    for (entry_index, (entry_seq, entry)) in entries.iter().enumerate() {
        let header = logs_panel_header(entry);
        let timestamp_width = entry.timestamp.len() + 1; // "HH:MM:SS "
        let prefix_width = timestamp_width + header.len();

        for (offset, text) in wrap_log_message_with_offsets(
            &entry.message,
            available_width,
            header.len(),
            prefix_width,
        ) {
            lines.push(LogDisplayLine {
                entry_index,
                entry_seq: *entry_seq,
                source_byte_offset: offset,
                is_first: offset == 0,
                text,
            });
        }
    }

    lines
}

/// Project a source-coordinate anchor onto the current display-line sequence.
///
/// The anchored entry may have been trimmed away or filtered out; both cases
/// resolve deterministically to the nearest surviving line rather than to a
/// silent jump back to the newest output.
pub(crate) fn project_anchor_to_line(lines: &[LogDisplayLine], anchor: LogViewAnchor) -> usize {
    let mut exact = None;
    let mut first_newer = None;

    for (index, line) in lines.iter().enumerate() {
        if line.entry_seq == anchor.entry_seq {
            if line.source_byte_offset <= anchor.source_byte_offset {
                exact = Some(index);
            }
        } else if line.entry_seq > anchor.entry_seq && first_newer.is_none() {
            first_newer = Some(index);
        }
    }

    exact
        .or(first_newer)
        // Anchored entry is newer than everything left: clamp to the last line.
        .unwrap_or_else(|| lines.len().saturating_sub(1))
}

/// Top visible display line for the current anchor.
///
/// `None` means "follow the newest line", which is exactly the auto-scroll
/// position. A stale anchor can never scroll past the end.
pub(crate) fn resolve_start_line(
    lines: &[LogDisplayLine],
    anchor: Option<LogViewAnchor>,
    visible_height: usize,
) -> usize {
    let max_start = lines.len().saturating_sub(visible_height);
    match anchor {
        None => max_start,
        Some(anchor) => project_anchor_to_line(lines, anchor).min(max_start),
    }
}

/// Anchor describing the display line at `index`, if it exists.
pub(crate) fn anchor_at_line(lines: &[LogDisplayLine], index: usize) -> Option<LogViewAnchor> {
    lines.get(index).map(|line| LogViewAnchor {
        entry_seq: line.entry_seq,
        source_byte_offset: line.source_byte_offset,
    })
}

/// Split `s` into `(prefix, remainder)` where `prefix` occupies at most
/// `max_width` terminal display columns and never cuts inside a UTF-8 codepoint.
///
/// If the very first character is wider than `max_width` (e.g. a wide CJK
/// character when `max_width` is 1) it is included anyway to prevent an
/// infinite loop in the caller.
pub(crate) fn take_chars_by_display_width(s: &str, max_width: usize) -> (&str, &str) {
    let mut current_width = 0usize;
    let mut byte_pos = 0usize;
    for ch in s.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(1);
        if current_width + char_width > max_width {
            // If no character has been consumed yet, take this one anyway to
            // avoid an infinite loop caused by a wide char exceeding max_width.
            if current_width == 0 {
                byte_pos += ch.len_utf8();
            }
            break;
        }
        current_width += char_width;
        byte_pos += ch.len_utf8();
    }
    (&s[..byte_pos], &s[byte_pos..])
}

/// Wrap a log message for the Logs view, returning each line with the byte
/// offset it starts at in `message`.
///
/// The first line starts after the timestamp+header prefix. Continuation lines
/// are NOT indented and use the full inner panel width, so more text is visible.
///
/// `available_width` is the width available after subtracting borders and
/// timestamp, `header_width` the width of the contextual header, and
/// `prefix_width` timestamp width + header width.
pub(crate) fn wrap_log_message_with_offsets(
    message: &str,
    available_width: usize,
    header_width: usize,
    prefix_width: usize,
) -> Vec<(usize, String)> {
    if available_width == 0 {
        return vec![(0, message.to_string())];
    }

    let first_width = available_width.saturating_sub(header_width);
    if first_width == 0 {
        return vec![(0, message.to_string())];
    }

    let mut lines = Vec::new();
    let (first_part, mut remaining) = take_chars_by_display_width(message, first_width);
    lines.push((0, first_part.to_string()));

    if remaining.is_empty() {
        return lines;
    }

    // Continuation lines: no indent, so they reclaim the timestamp+header columns.
    let continuation_width =
        available_width.saturating_add(prefix_width.saturating_sub(header_width));

    let mut offset = first_part.len();
    while !remaining.is_empty() {
        if continuation_width == 0 {
            lines.push((offset, remaining.to_string()));
            break;
        }

        let (chunk, rest) = take_chars_by_display_width(remaining, continuation_width);
        lines.push((offset, chunk.to_string()));
        offset += chunk.len();
        remaining = rest;
    }

    lines
}

/// Text-only wrapper kept for wrapping assertions that do not need offsets.
#[cfg(test)]
pub(crate) fn wrap_log_message(
    message: &str,
    available_width: usize,
    header_width: usize,
    prefix_width: usize,
) -> Vec<String> {
    wrap_log_message_with_offsets(message, available_width, header_width, prefix_width)
        .into_iter()
        .map(|(_, text)| text)
        .collect()
}

pub(super) fn apply_log_buffer_limit(current_len: usize, max_entries: usize) -> bool {
    current_len > max_entries
}

pub(super) fn toggle_logs_panel(current: bool) -> bool {
    !current
}

/// Flip the presentation-only selected-proposal log filter.
pub(super) fn toggle_selected_proposal_log_filter(current: bool) -> bool {
    !current
}

/// Decide whether a cursor move must return the Logs panel to its newest
/// matching position.
///
/// Only an enabled filter whose target actually changed invalidates the current
/// display-line anchor; cursor movement with the filter off leaves the
/// unfiltered scroll position alone.
pub(super) fn should_reset_log_position_for_target_change(
    filter_enabled: bool,
    previous_target: Option<&str>,
    next_target: Option<&str>,
) -> bool {
    filter_enabled && previous_target != next_target
}

/// Exact structured association between a log entry and the filter target.
///
/// Proposal identity comes only from `LogEntry::change_id`; entries without one
/// (global orchestration output) and entries carrying a different ID (including
/// remote project-only IDs) are excluded while the filter is enabled.
pub(super) fn log_entry_matches_selected_proposal(
    filter_enabled: bool,
    entry_change_id: Option<&str>,
    target_change_id: Option<&str>,
) -> bool {
    if !filter_enabled {
        return true;
    }

    match (entry_change_id, target_change_id) {
        (Some(entry_id), Some(target_id)) => entry_id == target_id,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_log_buffer_limit_only_when_exceeding_max() {
        assert!(!apply_log_buffer_limit(1000, 1000));
        assert!(apply_log_buffer_limit(1001, 1000));
    }

    /// Deterministic entry with a fixed timestamp width; `LogEntry::info`
    /// timestamps are always `HH:MM:SS`, so wrapping math stays stable.
    fn entry(message: &str) -> LogEntry {
        LogEntry::info(message)
    }

    fn seq_entries(entries: &[LogEntry], first_seq: u64) -> Vec<(u64, &LogEntry)> {
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (first_seq + index as u64, entry))
            .collect()
    }

    #[test]
    fn wrapped_lines_carry_source_offsets_that_reconstruct_the_message() {
        let message: String = ('a'..='z').cycle().take(300).collect();
        let wrapped = wrap_log_message_with_offsets(&message, 40, 10, 19);

        assert!(wrapped.len() > 1);
        assert_eq!(wrapped[0].0, 0);
        let rebuilt: String = wrapped.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(rebuilt, message);

        // Offsets must address the exact byte position each line starts at.
        for (offset, text) in &wrapped {
            assert!(message.is_char_boundary(*offset));
            assert!(message[*offset..].starts_with(text.as_str()));
        }
    }

    #[test]
    fn wrapped_multibyte_lines_keep_char_boundaries_and_display_width() {
        let message = "漢字と絵文字🎉".repeat(20);
        let wrapped = wrap_log_message_with_offsets(&message, 21, 0, 9);

        let rebuilt: String = wrapped.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(rebuilt, message);
        for (offset, text) in &wrapped {
            assert!(message.is_char_boundary(*offset));
            assert!(unicode_width::UnicodeWidthStr::width(text.as_str()) <= 30);
        }
    }

    #[test]
    fn one_oversized_entry_produces_addressable_display_lines() {
        let long = "x".repeat(400);
        let entries = vec![entry(&long)];
        let lines = build_log_display_lines(&seq_entries(&entries, 0), 40);

        // 40 columns - 2 borders - 9 timestamp = 29 message columns.
        assert!(lines.len() > 10);
        assert!(lines[0].is_first);
        assert!(!lines[1].is_first);
        // First line is narrower than continuation lines, which reclaim the
        // timestamp columns.
        assert_eq!(lines[0].text.chars().count(), 29);
        assert_eq!(lines[1].text.chars().count(), 38);
    }

    #[test]
    fn anchor_projection_addresses_lines_inside_one_entry() {
        let long = "x".repeat(400);
        let entries = vec![entry(&long)];
        let lines = build_log_display_lines(&seq_entries(&entries, 0), 40);

        let third = anchor_at_line(&lines, 2).unwrap();
        assert_eq!(third.entry_seq, 0);
        assert_eq!(project_anchor_to_line(&lines, third), 2);
    }

    #[test]
    fn anchor_reprojects_to_the_same_source_position_after_a_width_change() {
        let long = "x".repeat(400);
        let entries = vec![entry(&long)];
        let narrow = build_log_display_lines(&seq_entries(&entries, 0), 40);

        let anchor = anchor_at_line(&narrow, 4).unwrap();
        let wide = build_log_display_lines(&seq_entries(&entries, 0), 120);
        let projected = project_anchor_to_line(&wide, anchor);

        // The wider layout has fewer lines, so the anchored source byte must land
        // on the line that still contains it.
        let line = &wide[projected];
        assert!(line.source_byte_offset <= anchor.source_byte_offset);
        let line_end = line.source_byte_offset + line.text.len();
        assert!(anchor.source_byte_offset < line_end);
    }

    #[test]
    fn trimmed_anchor_clamps_to_the_oldest_surviving_line() {
        let entries = [entry("first"), entry("second"), entry("third")];
        // Buffer trimmed: sequence numbers 0 and 1 are gone.
        let surviving = seq_entries(&entries[2..], 2);
        let lines = build_log_display_lines(&surviving, 60);

        let stale = LogViewAnchor {
            entry_seq: 0,
            source_byte_offset: 0,
        };
        assert_eq!(project_anchor_to_line(&lines, stale), 0);
    }

    #[test]
    fn anchor_newer_than_every_surviving_line_clamps_to_the_last_line() {
        let entries = vec![entry("only")];
        let lines = build_log_display_lines(&seq_entries(&entries, 0), 60);

        let stale = LogViewAnchor {
            entry_seq: 99,
            source_byte_offset: 0,
        };
        assert_eq!(project_anchor_to_line(&lines, stale), lines.len() - 1);
    }

    #[test]
    fn start_line_follows_newest_without_an_anchor_and_never_overruns() {
        let entries: Vec<LogEntry> = (0..10).map(|i| entry(&format!("line {i}"))).collect();
        let lines = build_log_display_lines(&seq_entries(&entries, 0), 60);
        assert_eq!(lines.len(), 10);

        assert_eq!(resolve_start_line(&lines, None, 4), 6);
        assert_eq!(resolve_start_line(&lines, None, 20), 0);

        let stale = LogViewAnchor {
            entry_seq: 9,
            source_byte_offset: 0,
        };
        // Anchoring the last line would show a single row; clamp to the last full page.
        assert_eq!(resolve_start_line(&lines, Some(stale), 4), 6);
    }

    #[test]
    fn empty_log_sequence_resolves_deterministically() {
        let lines: Vec<LogDisplayLine> = Vec::new();
        assert_eq!(resolve_start_line(&lines, None, 5), 0);
        assert_eq!(
            resolve_start_line(
                &lines,
                Some(LogViewAnchor {
                    entry_seq: 3,
                    source_byte_offset: 7
                }),
                5
            ),
            0
        );
        assert!(anchor_at_line(&lines, 0).is_none());
    }

    #[test]
    fn logs_panel_header_matches_the_logs_view_format() {
        let mut with_all = entry("m");
        with_all.change_id = Some("alpha".to_string());
        with_all.operation = Some("archive".to_string());
        with_all.iteration = Some(2);
        assert_eq!(logs_panel_header(&with_all), "[alpha:archive:2] ");

        let mut analysis = entry("m");
        analysis.operation = Some("analysis".to_string());
        assert_eq!(logs_panel_header(&analysis), "[analysis:1] ");

        assert_eq!(logs_panel_header(&entry("m")), "");
    }

    #[test]
    fn toggle_logs_panel_flips_visibility_flag() {
        assert!(toggle_logs_panel(false));
        assert!(!toggle_logs_panel(true));
    }

    #[test]
    fn toggle_selected_proposal_log_filter_flips_flag() {
        assert!(toggle_selected_proposal_log_filter(false));
        assert!(!toggle_selected_proposal_log_filter(true));
    }

    #[test]
    fn cursor_move_resets_log_position_only_when_active_target_changes() {
        assert!(should_reset_log_position_for_target_change(
            true,
            Some("alpha"),
            Some("beta")
        ));
        assert!(!should_reset_log_position_for_target_change(
            true,
            Some("alpha"),
            Some("alpha")
        ));
        assert!(!should_reset_log_position_for_target_change(
            false,
            Some("alpha"),
            Some("beta")
        ));
    }

    #[test]
    fn cursor_move_resets_log_position_when_target_becomes_unavailable() {
        assert!(should_reset_log_position_for_target_change(
            true,
            Some("alpha"),
            None
        ));
        assert!(!should_reset_log_position_for_target_change(
            true, None, None
        ));
    }

    #[test]
    fn disabled_filter_keeps_every_entry_eligible() {
        assert!(log_entry_matches_selected_proposal(
            false,
            Some("beta"),
            Some("alpha")
        ));
        assert!(log_entry_matches_selected_proposal(false, None, None));
    }

    #[test]
    fn enabled_filter_matches_only_exact_structured_change_id() {
        assert!(log_entry_matches_selected_proposal(
            true,
            Some("alpha"),
            Some("alpha")
        ));
        assert!(!log_entry_matches_selected_proposal(
            true,
            Some("beta"),
            Some("alpha")
        ));
        // Remote project-only identity must not be promoted to proposal identity.
        assert!(!log_entry_matches_selected_proposal(
            true,
            Some("project-1"),
            Some("project-1::demo/alpha")
        ));
    }

    #[test]
    fn enabled_filter_excludes_unscoped_and_targetless_entries() {
        assert!(!log_entry_matches_selected_proposal(
            true,
            None,
            Some("alpha")
        ));
        assert!(!log_entry_matches_selected_proposal(
            true,
            Some("alpha"),
            None
        ));
        assert!(!log_entry_matches_selected_proposal(true, None, None));
    }
}
