# Tasks: Fix Footer Progress Tracking

## Implementation Tasks

### 1. Update Progress Calculation Filter
- [x] Modify `render_status` function in `src/tui.rs` (lines 1603-1616)
- [x] Change filter to include `Queued`, `Processing`, `Completed`, and `Archived` statuses
- [x] Exclude only `NotQueued` and `Error` statuses

### 2. Add Unit Tests
- [x] Add test case `test_progress_calculation_includes_completed_changes`
- [x] Add test case `test_progress_calculation_includes_archived_changes`
- [x] Verify existing tests still pass

### 3. Manual Verification
- [x] Run TUI with multiple changes
- [x] Verify progress bar shows cumulative progress
- [x] Verify completed tasks remain in progress calculation
- [x] Verify archived tasks remain in progress calculation

## Validation

- [x] Run `cargo test` to verify all tests pass
- [x] Run `cargo clippy` to verify no new warnings
- [x] Manually test TUI behavior with real changes
