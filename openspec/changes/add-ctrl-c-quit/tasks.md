# Tasks: Add Ctrl+C as TUI Quit Shortcut

## Implementation Tasks

1. [x] Update `src/tui.rs` imports to include `KeyModifiers`
2. [x] Add `Ctrl+C` key handling alongside existing `q` key handler
3. [x] Update help text if displayed in TUI to show both shortcuts
4. [x] Verify existing `q` key behavior remains unchanged
5. [x] Test `Ctrl+C` quit functionality in selection mode
6. [x] Test `Ctrl+C` quit functionality in execution mode

## Validation

- [x] Run `cargo test` to ensure no regressions (113 tests passed)
- Manual test: Launch TUI, press `Ctrl+C`, verify clean exit
- Manual test: Launch TUI, press `q`, verify clean exit (unchanged behavior)
