# Tasks

## Issue 1: Dynamic Key Hints

- [ ] Update `render_changes_list_select` key hint logic
  - Hide Space/@/e when `app.changes.is_empty()`
  - Hide F5 when no changes are queued
  - Current implementation at render.rs:192-211 partially handles this

- [ ] Update `render_changes_list_running` to use dynamic key hints
  - Currently uses hardcoded hint string at line 300
  - Apply same conditional logic as select mode

## Issue 2: Approval State Transition Regression

- [ ] Debug `ApproveAndQueue` handler in runner.rs:329-358
  - Verify `approval::approve_change()` is being called
  - Verify `update_approval_status()` finds the change
  - Verify `selected = true` line is executed
  - Add debug logging if needed

- [ ] Investigate channel communication
  - `cmd_tx.send()` at runner.rs:145 may not be received
  - `cmd_rx.try_recv()` at runner.rs:315 may not process the command
  - Check if there's a timing issue

- [ ] Fix root cause of state transition failure
  - Ensure `[ ]` → `@` → `[x]` in Select mode
  - Maintain `[ ]` → `@` → `[@]` in Running mode (current behavior is correct here)

## Validation

- [ ] Run `cargo fmt && cargo clippy && cargo test`
- [ ] Manual testing:
  - Test with empty changes list (no Space/@/e/F5 shown)
  - Test `@` on unapproved change → should show `[x]` and log message
  - Test in running mode → `@` should show `[@]`
- [ ] Verify logs appear when pressing `@`
