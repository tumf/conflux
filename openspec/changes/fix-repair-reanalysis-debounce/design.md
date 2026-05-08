# Design: Repair re-analysis debounce handling

## Current behavior

The scheduler reconciles local queued candidates with reducer-visible queued intent and with existing worktrees. For archived-dirty workspaces that no longer have an active `openspec/changes/{change_id}` directory but do have an archive entry, `archived_dirty_repair_candidate_from_workspace` can synthesize an OpenSpec change candidate so the workflow can resume archive/merge repair.

That repair path is correct in principle because it is derived from workspace state and preserves restartability. The failure mode is that repair candidate discovery can be treated like a normal queue edit. Normal queue edits intentionally refresh debounce to batch user changes. Repair candidate rediscovery is not a user queue edit, so using the same debounce semantics can delay or prevent analysis when the same repair candidate is found repeatedly.

## Target behavior

Archived-dirty repair candidate discovery should be a repair trigger, not a normal queue notification.

A good implementation should make the scheduler answer three questions separately:

1. Did a user/reducer queued change become analysis-eligible?
2. Did a workspace-derived repair candidate become analysis-eligible?
3. Did anything actually change since the last scheduler evaluation?

Only the first case should behave like normal queue debounce. The second case should proceed promptly because the scheduler is repairing a known workspace-derived lifecycle state. The third case should not claim progress or spam diagnostics.

## Recommended implementation shape

Introduce a small result type for reconciliation, for example:

```rust
struct QueueReconciliationOutcome {
    queued_added: usize,
    repair_added: usize,
}
```

Then update scheduler reason selection so:

- `queued_added > 0` keeps existing queue debounce behavior.
- `repair_added > 0` uses a repair-specific trigger, for example `ReanalysisReason::RepairCandidate`.
- unchanged repeated repair discovery does not update `last_queue_change_at`.

`ReanalysisReason::RepairCandidate` should be included in debounce bypass logic, or handled equivalently by a bounded one-shot path. This keeps normal queue batching intact while making repair deterministic.

## Diagnostic handling

The first discovery of an archived-dirty repair candidate should remain visible because it explains why a seemingly unqueued change is being analyzed again. Repeated unchanged discoveries should be debug-level, deduped, rate-limited, or summarized.

This proposal does not require a persistent diagnostic cache. Loop-local in-memory sets are acceptable for dedupe because they do not control resume routing and do not survive process restart.

## Constitution compatibility

The repair candidate itself must continue to be derived from:

- active worktree file state
- archive entry file state
- workspace git state
- base-branch tree comparison where needed

The implementation must not introduce durable workflow-control state outside the worktree. In-memory scheduler bookkeeping for debounce and diagnostics is allowed because deleting `~/.local/state/cflx/**` must not change the next action chosen for the same workspace contents.

## Verification strategy

The key regression test should simulate repeated discovery of the same archived-dirty repair candidate without reducer queued intent and prove the scheduler does not remain in `debounce_active` indefinitely.

A second test should preserve existing debounce behavior for ordinary queue notifications so the fix does not remove batching for real user queue edits.
