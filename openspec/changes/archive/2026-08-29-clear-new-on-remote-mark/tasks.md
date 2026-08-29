## Implementation Tasks

- [x] Add regression coverage that drives operator execution-mark mutations through the shared remote command service and then projects them into a TUI containing NEW rows; cover mark, unmark, unchanged/no-op, unrelated rows, and lifecycle/system revocation. (verification: integration - `cargo test tui::state::execution_mark_tests --lib`; verification-id: remote-mark-new-tests)
- [x] Carry target-scoped operator interaction identity from settled shared mark commands to TUI projection so external mark or unmark acknowledgement clears only the interacted row's `is_new` state and updates `new_change_count`. (verification: integration - `cargo test tui::state::execution_mark_tests --lib`; verification-id: remote-mark-new-tests)
- [x] Preserve TUI Space/bulk behavior and prove passive synchronization, rejected rows, queue intent, retry, lifecycle state, and unrelated NEW rows remain unchanged. (verification: integration - `cargo test tui::state::execution_mark_tests --lib`; verification-id: remote-mark-new-tests)

## Notes

- Interaction identity lives on `ExecutionMarkStore` as an unacknowledged-target set, recorded inside `arm_settlement` (`src/orchestration/operator_command.rs`). That entry point is already reached by exactly the accepted standalone operator writes and by nothing else — system revocation, refused/unchanged commands, and Start-admission writes all bypass it — so the acknowledgement rule and the settlement rule cannot drift apart.
- `AppState::sync_execution_marks_from_store` (`src/tui/state.rs`) drains that set and retires the badge of every row it names, then recounts `new_change_count` from the surviving rows. Draining is the acknowledgement, so a passive projection pass sees an empty batch.
- `operator_outcome_event` in `src/orchestration/operator_coordinator.rs` became `pub(crate)` so the regression tests publish the exact event the shared transaction publishes rather than a hand-built copy.
- The proposal gained the `## Retired Scenarios` declaration the promotion gate requires for the two consolidated local scenarios; `openspec_cmd::promotion::tests::every_pending_change_promotes_without_dropping_a_scenario` failed without it and passes with it.
- evidence: `cargo test tui::state::execution_mark_tests --lib` — 12 passed, 0 failed.
- evidence: `cargo test --tests --no-fail-fast` — every integration binary passed; the lib binary reports 4231 passed / 10 failed, and all 10 are pre-existing `tui::render::tests` header-width failures reproduced identically on a clean `HEAD` worktree (they depend only on the long `CARGO_PKG_VERSION` string, which this change does not touch).
- evidence: `cargo clippy --all-targets --all-features` and `cargo fmt --all -- --check` clean.

## Final Validation

Archive validation is the authoritative OpenSpec gate. Commands run:

`cflx openspec validate clear-new-on-remote-mark --strict --evidence warn` — Validation passed

`cflx openspec validate clear-new-on-remote-mark --archive-gate` — Validation passed
