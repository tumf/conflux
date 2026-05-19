## Implementation Tasks

- [ ] Add a reducer terminal-state query suitable for scheduler dispatch guards, or reuse existing state accessors to distinguish final terminal states from retryable terminal errors. (verification: unit - add/extend `src/orchestration/state.rs` tests proving `merged`, `archived`, and `rejected` are final dispatch stops while terminal `error` remains explicit-retry gated)
- [ ] Guard dynamic queue ingestion so reducer-terminal final changes are not pushed into scheduler-local `queued`. (verification: unit - add/extend `src/parallel/tests/executor.rs` coverage for `check_dynamic_queue_and_add_changes` where dynamic queue contains a terminal `merged` change and `queued` remains empty)
- [ ] Guard workspace dispatch preflight so final terminal changes skip before workspace acquisition and before normal apply/acceptance/archive execution can begin. (verification: unit/integration - add/extend `src/parallel/tests/executor.rs` coverage that a terminal `merged` change passed to `dispatch_change_to_workspace` emits no `ArchiveStarted` and does not enter in-flight/workspace execution)
- [ ] Preserve existing recoverable terminal-error behavior while adding final terminal guards. (verification: unit - add/extend `src/orchestration/state.rs` and `src/parallel/tests/executor.rs` terminal-error tests with a regression assertion that terminal error still requires `ReducerCommand::RetryError` before ordinary dispatch eligibility)
- [ ] Prevent stale archive lifecycle display regression for merged TUI rows. (verification: unit - add/extend `src/tui/state/event_handlers/processing.rs` or `src/tui/state.rs` tests showing `handle_archive_started` leaves a `merged` row as `merged`)
- [ ] Verify workspace resume terminal handling remains unchanged for `WorkspaceState::Merged`. (verification: unit/integration - keep or extend `src/parallel/dispatch.rs` / `src/execution/state.rs` tests proving merged resume skips apply, acceptance, archive, and merge handoff)
- [ ] Run targeted Rust regression tests and formatting/lint checks required by the repository. (verification: manual - run the specific `cargo test ...` commands added above plus repository lint/typecheck commands discovered during implementation)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-merged-archive-dispatch --archive-gate`
