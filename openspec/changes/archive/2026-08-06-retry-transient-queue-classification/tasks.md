## Implementation Tasks

- [x] Introduce one coherent reducer work snapshot for a scheduler evaluation, containing ordinary queue eligibility, reducer-owned queue IDs, lane waits, terminal errors, active/resolving state, Acceptance holds, and external blocker holds. Completion requires asynchronous lock acquisition before reducer-dependent reconciliation or drain/idle decisions, copying only required facts, and releasing the reducer guard before repository, VCS, dependency, analyzer, or dispatch awaits. (verification: unit - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [x] Refactor `admit_dynamic_queue_hint`, `reconcile_queued_candidates_from_shared_state`, `DependencyContext`, and `classify_queued_work` to consume the captured view or an equivalent awaited acquisition. Completion requires transient unreadability to preserve any popped dynamic hint, avoid an empty reconciliation result, and remain fail-closed without producing stable candidate-unavailable, blocked-only, or drained evidence. Readable-state admission and revocation semantics must remain unchanged. (verification: unit - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [x] Make finite and persistent scheduler termination/idle decisions depend on a completed coherent work snapshot. Completion requires a finite run to avoid `DrainedSuccessfully`/`BlockedOrStalled` and a persistent run to avoid `wait_for_persistent_idle_wake` while reducer intent is temporarily unreadable; releasing the writer must continue the same evaluation without another notification. (verification: integration - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [x] Add full scheduler-path contention coverage starting with an empty scheduler-local queue and reducer-visible queued intent. Completion requires a separate task to hold/release the reducer writer, zero reconciliation/analyzer/dispatch activity before release, normal reconciliation and dispatch after release without another queue notification, bounded Tokio timeouts, and no sleeps longer than the repository's one-second default-test limit. (verification: integration - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [x] Add the complementary held-change regression: after the same contention clears, a real Acceptance/external hold is classified as stable blocked-only, remains out of ordinary dispatch, and does not repeat Acceptance. Completion requires inspecting analyzer/dispatch counters and reducer-held membership rather than only a log message. (verification: integration - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [x] Replace lock-unavailable tests that hold a reducer write guard while synchronously constructing `DependencyContext` or awaiting classification in the same task. Completion requires coordinated separate-task contention tests for `src/parallel/dependency.rs` and the existing executor lock-contention case, proving the awaited-read contract without self-deadlock. (verification: unit - `cargo test --lib reducer_snapshot_contention`; verification-id: queue-classification-liveness)

- [x] Preserve cancellation and stable-idle behavior. Completion requires tests proving pending snapshot acquisition can be cancelled and genuinely drained or blocked-only persistent state still waits only on scheduler-owned events without worktree polling or repeated analyzer invocation. (verification: integration - `cargo test --lib persistent_idle && cargo test --lib reanalysis_trigger_lifetime`; verification-id: queue-classification-liveness)

## Notes

Implementation shape: `src/parallel/work_snapshot.rs` holds `ReducerWorkSnapshot`, a copied
view distinguishing captured / absent (no reducer wired) / incomplete (acquisition abandoned
by cancellation). `ParallelExecutor::capture_reducer_work_snapshot` awaits the Tokio read
lock — racing the cancellation token when one is wired — copies the facts, and drops the
guard before returning. The scheduler loop captures exactly one view per evaluation and
threads it through dynamic-hint admission, lane-wait sync, reconciliation, the `work_drained`
check, `prepare_dispatch_candidates`, and both idle/exit decisions.

Three additional `try_read` sites in the same failure family were converted while proving the
contract, because each turned contention into a lifecycle claim:

- the startup gate in `execute_with_order_based_reanalysis`, which reported a *completed* run
  when an empty local queue met an unreadable reducer;
- the dispatch terminal-state gate in `src/parallel/dispatch.rs`, which failed **open** and
  could dispatch a merged/pushed/rejected/error change during a reducer write;
- `has_resolve_wait` and the `no_analysis_diagnostic` reducer read, which could respectively
  skip a run with real lane intent and drop the operator's only no-analysis notice.

Dispatch is proven at `select_changes_for_dispatch` rather than by letting the full loop
create real worktrees and spawn agent commands; reaching the analyzer in the full-loop test
already proves classification produced a dispatchable candidate. This mirrors the existing
convention in `src/parallel/tests/failed_dependency.rs`.

- evidence: `cargo test --lib reducer_snapshot_contention` — 8 passed, 0 failed (0.44s)
- evidence: `cargo test --lib persistent_idle` — 3 passed; `cargo test --lib reanalysis_trigger_lifetime` — 7 passed
- evidence: `cargo test --lib` — 3320 passed, 0 failed, 14 ignored (45.2s)
- evidence: `cargo fmt --check` clean; `cargo clippy --locked --all-targets --all-features -- -D warnings` clean
- evidence: verification runs used a private `CARGO_TARGET_DIR`; the repository's shared target dir serves a stale `--lib` test binary (see `cargo-shared-target-stale-test-binary`)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate retry-transient-queue-classification --archive-gate`.

## Future Work

- Consider a reusable versioned reducer snapshot type only if another subsystem needs the same coherent multi-field read boundary.
