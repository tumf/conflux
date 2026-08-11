## Implementation Tasks

- [ ] Add a process-local Running-mode mark-set stability coordinator at the shared operator/orchestration boundary. It must observe accepted single and bulk mark outcomes, keep only the latest snapshot, restart one 10-second deadline on each real change, cancel pending settlement when Running mode ends, and remain event-driven when no settlement is pending. Complete when paused-time tests prove no early settlement, deadline reset, one final settlement, mode-exit cancellation, and restart-empty state. (verification: unit - `cargo test --lib running_mark_reanalysis`; verification-id: running-mark-reanalysis-tests)

- [ ] Classify the final stable snapshot against one coherent reducer/operator view and derive only eligible additions plus provenance-safe pending removals. Complete when tests prove marked ordinary `not queued` rows are selected; active, error, retry, merge/resolve wait, terminal, and ineligible rows are excluded; explicit queue membership is never claimed as mark-created; and duplicate snapshots are no-ops. (verification: unit - `cargo test --lib running_mark_reanalysis`; verification-id: running-mark-reanalysis-tests)

- [ ] Wire stable additions and eligible removals through the existing shared queue command path so reducer queue intent, `DynamicQueue`, queue hooks, authoritative outcomes, and scheduler notification retain their existing cardinality and ordering. Complete when real-queue tests prove each actual membership delta produces exactly one reducer transition and dynamic mutation, no-op reconciliation produces none, and removal never emits cancellation or stop. (verification: integration - `cargo test --lib running_mark_reanalysis`; verification-id: running-mark-reanalysis-tests)

- [ ] Route local TUI Space and bulk `x`, plus equivalent accepted shared operator mark commands, into the stability coordinator without adding frontend-local timers or a new key. Complete when adapter tests prove all equivalent Running-mode mark mutations schedule the same settlement, while Select, Stopping, Stopped, and Error remain mark-only and overlays retain input ownership. (verification: integration - `cargo test --lib running_mark_reanalysis`; verification-id: running-mark-reanalysis-tests)

- [ ] Preserve scheduler semantics after stable admission: a real queue addition must create the existing queue-addition reanalysis edge, analyze queued candidates during active resolve and at zero capacity, and dispatch only when capacity is available. Complete when deterministic scheduler-loop tests observe `AnalysisStarted` before resolve completion, no `ApplyStarted` at zero capacity, and later dispatch after a capacity transition without another mark or Start action. (verification: integration - `cargo test --lib running_mark_reanalysis`; verification-id: running-mark-reanalysis-tests)

- [ ] Add regression coverage for rapid mixed mark/unmark sequences and concurrent lifecycle changes at settlement. Complete when tests prove only the final stable snapshot is applied, a row becoming active or terminal before settlement is skipped safely, unmark cannot revoke explicit/admitted work, and tests use paused time or synchronization rather than short wall-clock assertions. (verification: integration - `cargo test --lib running_mark_reanalysis`; verification-id: running-mark-reanalysis-tests)

## Future Work

- Making the 10-second mark stability interval configurable.
- Extending mark-driven current-run admission to non-TUI product surfaces that do not use the shared operator command boundary.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate restore-running-mark-reanalysis --archive-gate`
