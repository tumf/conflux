## Implementation Tasks

- [ ] Remove `ChangeArchived` from successful-event mark revocation while preserving the existing target-scoped Error, Rejected, refresh-ineligible, dequeue, and merge-hook-recovery revocations in `src/events.rs`. (verification-id: archive-mark-preservation) (verification: unit - `cargo test --lib execution_mark -- --nocapture`)
- [ ] Add a deterministic regression test that starts with two marked changes, dispatches `ChangeArchived` for one, and asserts both `ExecutionMarkStore` and TUI rows remain marked without synthesizing queue intent. (verification-id: archive-mark-preservation) (verification: unit - `cargo test --lib execution_mark -- --nocapture`)
- [ ] Run the focused execution-mark suite and API/TUI local-command convergence regression to prove unrelated target-scoped marks are preserved. (verification-id: archive-mark-preservation) (verification: integration - `cargo test --lib accepted_operator_command_tui_convergence_local_command_preserves_unrelated_remote_marks -- --nocapture`)

## Future Work

None.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate preserve-marks-after-archive --archive-gate`
