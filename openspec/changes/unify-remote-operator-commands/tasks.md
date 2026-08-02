## Implementation Tasks

- [x] Define shared application-service methods and typed changed/no-op/failed outcomes for start, retry, stop, cancel stop, force stop, and resolve. (verification: unit - `cargo test --features web-monitoring --lib` verifies service tests cover the complete mode/status matrix and side-effect admission; verification-id: shared-command-tests)

- [x] Replace TUI-specific lifecycle execution paths with thin adapters over the shared services while preserving current warnings, events, scheduling, cancellation, and resolve-queue semantics. (verification: integration - `cargo test --features web-monitoring --lib` verifies TUI adapter tests prove existing successful and invalid-mode behavior through the service boundary; verification-id: shared-command-tests)

- [x] Replace v2 channel-enqueue settlement with shared-service execution and settle command records only after actual acceptance, no-op, or failure plus the resulting synchronous projection revision. (verification: integration - `cargo test --features web-monitoring --lib` verifies command API tests prove no false success, one effect per idempotency key, stale refusal, and settled readback; verification-id: shared-command-tests)

- [x] Connect start to the authoritative marked target set, retry to reconciled routing plus scheduler dispatch, and resolve to single-resolver FIFO scheduling for both live and idle schedulers. (verification: integration - `cargo test --features web-monitoring --lib` verifies table-driven cross-adapter tests compare target IDs, reducer state, scheduler notifications/spawns, and events; verification-id: shared-command-tests)

- [x] Preserve cancellation-first stop-and-dequeue and implement truthful graceful/immediate/force classification and recovery across duplicate, timeout, missing-handle, and completed-work cases. (verification: integration - `cargo test --features web-monitoring --lib` verifies cancellation and force-stop tests prove unsafe cases preserve active state and do not claim force success; verification-id: shared-command-tests)

## Implementation Notes

- The shared run-lifecycle service is `src/orchestration/run_control.rs`, with unit tests in
  `src/orchestration/run_control/tests.rs` over an in-memory reducer, an in-memory queue, and a
  recording scheduler port. No process, repository, network, or timer is involved.
- `src/tui/run_supervisor.rs` implements `RunSchedulerPort` for the local TUI. It owns the
  orchestrator task handle and cancellation token that `src/tui/runner.rs` used to keep in the key
  loop, so a remote start really spawns the run a keypress would have spawned.
- Cross-adapter parity lives in `src/tui/command_handlers/cross_adapter_tests.rs`: one table drives
  every in-scope intent through both `handle_tui_command` and `SharedServiceExecutor` over
  identically arranged harnesses, then compares scheduler calls, reducer display statuses, the mark
  store, and the resolver ledger as a single value. Rows cover valid, invalid-mode, empty-target,
  duplicate, stale-target, scheduler-live, scheduler-idle, and runtime-launch-failure cases.
- `AppState::resolve_merge` is now presentation only. Reservation, FIFO ordering, duplicate
  rejection, the reducer transition, and scheduler dispatch moved into `RunControlService`, so the
  `AppState` tests that asserted those effects were rewritten against the new split rather than kept
  as duplicate coverage.
- `AppState::set_execution_marks` was added so a caller that builds the shared services first can
  bind the app to the mark store those services already read; production still wires it the other
  way round from `runner.rs`.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-remote-operator-commands --archive-gate`

- `cargo test --features web-monitoring --lib`: 2754 passed, 0 failed, 8 ignored.
- `cflx openspec validate unify-remote-operator-commands --strict`: passed.

## Future Work

- UI confirmation and user-facing command feedback remain consumer responsibilities.
