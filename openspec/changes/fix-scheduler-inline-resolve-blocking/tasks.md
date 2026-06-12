# Tasks: Fix Scheduler Inline Resolve Blocking

## Implementation Tasks

- [ ] Task 1: Make `attempt_merge()` lane entry non-blocking in `src/parallel/merge.rs`:
      evaluate `auto_resolve_count` + `manual_resolve_count` before any lock and return the
      existing auto-resumable `Deferred("Resolve in progress for another change")`; acquire
      `global_merge_lock()` with `try_lock()` and return an auto-resumable
      `Deferred` ("merge lane busy" class reason) on contention. Completion condition: no
      `global_merge_lock().lock().await` remains in `attempt_merge()`.
      verification: unit - test in `src/parallel/tests/` asserting `attempt_merge` returns
      auto-resumable `Deferred` without blocking while another task holds
      `global_merge_lock`, plus existing merge tests still pass
- [ ] Task 2: Extend the scheduler merge-result channel payload
      (`MergeResult`/`MergeTaskOutcome` in `src/parallel/types.rs` or module-local types) so
      a spawned base-lane retry can report change_id + merged/deferred/failed + retry-lane
      origin back to the loop. Completion condition: the payload carries enough data for
      `handle_merge_result` to clear ResolveWait/RejectWait intent and emit today's events.
      verification: unit - compile-time usage in `src/parallel/queue_state.rs` result
      handling; channel round-trip covered by the Task 6 integration tests via `cargo test`
- [ ] Task 3: Convert `maybe_dispatch_resolve_wait_retry()` in
      `src/parallel/queue_state.rs` to promotion + spawn: keep
      `promote_next_base_mutating_lane_waiter()` as the single-flight guard, spawn the
      existing retry bodies (`retry_deferred_merges_for` /
      `retry_deferred_rejection_review_for`) in a detached executor clone (mirroring
      `spawn_merge_task`), report through the merge-result channel, and move
      scheduler-local bookkeeping (`resolve_wait_changes`, `last_dispatched_*`,
      `resolve_wait_retry_triggered`) to the result handler. Completion condition: the
      scheduler loop task no longer transitively awaits `merge_and_resolve()` or
      `resolve_conflicts_with_retry()` from `maybe_dispatch_resolve_wait_retry()`.
      verification: integration - gated-resolve test in `src/parallel/tests/manual_resolve.rs`
      (from Task 6) fails before this task and passes after via `cargo test`
- [ ] Task 4: Route the inline `retry_deferred_base_lane_waiters().await` call sites in
      completion handling (`handle_workspace_completion` error/rejected branches,
      `handle_merge_result`) through the same non-blocking spawn dispatch. Completion
      condition: no completion-handler path awaits the resolve agent inline; consecutive
      resolve waiters cannot chain inside the loop task.
      verification: integration - multi-waiter test in `src/parallel/tests/auto_resolve.rs`:
      two ResolveWait entries with gated resolves; assert a queued ordinary change reaches
      `AnalysisStarted` between/during retries via `cargo test`
- [ ] Task 5: Include the spawned retry window in drain/idle accounting: cover it with
      `pending_merge_count` or an equivalent counter consumed by `is_fully_drained()` and
      the `work_drained` check in `src/parallel/orchestration.rs`. Completion condition:
      scheduler cannot exit or enter persistent idle while a spawned base-lane retry is in
      flight.
      verification: unit - drain test in `src/parallel/tests/executor.rs` with an in-flight
      spawned retry and otherwise empty state asserts the loop does not exit until the
      result arrives
- [ ] Task 6: Add regression tests with a slow/gated resolve command in
      `src/parallel/tests/` (manual_resolve / auto_resolve suites): while the resolve gate
      is held, (a) a dynamically queued change is ingested and `AnalysisStarted` fires
      within bounded scheduler ticks, (b) with free recalculated capacity the change is
      dispatched (`ApplyStarted`) before the gate releases, (c) with zero recalculated
      capacity dispatch is suppressed with the capacity-gated diagnostic, (d) the lock
      variant: with `global_merge_lock` held by another task and a resolve_wait entry
      present, a queue notification still leads to analysis. Tests must fail against the
      pre-change inline implementation; keep default-suite tests under 1 second (use the
      `heavy` feature gate for wide-window variants).
      verification: integration - `cargo test -p conflux parallel::tests` red on old code,
      green on new code
- [ ] Task 7: Verify deferred-then-retried recovery still converges: a merge deferred by
      `try_lock` contention or active-resolve precheck is retried to success after the
      blocking work completes, without user action.
      verification: integration - test in `src/parallel/tests/auto_resolve.rs` asserting
      `MergeDeferred(auto)` followed by `MergeCompleted` once the gate/lock is released
- [ ] Task 8: Run quality gates with `cargo fmt --check`, `cargo clippy`, and the parallel
      scheduler test suites; confirm no existing resolve-wait, reject-wait, blocked-only, or
      persistent-idle tests regress.
      verification: integration - `cargo test` for `parallel::tests::manual_resolve`,
      `parallel::tests::auto_resolve`, and `parallel::tests::executor` plus workspace build

## Future Work

- Manual TUI confirmation on a real repository: start a long resolve, press `x` on a new
  change, observe analyze starting while resolving (verification: manual - requires
  interactive TUI and a real conflicting merge, which is intentional coverage; reviewers
  evaluate via the cflx log lines for queue ingestion and `AnalysisStarted` timestamps
  during the resolve window).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-scheduler-inline-resolve-blocking --archive-gate`
