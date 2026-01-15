# Tasks

- [x] 1. Add `QueueStatus::Merged` variant to `src/tui/types.rs`
  - Add `Merged` variant after `Archived` (around line 52)
  - Add `QueueStatus::Merged => "merged"` to `display()` method (around line 70)
  - Add `QueueStatus::Merged => Color::LightBlue` to `color()` method (around line 85)
  - Verify: `cargo build` succeeds
  - Verify: `cargo clippy` passes with no warnings

- [x] 2. Update `MergeCompleted` event handler in `src/tui/state/events.rs`
  - Change `change.queue_status = QueueStatus::Archived;` to `QueueStatus::Merged` (around line 156)
  - Verify: `cargo build` succeeds
  - Verify: Event processing works correctly

- [x] 3. Add `Merged` to progress update protection logic in `src/tui/state/events.rs`
  - Add `| QueueStatus::Merged` to terminal state check (around line 50)
  - Add `| QueueStatus::Merged` to Refresh progress update protection (around line 310)
  - Add `| QueueStatus::Merged` to terminal state judgment (around line 352)
  - Verify: `cargo build` succeeds
  - Verify: Progress updates stop at terminal state

- [x] 4. Add `Merged` support to UI rendering in `src/tui/render.rs`
  - Update `get_checkbox_display()` to handle `Merged` like `Archived` (around line 28)
  - Extend `is_archived` check to `matches!(change.queue_status, QueueStatus::Archived | QueueStatus::Merged)` (around lines 220, 386)
  - Add `| QueueStatus::Merged` to terminal state status display (around line 440)
  - Add `| QueueStatus::Merged` to "Done" judgment (around line 555)
  - Verify: `cargo build` succeeds
  - Verify: TUI displays status correctly

- [x] 5. Update test cases
  - Add `test_queue_status_merged_display` and `test_queue_status_merged_color` tests to `src/tui/types.rs`
  - Change test name to `test_merge_completed_sets_merged_status` in `src/tui/state/events.rs` (around line 549)
  - Change assertion to `QueueStatus::Merged` in `src/tui/state/events.rs` (around line 560)
  - Add `Merged` to terminal state tests (around lines 1000-1026 in `src/tui/state/events.rs`)
  - Verify: `cargo test` passes all tests
  - Verify: New tests work correctly

- [x] 6. Run full test suite and verify functionality
  - Run `cargo test` and verify all tests pass
  - Run `cargo clippy -- -D warnings` and verify it succeeds
  - Verify: `Merged` state displays correctly in TUI during parallel execution
  - Verify: `Archived` remains the final state in Serial mode
