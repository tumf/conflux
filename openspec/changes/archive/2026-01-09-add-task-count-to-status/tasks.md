# Tasks: Add Task Count to Terminal Status Display

## 1. Implementation

- [x] 1.1 Update `render_changes_list_running()` in `src/tui.rs` to include task count in status text for terminal states (`completed`, `archived`, `error`)
- [x] 1.2 Adjust column width formatting to accommodate longer status strings
- [x] 1.3 Add unit tests for the new status text format

## 2. Validation

- [x] 2.1 Run existing TUI tests to ensure no regressions
- [x] 2.2 Manually verify status display with sample changes in each terminal state
- [x] 2.3 Run `cargo clippy` and `cargo fmt` to ensure code quality
