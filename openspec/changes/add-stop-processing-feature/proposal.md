# Proposal: Add Stop Processing Feature

## Summary

Add the ability to stop ongoing processing in TUI mode using the Escape key. This provides users with control to halt orchestration without fully quitting the application.

## Problem Statement

Currently, users can only quit the entire TUI application using `q` or `Ctrl+C`. There is no way to:
- Stop current processing while staying in the TUI
- Return to selection mode to modify the queue
- Gracefully stop after the current change completes vs immediately force-stop

## Proposed Solution

Add an Escape key binding in TUI running mode that:
1. **First press (Graceful Stop)**: Stops accepting new changes and completes the current agent process
2. **Second press (Force Stop)**: Immediately kills the current agent process

After stopping, the TUI transitions to a "Stopped" state where users can:
- Review what was completed
- Modify the queue
- Resume processing with F5

## Scope

### In Scope
- TUI running mode stop functionality
- Graceful and force stop modes
- Status display for stopping/stopped states
- Help text updates

### Out of Scope
- CLI `run` subcommand stop behavior (already handles Ctrl+C)
- External stop command from another terminal
- Pause/resume functionality (different from stop)

## Spec Deltas

### cli/spec.md
- Add requirement for Escape key stop behavior
- Add requirement for stopping states display
- Add requirement for stopped mode queue management

## Success Criteria

1. Users can press Esc during processing to initiate graceful stop
2. Users can press Esc twice to force stop immediately
3. After stopping, users can modify queue and resume with F5
4. Help text clearly indicates stop functionality
