# Tasks

## Implementation

- [x] 1.1. Add `ApproveOnly(String)` variant to `TuiCommand` enum in `tui.rs`
- [x] 1.2. Update `toggle_approval` method to return `ApproveOnly` instead of `ApproveAndQueue` when in Running mode
- [x] 1.3. Add handler for `TuiCommand::ApproveOnly` in main event loop (same as `ToggleApproval` but without queue addition)

## Testing

- [x] 2.1. Add unit test for Running mode approval transition: `[ ]` → `[@]`
- [x] 2.2. Add unit test to verify Select mode still returns `ToggleApproval`
- [x] 2.3. Add unit test to verify Completed mode still auto-queues: `[ ]` → `[x]`
- [x] 2.4. Add unit test to verify Stopped mode still auto-queues: `[ ]` → `[x]`
- [x] 2.5. Run full test suite and verify no regressions
