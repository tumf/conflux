# Tasks

## Implementation Tasks

- [x] Create `src/task_parser.rs` module with regex-based task parsing
- [x] Implement `TaskProgress` struct with `completed` and `total` fields
- [x] Implement `parse_content()` function to parse task markdown content
- [x] Implement `parse_file()` function to read and parse tasks.md files
- [x] Implement `parse_change()` function to locate and parse change's tasks.md
- [x] Update `src/openspec.rs` to use native parsing when CLI returns 0/0

## Testing Tasks

- [x] Add unit tests for bullet list format (`- [ ]`, `- [x]`)
- [x] Add unit tests for numbered list format (`1. [ ]`, `1. [x]`)
- [x] Add unit tests for mixed format (bullets and numbers)
- [x] Add unit tests for edge cases (indented items, headers, inline checkboxes)
- [x] Add integration test for fallback behavior

## Validation

- [x] Run `cargo test` to verify all tests pass
- [x] Run `cargo clippy` to check for warnings
- [x] Test with real openspec changes directory
- [x] Verify TUI displays correct task counts

## Dependencies

- Tasks 1-6 are sequential (module structure)
- Tasks 7-11 can run in parallel after Task 3
- Tasks 12-15 depend on all implementation and testing tasks
