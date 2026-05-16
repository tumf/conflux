# Design: Persistent idle scheduler waits without scan polling

## Current behavior

`ParallelExecutor::execute_with_order_based_reanalysis` runs a scheduler loop that always reaches a `tokio::select!` containing a 500ms sleep. When the scheduler is persistent and all work is drained, the finite exit path is skipped, the sleep fires, and the loop starts over.

At loop start, the scheduler checks dynamic queue entries, syncs reducer-owned wait state, and reconciles queued candidates from shared state. Queue reconciliation can scan worktrees and evaluate archived dirty repair candidates. This is correct while work may exist, but wasteful when the scheduler is fully idle and waiting only for new notifications.

## Design decision

Persistent idle should be a separate event-driven wait state.

The scheduler already has explicit notification sources:

- `DynamicQueue::push` wakes the scheduler when a user queues a change.
- `DynamicQueue::notify_scheduler` wakes the scheduler when a scheduler-owned retry should be reconsidered without adding a queue item.
- merge result channels wake the scheduler when background merge work finishes.

The idle path should wait on these sources instead of the debounce timer. The debounce timer remains useful only while queued work exists, capacity may recover, or recent queue edits should be coalesced.

## State model

The change must not add durable workflow-control state. Idle detection is derived from existing in-memory scheduler state:

- local queued list
- in-flight set
- reducer-owned resolve/reject wait snapshots
- manual resolve counter
- pending merge counter
- scheduler lifetime policy

Workflow routing remains governed by workspace files, workspace git state, base-branch tree comparison, and existing reducer-owned runtime state. Logs and counters remain observability-only.

## Verification strategy

Testing should avoid relying only on wall-clock log inspection. Prefer a test seam or counter-backed test manager that proves reconciliation/list-worktree work is not called repeatedly while persistent idle has no wake signal.

A second test should prove that a queue notification wakes the scheduler and lets normal queue ingestion/reanalysis resume. A third check should preserve finite drained exit behavior.
