# Proposal: remove-completed-mode

## Summary

Remove `AppMode::Completed` and return to `Select` mode after all processing completes. Log display is determined by log existence, not by mode.

## Problem

`Completed` mode is functionally identical to `Select` mode:
- Both allow queue operations (Space)
- Both allow approval operations (@)
- Both allow starting processing (F5)
- toggle_selection logic difference is artificial, not essential

The only visible difference is log display, but this should be based on whether logs exist, not on mode.

## Solution

1. Remove `AppMode::Completed` variant
2. Return to `Select` mode when all processing completes
3. Change render layout logic: show logs panel when `!app.logs.is_empty()`, regardless of mode

## Behavior Change

| Before | After |
|--------|-------|
| Processing completes → Completed mode | Processing completes → Select mode |
| Log panel shown only in Running/Completed/Stopped/Error modes | Log panel shown when logs exist |

## Scope

- **In scope**: Remove Completed mode, update render logic for log display
- **Out of scope**: Other mode changes (Running, Stopped, Error)
