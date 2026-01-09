# Proposal: Add Task Count to Terminal Status Display

## Change ID
`add-task-count-to-status`

## Why

In the TUI running mode, the status display for `completed`, `archived`, and `error` states shows only the status text (e.g., `[completed]`, `[archived]`, `[error]`) without the task count. Meanwhile, the `processing` state shows `⠋ [70%]` with progress percentage. Users want consistent visibility of task counts across all terminal states for better progress tracking and verification.

Currently:
- `processing`: `⠋ [ 70%]` (shows progress)
- `completed`: `[completed]` (no task count)
- `archived`: `[archived]` (no task count)
- `error`: `[error]` (no task count)

The task count (`8/13`) is displayed in a separate column to the right, but embedding it directly in the status display would provide clearer at-a-glance information.

## What Changes

- Modify the status text format in `render_changes_list_running()` to include task count for terminal states
- New format:
  - `completed`: `[completed 8/13]`
  - `archived`: `[archived 8/13]`
  - `error`: `[error 8/13]`
  - `processing`: Keep current `⠋ [ 70%]` format (progress percentage is more relevant during active processing)

## Impact

- Affected specs: `specs/cli/spec.md` (TUI Display requirements)
- Affected code: `src/tui.rs` (lines 1533-1538 in `render_changes_list_running`)
- Risk: Low - display formatting change only
- Breaking Changes: None - visual enhancement only
