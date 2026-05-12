# Design: Fix Error Retry Gating

## Classification

Requested artifact: implementation.

This is a behavior fix, not a spec-only change. Runtime scheduler and UI retry wiring must change alongside OpenSpec deltas.

## Premise / Context

- The Constitution requires workflow-control decisions to be derivable from workspace file state, workspace git state, and base-branch tree comparison. This change must not introduce external durable retry state.
- `TerminalState::Error` already exists in reducer-owned orchestration state and is the natural in-memory representation for the current run.
- Current specs allow delayed success events to supersede recoverable errors. This must be preserved for events from already-running work.
- TUI already communicates that error rows require a retry mark, so implementation should align scheduler behavior with that UI contract.
- `FailedChangeTracker` currently helps skip dependents but is not sufficient as the authoritative same-change dispatch gate.

## Approach

### Error as a dispatch gate

Treat reducer terminal error as a hard gate for ordinary apply dispatch. The gate applies when selecting queued changes, reconciling queue state, scanning existing worktrees, or dispatching repair/resume candidates.

The gate must be checked close enough to dispatch that stale local queues cannot bypass it. Upstream filtering is useful but not sufficient by itself.

### Explicit retry transition

Add one explicit retry transition for recoverable error state. The transition should:

- apply only to `TerminalState::Error`;
- clear terminal error and stale blocked metadata;
- restore queued intent for the selected change;
- clear stale per-change failure bookkeeping that would otherwise immediately skip the retry;
- leave `Rejected`, `Merged`, and `Archived` terminal states immutable.

TUI retry-mark/F5 should call this reducer transition instead of only updating local display state. Any CLI/server retry path should use the same semantics when applicable.

### Delayed success remains authoritative

A delayed `ChangeArchived`, `MergeCompleted`, or `ResolveCompleted` event can still supersede a recoverable error for the same change. This handles races where an error observation arrives before a more authoritative repository-visible success observation.

That supersession must not imply that the scheduler may create a new apply dispatch. It is a reducer event reconciliation rule, not retry intent.

### Dependency behavior

When a dependency errors, dependents should remain blocked/skipped. Once the dependency is explicitly retried and reaches repository-visible success, normal dependency analysis can unblock dependents. This avoids both infinite loops and permanent poisoning after successful retry.

## Trade-offs

- A single error stops automatic retries earlier than the existing same-error circuit breaker. This matches operator expectations and TUI wording, while keeping the circuit breaker as secondary protection for repeated explicit retries.
- The reducer remains the best source of current-run dispatch state, but the Constitution still forbids adding out-of-worktree durable workflow-control files. Resume behavior must derive from existing workspace/git/base evidence and should not depend on logs or caches.

## Verification Strategy

- Unit tests for reducer transitions: error gate, explicit retry, immutable final states, delayed success supersession.
- Integration-style tests for parallel scheduler/reanalysis and workspace resume: an errored change is not redispatched until explicit retry.
- TUI state tests for retry-mark/F5: selected error rows requeue, unselected error rows stay stopped.
- Dependency tests: dependents blocked by errored dependency and unblocked after explicit retry success.
