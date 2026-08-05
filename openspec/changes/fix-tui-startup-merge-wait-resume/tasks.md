## Implementation Tasks

- [x] Update reducer workspace reconciliation so `ChangesRefreshed.merge_wait_ids` establishes `WaitState::MergeWait` only for a fresh idle, not-queued, non-terminal change, while preserving active, pending, queued, terminal, and error states. (verification: unit - add reducer transition and guard cases under `src/orchestration/state.rs`; verification-id: startup-merge-wait-tests)
- [x] Wire the reconciled reducer status through the existing manual resolve service without weakening command admission for changes lacking archived-but-not-yet-merged workspace evidence. Completion requires `RunControlService::resolve_merge` to reserve `ResolveWait` and dispatch the scheduler from refresh-reconstructed state while stale ordinary `not queued` targets remain rejected. (verification: integration - add refresh-origin resolve cases under `src/orchestration/run_control/tests.rs`; verification-id: startup-merge-wait-tests)
- [x] Add a TUI adapter regression case that starts from fresh reducer state, applies startup-equivalent `ChangesRefreshed.merge_wait_ids`, submits `TuiCommand::ResolveMerge`, and proves the row reaches reducer-owned `resolve pending` without a scheduler-state rejection warning. (verification: integration - extend `src/tui/command_handlers.rs` or `src/tui/command_handlers/cross_adapter_tests.rs`; verification-id: startup-merge-wait-tests)
- [ ] Verify the implementation is formatted, lint-clean, and passes the default Rust test suite. Completion requires successful `make fmt`, `make lint`, and `make test` results with no source or test regressions attributable to this change. (verification: integration - `make fmt && make lint && make test`; verification-id: repository-quality-gates)

## Future Work

- No external deployment, credentials, or human approval is required for this repository-local bug fix.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-startup-merge-wait-resume --archive-gate`
