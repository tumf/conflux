# Change: Add Log Panel Scroll Feature

## Why
Currently, the TUI log panel only displays the most recent log entries that fit within the visible area. Users cannot scroll back to view older log messages, which makes it difficult to review past events or debug issues.

## What Changes
- Add scroll state tracking to AppState for log panel position
- Implement keyboard navigation (Page Up/Down, arrow keys) for log scrolling
- Add visual scroll indicator when logs exceed visible area
- Auto-scroll to bottom on new log entries (unless user is reviewing history)

## Impact
- Affected specs: `specs/cli/spec.md`
- Affected code: `src/tui.rs` (AppState, render_logs, key handling)
