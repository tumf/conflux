# Implementation Tasks

## 1. DynamicQueue Extension
- [x] 1.1 Add `remove` method to `src/tui/queue.rs` (remove specified ID from queue)
- [x] 1.2 Add unit tests for `remove` method (normal removal, non-existent ID, multiple removals, etc.)

## 2. TUI Command Processing Fix
- [x] 2.1 Modify `src/tui/runner.rs` `TuiCommand::UnapproveAndDequeue` to access dynamic_queue
- [x] 2.2 Call `dynamic_queue.remove()` during unapprove processing
- [x] 2.3 Add removal log message

## 3. Space Key Processing Fix
- [x] 3.1 Enable `src/tui/state/mod.rs` `toggle_selection` method to reference dynamic_queue when removing from queue
- [x] 3.2 When changing from `QueueStatus::Queued` to `NotQueued`, also remove from dynamic_queue
- [x] 3.3 Log successful removal

## 4. Testing and Verification
- [x] 4.1 Verify DynamicQueue unit tests pass (`cargo test queue`)
- [x] 4.2 Manual test: During sequence mode execution, change [x] to [@] and verify it's not executed
- [x] 4.3 Manual test: Remove from queue with Space key and verify it's not executed
- [x] 4.4 Run all tests (`cargo test`)
