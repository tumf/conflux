## Implementation Tasks

- [ ] Introduce one coherent reducer work snapshot for a scheduler evaluation, containing ordinary queue eligibility, reducer-owned queue IDs, lane waits, terminal errors, active/resolving state, Acceptance holds, and external blocker holds. Completion requires asynchronous lock acquisition before reducer-dependent reconciliation or drain/idle decisions, copying only required facts, and releasing the reducer guard before repository, VCS, dependency, analyzer, or dispatch awaits. (verification: unit - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Refactor `admit_dynamic_queue_hint`, `reconcile_queued_candidates_from_shared_state`, `DependencyContext`, and `classify_queued_work` to consume the captured view or an equivalent awaited acquisition. Completion requires transient unreadability to preserve any popped dynamic hint, avoid an empty reconciliation result, and remain fail-closed without producing stable candidate-unavailable, blocked-only, or drained evidence. Readable-state admission and revocation semantics must remain unchanged. (verification: unit - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Make finite and persistent scheduler termination/idle decisions depend on a completed coherent work snapshot. Completion requires a finite run to avoid `DrainedSuccessfully`/`BlockedOrStalled` and a persistent run to avoid `wait_for_persistent_idle_wake` while reducer intent is temporarily unreadable; releasing the writer must continue the same evaluation without another notification. (verification: integration - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Add full scheduler-path contention coverage starting with an empty scheduler-local queue and reducer-visible queued intent. Completion requires a separate task to hold/release the reducer writer, zero reconciliation/analyzer/dispatch activity before release, normal reconciliation and dispatch after release without another queue notification, bounded Tokio timeouts, and no sleeps longer than the repository's one-second default-test limit. (verification: integration - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Add the complementary held-change regression: after the same contention clears, a real Acceptance/external hold is classified as stable blocked-only, remains out of ordinary dispatch, and does not repeat Acceptance. Completion requires inspecting analyzer/dispatch counters and reducer-held membership rather than only a log message. (verification: integration - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Replace lock-unavailable tests that hold a reducer write guard while synchronously constructing `DependencyContext` or awaiting classification in the same task. Completion requires coordinated separate-task contention tests for `src/parallel/dependency.rs` and the existing executor lock-contention case, proving the awaited-read contract without self-deadlock. (verification: unit - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [ ] Preserve cancellation and stable-idle behavior. Completion requires tests proving pending snapshot acquisition can be cancelled and genuinely drained or blocked-only persistent state still waits only on scheduler-owned events without worktree polling or repeated analyzer invocation. (verification: integration - `cargo test --lib persistent_idle && cargo test --lib reanalysis_trigger_lifetime`; verification-id: queue-classification-liveness)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate retry-transient-queue-classification --archive-gate`.

## Future Work

- Consider a reusable versioned reducer snapshot type only if another subsystem needs the same coherent multi-field read boundary.
