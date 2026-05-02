# Design: Deferred merge retry idempotency for on_merged

## Background

Deferred merge retry bridges three concepts:

- scheduler-local retry sets (`resolve_wait_changes`)
- reducer/shared lifecycle state used by TUI and scheduler synchronization
- merge side effects, including `on_merged` hooks and workspace cleanup

The observed failure happened when these concepts disagreed: local retry state was cleared after success, but shared resolve-wait intent could still be synchronized back into the scheduler, causing the same change to retry again.

## Design Goals

- Treat successful merge integration as a terminal transition for retry intent.
- Keep `on_merged` timing unchanged: after repository-visible merge success, before `MergeCompleted`.
- Avoid persistent out-of-worktree workflow-control state.
- Make retry handling robust to repeated scheduler triggers, queue notifications, and stale in-memory entries.

## Proposed Approach

### 1. Clear retry intent at the source of truth

When deferred retry succeeds, remove the change from local retry sets and from reducer-owned shared state before another scheduler sync occurs. If shared state exposes only read APIs today, add a narrow mutation/event path that records successful merge completion and removes resolve-wait intent as part of the same transition.

### 2. Add a stale retry guard

Before running merge work for a deferred retry entry, check repository-visible evidence that the change is already merged. If already merged, remove the retry intent and do not run `on_merged`.

### 3. Preserve hook semantics

Do not move `on_merged` after `MergeCompleted`. Instead, make the retry queue idempotent so a second hook call is not reachable for the same merge success.

## Alternatives Considered

- Persistent lock files for `on_merged`: rejected because the constitution forbids out-of-worktree durable workflow-control state.
- Moving `on_merged` after `MergeCompleted`: rejected because existing hooks spec requires it before merged status transition.
- Making `scripts/bump.sh` detect duplicate releases: useful future hardening, but it does not fix orchestration retry correctness.

## Verification Strategy

- Unit-level reducer/shared-state test for resolve-wait cleanup on merge completion.
- Integration-level scheduler/deferred retry test that fires a second retry trigger after success and asserts no merge or hook repeat.
- Existing merge conflict tests continue to prove true conflicts still enter resolve and conflictless merge-ready states complete normally.
