# Proposal: Move Version Display to Header

## Summary

Move the application version display from the footer to the right side of the header for better visibility and cleaner footer layout.

## Background

Currently, the TUI displays the version number (e.g., "v0.1.0") in the right side of the footer area. This design has a few drawbacks:

1. **Footer crowding**: The footer already contains status information (selected count, new count, guidance messages), making it visually busy
2. **Low visibility**: Version information in the footer is often overlooked
3. **Layout complexity**: The footer uses horizontal split layout to accommodate version on the right

## Proposal

Move the version display to the right side of the header:
- Header left: "OpenSpec Orchestrator  [Mode]"
- Header right: "v0.1.0"

This matches common TUI patterns where version information appears in the title bar/header area.

## Benefits

1. **Cleaner footer**: Simplifies footer layout to focus on status and guidance
2. **Better visibility**: Version is more prominent in the header
3. **Consistent design**: Header naturally supports split layout for title/version
4. **Simplified code**: Footer rendering logic becomes simpler

## Scope

- Modify `render_header()` function to include version on the right
- Modify `render_footer_select()` function to remove version display
- Update related tests

## Non-Goals

- Changing the version format (stays as "vX.Y.Z")
- Adding version to running mode header (only selection mode for now)
- Changing version color styling

## Risk Assessment

- **Low risk**: UI-only change with no functional impact
- **Testing**: Visual verification required in TUI
