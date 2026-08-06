## Implementation Tasks

- [ ] Introduce one coherent reducer classification snapshot for `classify_queued_work`, containing ordinary queue eligibility, reducer-owned queue IDs, lane waits, terminal errors, active/resolving state, Acceptance holds, and external blocker holds. Completion requires asynchronous lock acquisition, copying only required facts, and releasing the reducer guard before repository, VCS, dependency, analyzer, or dispatch awaits. (verification: unit - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Refactor `DependencyContext` and queue classification to consume the same captured snapshot so separate `try_read` outcomes cannot disagree. Completion requires incomplete evidence to remain fail-closed without producing a stable blocked-only or candidate-unavailable result that can enter persistent idle. (verification: unit - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Add scheduler-path contention coverage that holds the reducer write lock, proves no analyzer or dispatch starts, releases the lock without queue notification, and proves the original evaluation continues to normal dispatch for a queued candidate. Completion requires bounded Tokio timeouts and no sleeps longer than the repository's one-second default-test limit. (verification: integration - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Add the complementary held-change regression: after the same contention clears, a real Acceptance/external hold is classified as stable blocked-only, remains out of ordinary dispatch, and does not repeat Acceptance. Completion requires inspecting analyzer/dispatch counters and reducer-held membership rather than only a log message. (verification: integration - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Preserve cancellation and stable-idle behavior. Completion requires tests proving pending snapshot acquisition can be cancelled and genuinely drained or blocked-only persistent state still waits only on scheduler-owned events without worktree polling or repeated analyzer invocation. (verification: integration - `cargo test --lib persistent_idle && cargo test --lib reanalysis_trigger_lifetime`; verification-id: queue-classification-liveness)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate retry-transient-queue-classification --archive-gate`.

## Future Work

- Consider a reusable versioned reducer snapshot type only if another subsystem needs the same coherent multi-field read boundary.
