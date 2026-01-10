# Tasks: remove-completed-mode

## Implementation Tasks

- [x] Remove `AppMode::Completed` variant from enum in `src/tui/types.rs`
- [x] Update `AllCompleted` event handler: set `mode = AppMode::Select` instead of `Completed`
- [x] Remove `Completed` branch from `render_header` mode text/color in `src/tui/render.rs`
- [x] Merge `toggle_selection` Completed case into Select case in `src/tui/state.rs`
- [x] Update `start_processing` condition: remove `Completed` check (Select only)
- [x] Merge `toggle_approval` Completed case into Select case
- [x] Update render dispatch: use log existence for layout, not mode
- [x] Update `render_status` to show completion message in Select mode when appropriate
- [x] Add success log when returning to Select mode after completion

## Spec Updates

- [x] Update `tui-editor/spec.md` to remove "Completed mode" scenarios
- [x] Add spec for log-based layout rendering

## Validation

- [x] Run `cargo build` - no compilation errors
- [x] Run `cargo test` - all tests pass
- [x] Run `cargo clippy` - no warnings
