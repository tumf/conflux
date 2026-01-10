# Tasks: remove-completed-mode

## Implementation Tasks

- [ ] Remove `AppMode::Completed` variant from enum in `src/tui/state.rs`
- [ ] Update `AllCompleted` event handler: set `mode = AppMode::Select` instead of `Completed`
- [ ] Remove `Completed` branch from `render_header` mode text/color in `src/tui/render.rs`
- [ ] Merge `toggle_selection` Completed case into Select case in `src/tui/state.rs`
- [ ] Update `start_processing` condition: remove `Completed` check (Select only)
- [ ] Merge `toggle_approval` Completed case into Select case
- [ ] Update render dispatch: use log existence for layout, not mode
- [ ] Update `render_status` to show completion message in Select mode when appropriate
- [ ] Add success log when returning to Select mode after completion

## Spec Updates

- [ ] Update `tui-editor/spec.md` to remove "Completed mode" scenarios
- [ ] Add spec for log-based layout rendering

## Validation

- [ ] Run `cargo build` - no compilation errors
- [ ] Run `cargo test` - all tests pass
- [ ] Run `cargo clippy` - no warnings
