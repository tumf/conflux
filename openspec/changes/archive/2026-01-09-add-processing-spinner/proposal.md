# Proposal: add-processing-spinner

## Summary

Add spinner animation to items with `QueueStatus::Processing` in the TUI's running mode to provide visual feedback that processing is active.

## Problem

In running mode, items being processed show only a static `[XX%]` indicator. Users cannot easily tell if processing is actively running or stalled.

## Solution

Add an animated spinner next to processing items:

- Spinner characters: `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` (Braille dot pattern)
- Update interval: 100ms (leveraging existing event polling)
- Display format: `⠋ [XX%]` - spinner followed by progress percentage

### Implementation

1. Add `spinner_frame: usize` field to `AppState`
2. Increment spinner frame on each render cycle (every 100ms)
3. Display spinner character for `QueueStatus::Processing` items in `render_changes_list_running`

## Scope

- **Modified**: `src/tui.rs` - Add spinner state and rendering logic
- **Modified**: CLI spec - Add spinner display requirement

## Dependencies

None

## Risks

- Low: UI-only change, no impact on processing logic
- Spinner updates use existing polling interval, no additional timers needed
