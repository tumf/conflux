## Implementation Tasks

- [ ] Define shared application-service methods and typed changed/no-op/failed outcomes for start, retry, stop, cancel stop, force stop, and resolve. (verification: unit - `cargo test --features web-monitoring --lib` verifies service tests cover the complete mode/status matrix and side-effect admission; verification-id: shared-command-tests)

- [ ] Replace TUI-specific lifecycle execution paths with thin adapters over the shared services while preserving current warnings, events, scheduling, cancellation, and resolve-queue semantics. (verification: integration - `cargo test --features web-monitoring --lib` verifies TUI adapter tests prove existing successful and invalid-mode behavior through the service boundary; verification-id: shared-command-tests)

- [ ] Replace v2 channel-enqueue settlement with shared-service execution and settle command records only after actual acceptance, no-op, or failure plus the resulting synchronous projection revision. (verification: integration - `cargo test --features web-monitoring --lib` verifies command API tests prove no false success, one effect per idempotency key, stale refusal, and settled readback; verification-id: shared-command-tests)

- [ ] Connect start to the authoritative marked target set, retry to reconciled routing plus scheduler dispatch, and resolve to single-resolver FIFO scheduling for both live and idle schedulers. (verification: integration - `cargo test --features web-monitoring --lib` verifies table-driven cross-adapter tests compare target IDs, reducer state, scheduler notifications/spawns, and events; verification-id: shared-command-tests)

- [ ] Preserve cancellation-first stop-and-dequeue and implement truthful graceful/immediate/force classification and recovery across duplicate, timeout, missing-handle, and completed-work cases. (verification: integration - `cargo test --features web-monitoring --lib` verifies cancellation and force-stop tests prove unsafe cases preserve active state and do not claim force success; verification-id: shared-command-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-remote-operator-commands --archive-gate`

The implementation must also pass `cargo test --features web-monitoring --lib`.

## Future Work

- UI confirmation and user-facing command feedback remain consumer responsibilities.
