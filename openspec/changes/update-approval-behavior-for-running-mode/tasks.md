# Tasks

## Implementation

- [ ] 1.1. Add `ApproveOnly(String)` variant to `TuiCommand` enum in `tui.rs`
- [ ] 1.2. Update `toggle_approval` method to return `ApproveOnly` instead of `ApproveAndQueue` when in Running mode
- [ ] 1.3. Add handler for `TuiCommand::ApproveOnly` in main event loop (same as `ToggleApproval` but without queue addition)

## Testing

- [ ] 2.1. Add unit test for Running mode approval transition: `[ ]` → `[@]`
- [ ] 2.2. Add unit test to verify Select mode still auto-queues: `[ ]` → `[x]`
- [ ] 2.3. Add unit test to verify Completed mode still auto-queues: `[ ]` → `[x]`
- [ ] 2.4. Run full test suite and verify no regressions

## Validation

- [ ] 3.1. Manual test: Start TUI in Running mode, press `@` on unapproved change, verify `[@]` state
- [ ] 3.2. Manual test: Start TUI in Select mode, press `@` on unapproved change, verify `[x]` state
