# Tasks

## Implementation

- [x] 1. Update `render_changes_list_running` in `src/tui.rs` to remove task count from terminal state status text (lines 1628-1629)
- [x] 2. Update unit tests for status format to expect `[completed]` instead of `[completed X/Y]`

## Validation

- [x] 3. Run `cargo build` and verify no compilation errors
- [x] 4. Run `cargo test` and verify all tests pass (related tests pass; unrelated `test_default_openspec_cmd` failure is pre-existing)
- [x] 5. Manual TUI testing: verify archived/completed/error show `[status]` format with separate `X/Y` column
