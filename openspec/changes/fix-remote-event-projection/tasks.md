## Implementation Tasks

- [ ] Inventory every `ExecutionEvent` producer and current reducer/TUI/Web/v2 application path, then designate one reducer/dispatch owner and explicit presentation-only exceptions. (verification: unit - `cargo test --features web-monitoring event projection reducer` verifies a table-driven ownership test contains every enum variant and fails when a new variant lacks classification; verification-id: projection-ownership-tests)

- [ ] Route orchestration events through one dispatch path that applies reducer state once and fans authoritative event/state output to frontend sinks without frontend reapplication. (verification: integration - `cargo test --features web-monitoring event projection reducer` verifies one emitted event produces one reducer transition and one sink delivery in serial and parallel fixtures; verification-id: projection-ownership-tests)

- [ ] Make v2 projection consume direct authoritative event/state output, preserving all fields and allocating one revision/sequence according to state-changing versus log-only semantics. (verification: integration - `cargo test --features web-monitoring event projection reducer` verifies golden projection tests detect field loss, duplicate revision increments, duplicate sequence allocation, and no-op revision changes; verification-id: projection-ownership-tests)

- [ ] Unify structured log delivery and retention for serial/parallel AI output, hooks, lifecycle, warnings, and errors with at-most-once entries. (verification: integration - `cargo test --features web-monitoring event projection reducer` verifies log tests compare both modes and prove one retained entry per internal log event plus correct 1000-entry retention; verification-id: projection-ownership-tests)

- [ ] Align terminal-state handling for late `AllCompleted`, duplicate `Stopped`, Error, and resolve/merge completion while preserving replay and gap recovery. (verification: integration - `cargo test --features web-monitoring event projection reducer` verifies ordered/out-of-order/duplicate event tests prove Error and Stopped are not incorrectly overwritten and streams remain recoverable; verification-id: projection-ownership-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-remote-event-projection --archive-gate`

The implementation must also pass `cargo test --features web-monitoring event projection reducer`.

## Future Work

- Removal of retained legacy frontend modules may occur in `modernize-web-monitoring-ui` after consumers migrate.
