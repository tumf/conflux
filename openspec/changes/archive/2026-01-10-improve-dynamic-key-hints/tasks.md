# Tasks

## Issue 1: Dynamic Key Hints

- [x] Update `render_changes_list_select` key hint logic
  - Hide Space/@/e when `app.changes.is_empty()`
  - Hide F5 when no changes are queued
  - Already implemented at render.rs:192-218

- [x] Update `render_changes_list_running` to use dynamic key hints
  - Applied same conditional logic as select mode
  - render.rs:296-327

## Issue 2: Approval State Transition Regression

- [x] Add tests for ApproveAndQueue state transition
  - `test_approve_and_queue_in_select_mode_returns_correct_command`
  - `test_approve_and_queue_state_update_simulation`
  - Tests verify correct state after ApproveAndQueue handler execution

## Validation

- [x] Run `cargo fmt && cargo clippy && cargo test`
  - All 202 tests pass
  - No clippy warnings
