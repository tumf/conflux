# Tasks: Open Editor in Change Directory

## Implementation Tasks

- [ ] 1. Add `launch_editor_for_change` helper function to `src/tui.rs`
  - Get `$EDITOR` environment variable with `vi` fallback
  - Construct change directory path
  - Validate directory exists
  - Execute editor process with current directory set

- [ ] 2. Add `KeyCode::Char('e')` handler in key event match
  - Check app.mode == AppMode::Select
  - Get current change from cursor position
  - Disable raw mode and leave alternate screen
  - Call `launch_editor_for_change`
  - Re-enable raw mode and enter alternate screen
  - Clear and redraw terminal

- [ ] 3. Update help text in `render_changes_list`
  - Add `e: edit` to the selection mode help string
  - Update title from `" Changes (↑↓/jk: move, Space: queue, @: approve, F5: run, q: quit) "` to include `e: edit`

- [ ] 4. Add error type `EditorLaunchFailed` to `src/error.rs`
  - New variant for editor launch errors
  - Implement Display for error message

- [ ] 5. Add unit tests for editor launch logic
  - Test EDITOR variable parsing
  - Test directory path construction
  - Test mode check (Select mode only)

## Validation Tasks

- [ ] 6. Manual testing: verify editor launches correctly with various editors
  - Test with `EDITOR=vim`
  - Test with `EDITOR=nvim`
  - Test with `EDITOR=code --wait`
  - Test without EDITOR (vi fallback)

- [ ] 7. Run `cargo test` to ensure no regressions
- [ ] 8. Run `cargo clippy` to ensure code quality
