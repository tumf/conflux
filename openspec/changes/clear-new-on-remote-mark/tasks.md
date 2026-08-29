## Implementation Tasks

- [ ] Add regression coverage that drives operator execution-mark mutations through the shared remote command service and then projects them into a TUI containing NEW rows; cover mark, unmark, unchanged/no-op, unrelated rows, and lifecycle/system revocation. (verification: integration - `cargo test tui::state::execution_mark_tests --lib`; verification-id: remote-mark-new-tests)
- [ ] Carry target-scoped operator interaction identity from settled shared mark commands to TUI projection so external mark or unmark acknowledgement clears only the interacted row's `is_new` state and updates `new_change_count`. (verification: integration - `cargo test tui::state::execution_mark_tests --lib`; verification-id: remote-mark-new-tests)
- [ ] Preserve TUI Space/bulk behavior and prove passive synchronization, rejected rows, queue intent, retry, lifecycle state, and unrelated NEW rows remain unchanged. (verification: integration - `cargo test tui::state::execution_mark_tests --lib`; verification-id: remote-mark-new-tests)

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected commands:

`cflx openspec validate clear-new-on-remote-mark --strict --evidence warn`

`cflx openspec validate clear-new-on-remote-mark --archive-gate`
