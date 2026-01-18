# Manual Verification Guide

This document describes how to manually verify the implementation of warning logs for Enter key operations in the TUI Worktrees view.

## Prerequisites

- Build the project: `cargo build --release`
- Have a test repository with OpenSpec changes
- Have a terminal with TUI support

## Test Case 2.1: Warning logs when Enter is pressed in Worktrees view

### Scenario 1: Not in Worktrees view
1. Run TUI: `cargo run --release -- tui`
2. Stay in Changes view (default view)
3. Press `Enter` key
4. **Expected**: Log panel shows warning: "Enter ignored: not in Worktrees view"

### Scenario 2: No worktree selected
1. Run TUI: `cargo run --release -- tui`
2. Press `Tab` to switch to Worktrees view
3. If worktree list is empty, press `Enter`
4. **Expected**: Log panel shows warning: "Enter ignored: no worktree selected"

### Scenario 3: worktree_command not configured
1. Ensure `.cflx.jsonc` does NOT have `worktree_command` configured
2. Run TUI: `cargo run --release -- tui`
3. Press `Tab` to switch to Worktrees view
4. If worktrees exist, select one and press `Enter`
5. **Expected**: Log panel shows warning: "Enter ignored: worktree_command not configured"

## Test Case 2.2: Enter execution continues when worktree_command is configured

### Scenario: Successful worktree command execution
1. Configure `.cflx.jsonc` with:
   ```jsonc
   {
     "worktree_command": "echo 'Test command in ${WORKTREE_PATH}'"
   }
   ```
2. Run TUI: `cargo run --release -- tui`
3. Press `Tab` to switch to Worktrees view
4. Select a worktree with arrow keys or j/k
5. Press `Enter`
6. **Expected**:
   - TUI suspends and shows command output
   - Command executes in the worktree directory
   - After command exits, TUI resumes
   - Log panel shows info: "Running worktree command in <path>"
   - Log panel shows success: "Worktree command completed successfully"

## Log Color Verification

The warning logs should appear with yellow/warning color in the TUI log panel, distinguishing them from:
- **Info logs** (white/default): "Running worktree command in <path>"
- **Success logs** (green): "Worktree command completed successfully"
- **Error logs** (red): "Failed to execute worktree command: <error>"

## Implementation Details

The implementation adds warning logs at three early-exit points in the Enter key handler:
- `src/tui/runner.rs:742` - Not in Worktrees view
- `src/tui/runner.rs:747` - No worktree selected
- `src/tui/runner.rs:752` - worktree_command not configured

These logs appear before any execution begins, providing immediate feedback to users about why Enter had no effect.
