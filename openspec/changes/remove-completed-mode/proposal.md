# Proposal: remove-completed-mode

## Summary

Remove the `Completed` mode from TUI and return to `Select` mode after all processing completes.

## Problem

Currently, after all queued changes are processed, the TUI transitions to `Completed` mode. This mode:
- Restricts editor launch (`e` key) unnecessarily
- Creates an additional state that serves no distinct purpose from `Select` mode
- Forces users to recognize yet another mode when they simply want to continue working

## Solution

Remove `AppMode::Completed` and transition directly to `Select` mode when all processing completes. This provides:
- Immediate ability to select and queue more changes
- Editor access (`e` key) right after completion
- Simpler mode state machine
- Consistent UX - users stay in the familiar Select mode

## Scope

- **In scope**: Remove `Completed` mode, update transitions, update specs
- **Out of scope**: Other mode changes (Running, Stopped, Error)

## Risk Assessment

- **Low risk**: This simplifies the codebase rather than adding complexity
- **Backward compatible**: No configuration changes needed
