# Proposal: Remove TUI Auto-Refresh Countdown

## Summary

Remove the auto-refresh countdown display (`Auto-refresh: Xs ↻`) from the TUI header. The countdown adds visual noise without providing meaningful value to the user.

## Motivation

The current TUI displays a countdown timer showing seconds until the next auto-refresh (e.g., `Auto-refresh: 5s ↻`). This information:

1. **Adds unnecessary visual clutter** - Users don't need to know exactly when the next refresh will occur
2. **Creates constant visual changes** - The countdown updates every second, which can be distracting
3. **Provides minimal actionable value** - Knowing "3 seconds until refresh" vs "5 seconds until refresh" doesn't change user behavior

The auto-refresh feature itself remains valuable and should continue to work; only the countdown display should be removed.

## Scope

- **In scope**: Remove countdown display from TUI header
- **Out of scope**: Changing auto-refresh interval or disabling auto-refresh functionality

## Impact

- **User Experience**: Cleaner, less distracting TUI header
- **Functionality**: No change to auto-refresh behavior
- **Code Changes**: Minimal - remove countdown calculation and display logic

## References

- Current implementation: `src/tui.rs:1432-1446`
- Related spec: `openspec/specs/cli/spec.md` (Requirement: 自動更新インジケーター)
