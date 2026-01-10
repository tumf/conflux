# Proposal: Add Checkbox Display and Approval Toggle in Running Mode

## Summary

Enhance TUI running/completed mode to display checkbox status indicators (`[ ]`, `[@]`, `[x]`) and allow approval toggling for non-processing changes.

## Motivation

Currently, the TUI has different display behaviors between selection mode and running mode:

1. **Selection mode**: Shows `[ ]` (unapproved), `[@]` (approved, not queued), `[x]` (queued)
2. **Running mode**: Only shows queue status text like `[queued]`, `[processing]`, etc.

This inconsistency makes it difficult for users to understand the approval and selection state of changes during execution. Additionally, users cannot approve/unapprove changes once they enter running mode, limiting workflow flexibility.

## Proposed Changes

### 1. Display checkbox in Running/Completed mode

Add the same checkbox indicators (`[ ]`, `[@]`, `[x]`) to the running mode change list, providing consistent visual feedback about approval and queue status.

### 2. Enable approval toggle in Running/Completed mode

Allow users to press `@` key to toggle approval status for changes that are NOT currently being processed (i.e., `NotQueued`, `Queued`, `Completed`, `Archived`, `Error` states).

## Scope

- **In scope**:
  - Modify `render_changes_list_running` to display checkbox indicators
  - Modify `toggle_approval` to work in Running/Completed modes
  - Update help text to reflect available operations

- **Out of scope**:
  - Approval toggle for `Processing` state changes (blocked for safety)
  - Changes to CLI run mode behavior

## Success Criteria

1. Running mode displays `[ ]`, `[@]`, `[x]` indicators alongside queue status
2. `@` key toggles approval in Running/Completed mode for non-processing changes
3. Processing changes cannot have their approval toggled (shows warning)
4. All existing tests pass
