# Tasks: Fix TUI Complete Status Without 100% Tasks

## Implementation Tasks

### 1. Update ProcessingCompleted Event Logic
- [x] Move `ProcessingCompleted` event sending to after task completion check in `run_orchestrator()` (tui.rs:1100-1103)
- [x] Only send `ProcessingCompleted` when `is_complete()` returns true
- [x] Add log entry when apply succeeds but tasks not yet complete

### 2. Update Queue Status Handling
- [x] Keep change in `Processing` status when apply succeeds but tasks incomplete
- [x] Ensure next iteration picks up partially complete changes
- [x] Verify change transitions to `Completed` only at 100%

### 3. Add Unit Tests
- [x] Add test case `test_apply_success_without_100_percent_stays_processing`
- [x] Add test case `test_apply_success_with_100_percent_completes`
- [x] Add test case `test_incomplete_change_continues_processing`

### 4. Manual Verification
- [x] Run TUI with a change that requires multiple apply iterations
- [x] Verify status shows "processing" during incomplete state
- [x] Verify status shows "completed" only when tasks are 100%
- [x] Verify archiving happens correctly after 100% completion

## Validation

- [x] Run `cargo test` to verify all tests pass
- [x] Run `cargo clippy` to verify no new warnings
- [x] Manually test TUI behavior with real changes
