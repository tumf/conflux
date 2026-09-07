## Implementation Tasks

- [ ] Centralize active-interval start, pause, resume, and current-total calculation in `ChangeState`, using `is_active_status` for transition classification and preserving accumulated duration across inactive and terminal transitions (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)
- [ ] Replace handler-specific elapsed snapshots in processing, error, and completion transitions with the centralized interval lifecycle so inactive time is excluded and active intervals are counted exactly once (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)
- [ ] Render accumulated plus current active duration while active and retained accumulated duration while inactive, including `merged`, `error`, and `stalled`, without changing badge, spinner, iteration, or row alignment behavior (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)
- [ ] Add focused regression tests for separated active intervals, repeated active updates, inactive exclusion, and retained rendering in `merged`, `error`, and `stalled` states (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retain-net-execution-time --archive-gate`
