# Tasks: Fix Spawned Retry Base-Mutating Lane Release

## Implementation Tasks

- [x] Task 1: Add a reducer lane-release method to `src/orchestration/state.rs`
      (e.g. `release_base_mutating_lane_after_retry(change_id, wait_state)`): for a
      non-terminal entry whose `activity` is `Resolving`/`Rejecting`, reset `activity` to
      `Idle`, restore `wait_state` to the given `ResolveWait`/`RejectWait`, and re-enqueue
      the change uniquely in the matching wait queue (`enqueue_unique_resolve_wait` /
      reject equivalent). No-op for terminal entries and for entries that are not the
      lane occupant. Completion condition: method exists with unit tests asserting
      activity reset, wait restoration, unique enqueue (no duplicates on repeated calls),
      no-op on terminal entries, and `global_invariants_hold()` after release.
      verification: unit - reducer tests in `src/orchestration/state.rs`
- [x] Task 2: Wire origin-aware lane release into
      `src/parallel/queue_state.rs::handle_merge_result_with_tx`: when
      `merge_result.origin` is `ResolveWaitRetry` or `RejectWaitRetry` and the outcome is
      `Ok(MergeTaskOutcome::Deferred { auto_resumable: true, .. })` or `Err(_)`, call the
      Task 1 release through `shared_orchestrator_state` with the wait kind matching the
      origin, then resync scheduler-local wait sets
      (`sync_resolve_wait_from_shared_state_nonblocking`). Manual deferrals
      (`auto_resumable: false`) keep current behavior. Completion condition: no result
      path for retry origins returns with the occupant still `Resolving`/`Rejecting` in
      shared state. verification: unit - test in `src/parallel/tests/executor.rs` driving
      `handle_merge_result_with_tx` with origin `ResolveWaitRetry` + `Deferred(auto)` and
      asserting `is_base_mutating_lane_occupied()` is false and the change is listed by
      `resolve_wait_change_ids()` afterwards (this test fails on current code)
- [ ] Task 3: Make retry failure reporting origin-aware in
      `handle_merge_result_with_tx` and the retry bodies: suppress the generic
      "Background merge failed" `ParallelEvent::Error` for `ResolveWaitRetry` /
      `RejectWaitRetry` results whose retry body already emitted `ResolveFailed` /
      `RejectionReviewFailed`, and emit an operator-visible event (log or
      `ParallelEvent::Error` with retry context) for the workspace-lookup failure paths
      in `retry_deferred_merges_for` / `retry_deferred_rejection_review_for` that
      currently only `warn!`. Completion condition: exactly one operator-visible failure
      report per retry failure; workspace-lookup failure is operator-visible.
      verification: unit - event-channel assertions in `src/parallel/tests/executor.rs`
      covering both a resolve-failure result and a workspace-lookup failure result
- [ ] Task 4: Lock-variant regression test (prior change Task 6d backfill): hold
      `global_merge_lock` from the test, place a change in ResolveWait in shared state,
      trigger retry dispatch so the spawned retry defers with "Merge lane busy", deliver
      the result, then assert the lane is unoccupied, the change is back in ResolveWait,
      and a subsequent dispatch promotes it again. Completion condition: test exists and
      fails when the Task 2 release is reverted; runs under 1 second using the existing
      `merge_lock_test_mutex` serialization. verification: integration - test in
      `src/parallel/tests/executor.rs` (red on pre-fix code, green after)
- [ ] Task 5: Deferred-then-retried convergence regression test (prior change Task 7
      backfill): after the lock holder releases and a merge-completion trigger fires
      (`handle_merge_result_with_tx` with a Merged result), assert the previously
      lane-busy-deferred change is re-promoted and its retry reaches a merged outcome
      (`MergeCompleted` event or shared-state merged terminal) without user action.
      Completion condition: test exists and fails when the Task 2 release is reverted;
      under 1 second with a controllable (instant) resolve mock.
      verification: integration - test in `src/parallel/tests/auto_resolve.rs`
- [ ] Task 6: Gated-resolve scheduler-loop regression test (prior change Task 6a/6c
      backfill): configure the resolve command as a controllable gated mock (blocks until
      the test releases a file/oneshot gate); while the gate is held, push a change via
      the dynamic queue and assert `AnalysisStarted` fires for it within bounded
      scheduler ticks and before the gate releases; cover the zero-recalculated-capacity
      variant asserting apply dispatch is suppressed with the
      `dispatch_capacity_zero_after_analysis` diagnostic. Completion condition: both
      assertions run against the scheduler loop (not by calling
      `perform_reanalysis_and_dispatch` directly) and complete under 1 second; any
      wide-window variant is `heavy`-gated. verification: integration - tests in
      `src/parallel/tests/manual_resolve.rs` or `auto_resolve.rs`
- [ ] Task 7: Drain/exit regression test (prior change Task 5 backfill): with a spawned
      base-lane retry in flight (gated so it has not reported) and queued/in-flight
      empty, assert a finite-lifetime scheduler does not exit and `is_fully_drained()`
      stays false until the retry result is delivered, then the scheduler proceeds to
      ResolveWait clearing and next-waiter promotion. Completion condition: test exists
      and exercises `pending_merge_count` accounting across the spawn window, under 1
      second. verification: integration - test in `src/parallel/tests/executor.rs`
- [ ] Task 8: Run quality gates (verification: integration - `cargo test --lib parallel::tests::executor` plus
      `cargo fmt --check`, `cargo clippy`,
      `cargo test --lib parallel::tests::auto_resolve`,
      `cargo test --lib parallel::tests::manual_resolve`, and
      `cargo test --lib orchestration::state`; all pass on the final tree); confirm no
      existing resolve-wait, reject-wait, blocked-only drain, persistent-idle, or
      terminal-gate tests regress. Completion condition: all listed commands pass.

## Future Work

- Manual TUI confirmation on a real repository: while a post-archive merge holds the
  global lock, queue a change with `x`, observe the lane-busy deferral and subsequent
  automatic convergence in the cflx logs (verification: manual - requires an interactive
  TUI session with a real conflicting merge; reviewers evaluate via
  `~/.local/state/cflx/logs` entries showing "Merge lane busy" followed by re-promotion
  and merge completion).
- Separate cleanup proposal for the `#[allow(dead_code)]` legacy inline retry paths left
  behind by `fix-scheduler-inline-resolve-blocking` (verification: manual - tracked as a
  future change proposal; no behavior change in this change).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-spawned-retry-lane-release --archive-gate`
