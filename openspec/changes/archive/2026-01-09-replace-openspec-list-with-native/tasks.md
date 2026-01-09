# Tasks

## Implementation

1. [x] Add `list_changes_native()` function in `src/openspec.rs`
   - Read `openspec/changes` directory
   - Filter for directories only
   - Parse each change's `tasks.md` using `task_parser::parse_change()`
   - Return `Vec<Change>` with id, completed_tasks, total_tasks

2. [x] Replace in `src/main.rs` (2 locations)
   - Line 35: TUI default mode initial changes
   - Line 47: TUI subcommand initial changes

3. [x] Replace in `src/tui.rs` (4 locations)
   - Line 725: auto refresh task
   - Line 1038: `archive_all_complete_changes` function
   - Line 1209: Phase 2 change selection
   - Line 1404: final verification

4. [x] Add unit tests for `list_changes_native()`
   - Test with empty changes directory
   - Test with valid changes
   - Test with missing tasks.md

5. [x] Run existing tests to verify no regression
