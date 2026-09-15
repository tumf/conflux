# Design: Lifecycle-owned concurrency slots

## Decision

Concurrency belongs to an admitted change lifecycle, not to an individual async task or phase counter.

One owned permit is acquired before workspace preparation and moves with the change through:

1. workspace preparation;
2. apply;
3. acceptance;
4. archive;
5. background merge;
6. automatic or manual resolve;
7. terminal settlement and cleanup.

The permit is transferred across task boundaries. It is never dropped and reacquired during a continuously admitted lifecycle.

## Settlement boundary

Release occurs only after repository-visible evidence establishes one of:

- merged and cleaned up;
- terminal error or rejection;
- explicit dequeue/not-queued settlement;
- configured push/publication terminal settlement.

`merge_wait` is not a release boundary. It preserves backpressure so unresolved old-baseline work prevents admission of newer divergent worktrees. Operator resolve continues under the retained slot.

## Runtime accounting

Use one authoritative change-ID membership for admission and running counts. Phase counters remain observability or wake-edge mechanisms and must not subtract capacity for a change already represented by lifecycle membership.

Avoid a second semaphore for merge/resolve. Multiple independent permits recreate the transfer race and allow double-counting.

## Restart recovery

The lifecycle membership is process-local, but recovery derives it from repository evidence:

- retained managed workspace;
- Git merge/integration evidence;
- reducer-owned active, resolving, queue, and wait dispositions.

No slot lease or next-action record is written outside the workspace. Ambiguous evidence fails closed for new dispatch until classification completes.

## Liveness

Terminal settlement emits or preserves the existing completion/slot-recovery wake edge. A retained `merge_wait` can intentionally consume all configured capacity; this is backpressure, not scheduler deadlock. Operator resolution, dequeue, rejection, or terminal failure releases the slot.

## Rejected alternatives

### Subtract `pending_merge_count`

Rejected. A global count cannot bind occupancy to change identity, risks double-counting auto resolve, and still leaves manual resolve admission ambiguous.

### Increment resolve counters earlier

Rejected. It narrows one gap but does not cover conflict-free background merge or preserve ownership through all terminal paths.

### Release at `merge_wait` and reacquire later

Rejected for this change. It admits newer branches while unresolved work remains based on an older baseline, recreating the reported merge-divergence problem.
