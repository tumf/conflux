# Tasks: remove-completed-mode

## Implementation Tasks

- [ ] Remove `AppMode::Completed` variant from enum in `src/tui.rs`
- [ ] Update `AllCompleted` event handler to set `mode = AppMode::Select` instead of `Completed`
- [ ] Remove `Completed` branch from `render_header` mode text
- [ ] Remove `Completed` branch from mode color mapping
- [ ] Update `toggle_selection` to remove `Completed` case (merge with `Select`)
- [ ] Update `toggle_approval` to remove `Completed` case (merge with `Select`)
- [ ] Update render dispatch to use `render_select_mode` for completed state
- [ ] Update `render_status` to handle post-completion Select mode appropriately
- [ ] Add success message/log when returning to Select mode after completion

## Spec Updates

- [ ] Update `tui-editor/spec.md` to remove "Completed mode" scenarios
- [ ] Verify no other specs reference `Completed` mode

## Validation

- [ ] Run `cargo build` - no compilation errors
- [ ] Run `cargo test` - all tests pass
- [ ] Run `cargo clippy` - no warnings
- [ ] Manual test: queue changes, run, verify returns to Select mode
- [ ] Manual test: verify `e` key works after completion
