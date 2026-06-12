# Design: Fix Scheduler Inline Resolve Blocking

## Root Cause Summary

The scheduler loop (`execute_with_order_based_reanalysis`) is a single task. Two paths
park that task for the full duration of an AI resolve agent run (minutes):

1. `maybe_dispatch_resolve_wait_retry()` → `retry_deferred_base_lane_waiters()` →
   `retry_deferred_merges_for()` → `attempt_merge()` → `merge_and_resolve()` →
   `resolve_conflicts_with_retry()` — the resolve agent is awaited inline at Step 2 of the
   loop, before queue reconciliation and `perform_reanalysis_and_dispatch()` ever run.
   The same chain is awaited inline from `handle_workspace_completion` (error and rejected
   branches) and `handle_merge_result`.
2. `attempt_merge()` awaits `global_merge_lock().lock()` before its "resolve in progress →
   Deferred" early return. A spawned post-archive merge task holds that lock across its own
   conflict-resolve agent run, so the scheduler-loop caller parks on the lock.

All historical fixes changed gating inside `perform_reanalysis_and_dispatch()`, which is
unreachable while the loop is parked — hence the bug surviving many "fixes". Tests mock
resolve with instant commands, so the parking window rounds to zero in CI.

## Approach

### 1. Spawned base-lane retry execution

Replace the inline execution inside `maybe_dispatch_resolve_wait_retry()` with a
spawn-based dispatch, mirroring `spawn_merge_task()`:

- Promotion stays as-is: `promote_next_base_mutating_lane_waiter()` (reducer write lock)
  picks at most one waiter and marks its activity `Resolving`/`Rejecting`. Reducer lane
  occupancy is the single-flight guard; while occupied, further promotions return `None`,
  so a second retry task cannot start.
- After promotion, build a detached executor clone (same fields as `spawn_merge_task`:
  config, event_tx, counters, hooks, cancel token, stagger state) and `tokio::spawn` the
  existing retry body (`retry_deferred_merges_for` for ResolveWait,
  `retry_deferred_rejection_review_for` for RejectWait) inside it.
- The spawned task sends its outcome through the existing `merge_result_tx` channel
  (extend `MergeResult`/`MergeTaskOutcome` with what the retry path needs — at minimum
  change_id, merged/deferred/failed, and whether it was a lane-retry — so
  `handle_merge_result` can clear ResolveWait intent, emit events, and trigger the next
  promotion). Scheduler-side bookkeeping (`resolve_wait_changes`,
  `last_dispatched_*`, `resolve_wait_retry_triggered`) is updated in the scheduler loop
  when the result arrives, not inside the spawned task, to keep `&mut self` state
  loop-owned.
- A `pending` counter must cover the spawned retry window so drain/exit checks stay
  truthful. Either reuse `pending_merge_count` (incremented before spawn, decremented in
  the result handler) or add an equivalent counter included in `is_fully_drained()` and
  `work_drained`. The reducer lane occupant also keeps
  `has_reducer_owned_lane_wait_or_active` true for the TUI restart path.

Completion-handler call sites (`handle_workspace_completion` error/rejected branches,
`handle_merge_result`) call the same non-blocking dispatch function. Net effect: the loop
only ever does promotion + spawn (fast reducer lock + task spawn), never agent execution.

`resolve_wait_base_dirty_changed_to_clean()` (a `git status`) and
`is_change_already_merged_to_base()` remain inline; they are subprocess-fast and bounded.
If the merged-to-base precheck is considered too slow for the loop, move it into the
spawned task together with the workspace lookup — the spawned body already tolerates
stale/missing workspaces.

### 2. Non-blocking `attempt_merge` lane entry

Reorder the head of `attempt_merge()`:

1. Read `auto_resolve_count` + `manual_resolve_count`; if > 0, return
   `Deferred(auto: "Resolve in progress for another change")` — unchanged reason, now
   evaluated before any lock.
2. `global_merge_lock().try_lock()`; on contention return
   `Deferred(auto: "merge lane busy")` (auto-resumable, so existing
   merge/resolve-completion retriggers pick it up).
3. Proceed as today while holding the guard.

This converts every potential multi-minute lock wait — from any caller — into an
auto-resumable deferral that the existing retry machinery already handles. Spawned
post-archive merge tasks that previously queued on the lock now defer; they re-enter
through `handle_merge_result` → `MergeDeferred` → ResolveWait, an existing, tested flow.

### 3. Regression tests that would have caught this

- **Gated resolve**: configure the resolve command as a controllable slow command (e.g.
  a script blocking on a file/FIFO, or a long `sleep` with the heavy-test guard) so the
  resolve window is deterministic and wide.
- While the resolve is active: push a change via the dynamic queue, then assert within a
  bounded time (a few scheduler ticks) that `AnalysisStarted` fires and — when
  `max_parallelism` leaves capacity — `ApplyStarted`/dispatch occurs, all before the gate
  is released.
- Lock variant: hold `global_merge_lock` (or run a gated spawned merge) with a
  resolve_wait entry present, send a queue notification, assert the loop still reaches
  analysis.
- Drain variant: with a spawned retry in flight and nothing else, assert the scheduler does
  not exit/idle-stop until the result arrives.
- Keep each default-suite test under 1 second (AGENTS.md rule); use short gate timeouts or
  mark wide-window variants `heavy`.

## Invariants

- Single base-mutating lane: at most one resolve/rejection-review executes at a time,
  enforced by reducer lane occupancy (unchanged).
- Capacity: resolve continues to consume slot capacity via the existing counters;
  ordinary apply dispatch stays capacity-gated at dispatch time.
- Constitution law 1: no new durable out-of-worktree workflow state; the spawn handoff is
  in-memory scheduling only, and all workflow truth stays repository-visible.
- Event ordering: `ResolveStarted` → (outcome events) ordering per change is preserved
  because the spawned task runs the same body that emits them today.

## Risks and Mitigations

- **Risk**: double-execution of a retry if dispatch is triggered twice quickly.
  **Mitigation**: promotion through the reducer is the only entry point and is
  single-occupant; `ActivePostArchiveMergeGuard` continues to dedupe per-change merge
  tasks.
- **Risk**: scheduler exits while a spawned retry is in flight.
  **Mitigation**: pending counter included in `is_fully_drained()`/`work_drained` +
  drain regression test.
- **Risk**: `try_lock` deferral causes livelock (merge never runs because the lane is
  always briefly busy). **Mitigation**: deferrals are auto-resumable and re-dispatched on
  every merge/resolve completion and queue notification — the same recovery contract that
  exists today for `Deferred(auto)`; add a test for deferred-then-retried success.
- **Risk**: `MergeResult` channel shape change ripples into web/TUI bridges.
  **Mitigation**: keep `ParallelEvent` surface unchanged; only the internal scheduler
  channel payload grows.
