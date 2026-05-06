## Implementation Tasks

- [x] Add a regression test in `src/parallel/tests/executor.rs` for manual `M` while another change is in flight, proving reducer-owned `ResolveWait` remains scheduler-visible and is retried after the in-flight work completes (verification: integration - run `cargo test test_manual_resolve_wait_retries_after_in_flight_apply_completes`).
- [x] Add or update TUI state tests in `src/tui/state.rs` and/or `src/tui/command_handlers.rs` so pressing `M` on `merge wait` records `resolve pending` without direct merge execution, even while the scheduler is already running (verification: unit - run `cargo test resolve_merge` and assert `ReducerCommand::ResolveMerge` intent plus scheduler notification behavior).
- [x] Fix the scheduler retry path in `src/parallel/orchestration.rs` and/or `src/parallel/queue_state.rs` so reducer-owned `ResolveWait` is not stranded behind unrelated apply/archive activity and is retried after completion or queue notification when the base-mutating lane is free (verification: integration - run `cargo test test_scheduler_dispatches_synced_manual_resolve_wait_without_queued_work` and `cargo test test_manual_resolve_wait_retries_after_in_flight_apply_completes`).
- [x] Preserve unrelated queue progress while resolve retry intent exists, including free-slot dispatch for queued changes during pending resolve wait (verification: integration - run `cargo test test_resolve_wait_does_not_block_queue_reanalysis_dispatch` and `cargo test test_resolving_with_free_slot_still_dispatches_queued_change`).
- [x] Ensure retry outcomes are visible and state-correct in `src/orchestration/state.rs`, `src/tui/state/event_handlers/completion.rs`, and `src/tui/state/event_handlers/errors.rs`: successful retry clears `ResolveWait` and reaches merged state; manual deferral returns to `MergeWait` with warning/log evidence instead of indefinite `resolve pending` (verification: integration - run `cargo test resolve_wait` and `cargo test merge_wait` and assert reducer display status transitions).
- [x] Run focused Rust tests for parallel scheduler and TUI resolve behavior, then run the repository's required lint/typecheck/test commands if available (verification: manual - captured passing output from `cargo test test_resolve_wait_does_not_block_queue_reanalysis_dispatch`, `cargo test test_resolving_with_free_slot_still_dispatches_queued_change`, `cargo test test_scheduler_dispatches_synced_manual_resolve_wait_without_queued_work`, `cargo test test_manual_resolve_wait_retries_after_in_flight_apply_completes`, full `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`; manual is intentional because full command selection may depend on current feature flags).

## Future Work

- Live TUI dogfood during a real multi-change run may be performed after implementation, but the proposal requires repository-verifiable tests before acceptance.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-resolve-wait-apply-progress --archive-gate`
