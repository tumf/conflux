## 1. Implementation
- [x] 1.1 Adjust guards/state updates to allow toggling only the execution mark with Space operation on `MergeWait`/`ResolveWait` (`src/tui/state/guards.rs`).
  - **Verify**: Confirm that `handle_toggle_running_mode` and `handle_toggle_stopped_mode` in `src/tui/state/guards.rs` toggle only `selected` without triggering `queue_status` or DynamicQueue operations.
- [x] 1.2 Remove complete block on Space operation for `ResolveWait` while maintaining queue state invariance.
  - **Verify**: Confirm that `validate_change_toggleable` in `src/tui/state/guards.rs` allows Space for `ResolveWait` while `QueueStatus` remains unchanged.
- [x] 1.3 Add/update unit tests corresponding to the changed behavior.
  - **Verify**: Run `cargo test tui::state` or relevant tests to confirm that toggling `MergeWait`/`ResolveWait` changes only `selected`.

- [x] 1.4 Allow @ operation on `MergeWait`/`ResolveWait` to toggle only approval state (queue state and DynamicQueue remain unchanged).
  - **Verify**: Confirm that `src/tui/state/mod.rs` does not block `ResolveWait`, and that approval/unapproval in wait states has no queue side effects in `src/tui/command_handlers.rs`.
- [x] 1.5 Add/update unit tests corresponding to the changed behavior (@ operation in wait states).
  - **Verify**: Run `cargo test tui::state` to confirm that @ operation on `ResolveWait`/`MergeWait` returns `UnapproveOnly`.
