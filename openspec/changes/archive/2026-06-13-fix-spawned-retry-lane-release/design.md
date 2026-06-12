# Design: Fix Spawned Retry Base-Mutating Lane Release

## Root Cause Summary

`fix-scheduler-inline-resolve-blocking` introduced two cooperating mechanisms:

- Promotion + spawn: `promote_next_base_mutating_lane_waiter()` marks the occupant
  `activity = Resolving`/`Rejecting`, `wait_state = None`, then
  `spawn_base_lane_retry_task` runs the retry body detached and reports through the
  merge-result channel with a `MergeResultOrigin`.
- `attempt_merge()` `try_lock`: lock contention now returns
  `Deferred(auto: "Merge lane busy")` instead of parking on the lock.

The seam between them leaks: the reducer's `MergeDeferred(auto_resumable=true)`
application deliberately does not touch an `is_active()` entry ("do not interrupt an
already-active merge attempt"), which is correct for events about *another* task's merge
attempt, but the promoted occupant's own deferral arrives through the same event. The
scheduler-side result handler also does nothing for Deferred/Err retry outcomes. Net
result: occupant stays `Resolving` with `wait_state = None`; the lane is occupied forever;
every future promotion returns `None`.

Reproduction window: a post-archive merge task holds `global_merge_lock` during its merge
phase (before any `ResolveStarted` marks it as the lane occupant). Any dispatch trigger in
that window (queue notification via `x`, wait-set delta) promotes a ResolveWait change,
whose spawned retry immediately defers on `try_lock` and strands.

## Approach

### Why scheduler-side release (not reducer event inference)

The reducer cannot distinguish "deferral reported by the occupant's own retry" from
"deferral reported about a change that is actively merging elsewhere" by event payload
alone — that knowledge lives in `MergeResult.origin`, which only the scheduler sees.
Therefore the release is driven from `handle_merge_result_with_tx`, matching the prior
change's stated design ("scheduler-side bookkeeping is updated in the scheduler loop when
the result arrives"), while the state mutation itself stays reducer-owned via a dedicated
`OrchestratorState` method.

### Release semantics

`release_base_mutating_lane_after_retry(change_id, wait_kind)`:

- Preconditions: entry exists, not terminal, `activity` is `Resolving` (for
  `ResolveWait` origin) or `Rejecting` (for `RejectWait` origin). Otherwise no-op —
  the occupant may have been legitimately transitioned by a concurrent success event
  (`ResolveCompleted` → merged terminal) before the result was handled; release must not
  regress terminal or merged states.
- Effects: `activity = Idle`; `wait_state = wait_kind`; unique re-enqueue into the
  matching wait queue. `clear_blocked_metadata()` is NOT called (blocked metadata was
  already cleared at promotion; deferral reasons surface via `MergeDeferred` events).
- Invariant: `global_invariants_hold()` (≤1 lane occupant) holds before and after.

Outcome mapping in `handle_merge_result_with_tx` for retry origins:

| Outcome                         | Action                                                       |
| ------------------------------- | ------------------------------------------------------------ |
| `Ok(Merged)`                    | unchanged (dispatch next waiter)                              |
| `Ok(Deferred{auto: true})`      | release lane, restore origin wait kind, resync local sets     |
| `Ok(Deferred{auto: false})`     | unchanged (reducer manual branch already set Idle+MergeWait)  |
| `Err(_)`                        | release lane, restore origin wait kind, resync local sets;    |
|                                 | skip duplicate generic Error if a specific failure event was  |
|                                 | already emitted by the retry body                             |

For `Err` after `ResolveFailed` was emitted, the reducer has already set
`Idle + MergeWait`; the release no-op precondition (activity no longer `Resolving`)
makes the scheduler-side call safe and idempotent — it only acts when the failure path
emitted nothing (workspace-lookup failure).

### Re-promotion path (no new triggers needed)

After release, the change is again listed by `resolve_wait_change_ids()` /
`reject_wait_change_ids()`. The scheduler's per-iteration
`sync_resolve_wait_from_shared_state_nonblocking()` repopulates local wait sets, the
set-delta check in `should_dispatch_resolve_wait_retry()` arms a dispatch, and existing
triggers (merge/resolve completion, queue notification) re-promote. No new wakeup
machinery is added; livelock is avoided because the lock holder's own completion always
delivers a `MergeResult` that re-enters dispatch.

### Test gating strategy

- Lock variant and convergence tests serialize on the existing `merge_lock_test_mutex`
  in `src/parallel/merge.rs` tests (the global lock is process-wide).
- Gated resolve uses a command blocking on a file created by the test (removed to
  release) or an equivalent oneshot-backed mock, never wall-clock sleeps, keeping
  default-suite tests under 1 second per AGENTS.md.
- The lock-variant and convergence tests assert post-conditions that are false on the
  current code (lane occupied, change stranded), satisfying the "fails before the fix"
  evidence requirement.

## Invariants

- Single base-mutating lane occupant (unchanged), now with the guarantee that occupancy
  always corresponds to a live spawned task or an inline manual resolve.
- Constitution Law 1: all state involved is in-memory reducer/scheduler state; no
  out-of-worktree durable workflow state is introduced.
- Capacity accounting, `pending_merge_count` drain coverage, and event ordering per
  change are unchanged.

## Risks and Mitigations

- **Risk**: release races with a concurrent success event for the same change.
  **Mitigation**: release preconditions (non-terminal, activity matches origin) make it
  a no-op when a success/failure event already transitioned the entry; covered by a
  reducer unit test.
- **Risk**: double release if the same result were processed twice.
  **Mitigation**: release is idempotent (second call sees `Idle`, no-ops); unique
  enqueue prevents queue duplication.
- **Risk**: suppressing the generic Error event hides failures.
  **Mitigation**: suppression applies only to retry origins, and only because the retry
  body already emitted `ResolveFailed`/`RejectionReviewFailed`; the previously silent
  workspace-lookup path gains an event instead.
