## Context

The scheduler has reducer-owned queue intent and a scheduler-local candidate vector. Failed-dependency filtering removes only the local candidate. Reconciliation restores it from reducer intent and reports a false new queue edge. `QueueNotification` correctly bypasses matching-signature suppression for genuine additions, so global suppression changes are the wrong layer.

A second boundary matters: runtime retry admission currently wakes the scheduler without carrying the retried change ID or whether `RetryError` changed reducer state. `FailedChangeTracker` therefore cannot safely distinguish accepted retry from generic notification.

## Decision

Represent failed-dependent work as stable blocked queued work and introduce a target-ID-bearing one-shot retry edge.

- Reducer queue intent remains authoritative.
- Local candidates retain failed-dependent entries until explicit dequeue/revocation.
- Dispatch classification excludes blocked entries but continues independent work.
- Reconciliation reports additions only for genuinely absent admitted IDs.
- `ChangeSkipped` remains a once-per-blocker-epoch compatibility observation and does not deselect or revoke queue intent. `DependencyBlocked` owns blocked state/presentation.
- Only `RetryError(id)` with `ReduceOutcome::Changed` publishes `ExplicitRetry(id)` to the live scheduler.
- The scheduler consumes `ExplicitRetry(id)` before reconciliation/classification, clears only `id` from the ephemeral failed tracker and clears dependent blocker-notification epochs involving `id`, then arms one reevaluation.
- Refused/no-op retry, `AddToQueue`, and generic `QueueNotification` never clear failed state.
- Retry clearing removes a fast failure gate only. Repository and dependency evidence still decides resolution.

## State Transitions

1. A fails: `mark_failed(A)`.
2. B depends on A: retain B locally; emit exactly one `ChangeSkipped(B,A)` and one `DependencyBlocked(B)` for this blocker epoch.
3. Unchanged wake: no local removal, reconciliation addition, queue edge, analyzer attempt, or duplicate event.
4. Genuine C addition or relevant signature change: preserve normal analysis and dispatch; B remains blocked.
5. Accepted `RetryError(A)`: publish one `ExplicitRetry(A)` edge.
6. Scheduler consumes the edge before classification: clear A's ephemeral failure and the A-related notification epoch; reevaluate once.
7. A queued/in-flight/unmerged: B remains blocked through normal dependency checks.
8. A resolves authoritatively: B becomes dispatchable through normal checks.
9. A fails again: establish a new failed-blocker epoch and bounded observations.
10. B is dequeued: remove B locally and clear B's blocker epoch. Explicit re-add is a genuine queue addition.
11. Process restart: ephemeral tracker and epochs are empty; workspace and Git evidence recompute routing.

## Event Semantics

`ChangeSkipped` means one dispatch exclusion observation for a failed dependency. It does not mean queue intent was revoked. TUI and other consumers must not set selection false solely from this compatibility event when reducer intent remains queued.

`DependencyBlocked` is the authoritative blocked transition. It is emitted once for a stable blocker fingerprint. A changed blocker set, accepted retry followed by refailure, or dequeue followed by explicit re-add starts a new epoch.

## Scheduler Lifetime

- Mixed B/C queue: C remains analyzable and dispatchable.
- Finite scheduler with only B: return blocked/stalled, never `AllCompleted`.
- Persistent scheduler with only B: wait for explicit queue/retry notifications without timer-driven polling.
- Genuine dynamic additions retain one immediate matching-signature bypass.

## Alternatives Rejected

### Suppress every matching `QueueNotification`

Breaks genuine new-work latency and existing edge semantics.

### Remove reducer queue intent for B

Silently discards operator intent and prevents automatic recovery.

### Deduplicate only logs

Leaves analysis, reconciliation, and repository churn intact.

### Infer retry from every queued state

Cannot distinguish explicit retry from ordinary add/no-op notification and may clear failures incorrectly.

## Verification Strategy

Use paused Tokio time and the real scheduler loop. Assert exact lower and upper bounds: initial analyzer count exactly one where analysis is required, each event exactly once per epoch, stable `queued_added == 0`, and B remains locally represented. These assertions make zero-analysis stubs, log-only patches, analyzer-only suppression, and retained local churn fail.

Retry coverage must reuse the same persistent executor and enter through production `RetryError`, separately testing changed, no-op/refused, unresolved, resolved, and refailed outcomes. Additional tests cover dynamic C addition, dequeue/re-add, finite/persistent lifetime, and restart recomputation.
