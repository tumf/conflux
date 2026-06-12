# Design: Fix Spawned Retry Give-Up Paths Leaving the Base-Mutating Lane Occupied

## Abandon vs. restore: two distinct lane-release semantics

The prior change (`fix-spawned-retry-lane-release`) introduced
`release_base_mutating_lane_after_retry(change_id, wait_state)`, which RESTORES the
occupant to its origin wait kind and re-enqueues it — correct for retryable outcomes
(auto-resumable Deferred, transient Err): the work should be attempted again on the
next trigger.

Give-up outcomes are different. When the retry body concludes the intent is stale
(workspace gone, stale workspace path, change already merged to base), it clears the
retry intent and reports `Ok(Merged)` purely as a next-waiter dispatch trigger. Reusing
the restore-style release here would re-enqueue the abandoned change, producing a
give-up loop: every later merge-completion trigger would promote it, give up again,
and re-enqueue — wasting lane turns and log noise indefinitely. Hence a second,
deliberately non-restoring method:

```text
abandon_base_mutating_lane_occupant(change_id):
  - no-op if entry is terminal or activity not in {Resolving, Rejecting}
  - activity = Idle
  - wait_state stays None (do NOT restore ResolveWait/RejectWait)
  - clear blocked metadata
  - remove from resolve_wait_queue and reject_wait_queue
```

`global_invariants_hold()` is preserved trivially: the method only removes the single
active occupant from lane occupancy.

## Why release happens in the retry body, not the result handler

An alternative was to make `handle_merge_result_with_tx`'s `Merged` branch release the
lane for retry origins when the occupant is "still active without a terminal state".
That inference is racy: legitimate terminal transitions for some genuinely-completed
retry outcomes travel asynchronously through the event channel rather than through
synchronous shared-state writes. Examples:

- Rejection review Confirm success applies `RejectionReviewCompleted` synchronously via
  `apply_rejection_review_event_in_shared_state`, but the terminal `ChangeRejected` is
  only SENT on `event_tx` (`src/parallel/queue_state.rs:1460-1467`) and applied later
  by the event loop. At result-delivery time the occupant may still look "active
  without terminal", and a result-handler release would wrongly reset it to
  `RejectWait`-less Idle or, with restore semantics, re-enqueue a change that was just
  rejected.

The give-up sites, by contrast, know with certainty that they are abandoning the
intent. Releasing there — synchronously with `clear_resolve_wait_intent_for_outcome` /
`clear_reject_wait_intent_for_success`, through the same `shared_orchestrator_state`
write path — is race-free regardless of event-channel ordering, by the same argument
that validated the prior change's release ordering: whichever of
(reducer event application, abandon write) lands second is a no-op or idempotent.

## Interaction with the prior change's release wiring

Unchanged. `release_base_mutating_lane_after_retry` keeps handling
`Deferred(auto_resumable=true)` and `Err` outcomes in the result handler. The abandon
method handles only the four give-up sites inside the retry bodies. The two methods
never run for the same outcome: give-up paths return `Ok(Merged)`, which the result
handler does not release. The result handler's `Merged` branch continues to call
`dispatch_deferred_base_lane_waiter`, which now finds the lane free after a give-up
and can promote the next waiter — this is the convergence property the new tests
assert.

## Test strategy

- Reducer unit tests for the abandon method mirror the existing
  `release_base_mutating_lane_after_retry_*` tests (promotion via
  `promote_next_base_mutating_lane_waiter`, then abandon, then assertions).
- The two existing workspace-lookup operator-visibility tests gain a promoted occupant
  in shared state and post-conditions on `is_base_mutating_lane_occupied()` and
  next-waiter promotion, turning them from event-only assertions into lane-safety
  regressions that fail without the fix.
- A give-up convergence test queues two ResolveWait waiters, lets the first give up
  (missing workspace via `TestWorkspaceManager`), and asserts the second is promoted
  by the resulting `Merged`-trigger dispatch.
