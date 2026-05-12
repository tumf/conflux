# Design

## Root Cause

The TUI currently tracks a local, non-durable `is_resolving` flag to serialize manual resolve requests and decide whether `resolve_merge()` should return `TuiCommand::ResolveMerge` immediately or only queue the row locally.

That local flag is set before actual execution starts so fast consecutive `M` key presses cannot dispatch multiple resolve commands. This is intentional and covered by the existing `resolve-merge-exclusive-execution` requirement.

The bug appears because the success event that closes a manual merge retry is not always `ResolveCompleted`.

Parallel manual merge retry flow:

1. User presses `M` on a `merge wait` row.
2. TUI sets `is_resolving = true`, displays `resolve pending`, and emits `TuiCommand::ResolveMerge`.
3. Scheduler consumes reducer-owned `ResolveWait` and calls merge retry.
4. Merge retry emits `ResolveStarted`, then successful integration emits `MergeCompleted`.
5. TUI marks the row `merged` but does not clear `is_resolving` or drain queued resolve work.

The stale flag causes the next `M` press to take the queued-only branch in `resolve_merge()`, returning `None`. Since scheduler notification is performed by `handle_tui_command(TuiCommand::ResolveMerge)`, no notification occurs.

## Design Approach

Treat `MergeCompleted` as a valid resolve lifecycle completion event when it follows a TUI manual merge retry.

The implementation should prefer a small helper that performs the common local completion behavior:

- clear `is_resolving`
- pop the next TUI-local queued resolve item, if any
- set that next row to `resolve pending`
- return `Some(TuiCommand::ResolveMerge(next_id))` when queued work exists
- otherwise transition back to select mode when appropriate

`handle_resolve_completed()` can continue to do its existing row-specific success handling, then call the helper. `handle_merge_completed()` should do its existing row-specific merge success handling, then call the same helper when the local state indicates a resolve lifecycle was active or queued.

## Boundaries

This change must not move workflow authority into TUI state. The TUI flag and queue are coordination/display helpers only. Reducer-owned `ResolveWait` remains the scheduler-consumable source of retry intent.

The fix should not change `OrchestratorState`, merge conflict resolution, or workspace status classification unless tests prove the stale flag fix is insufficient.

## Verification Strategy

Use unit tests for the TUI state machine because the root cause is local event handling, not git behavior.

Required regression coverage:

- `MergeCompleted` clears `is_resolving` after a manual retry lifecycle.
- `MergeCompleted` drains the queued resolve item and returns `TuiCommand::ResolveMerge`.
- `handle_orchestrator_event()` propagates that command from the `MergeCompleted` arm.
- A later `resolve_merge()` call after `MergeCompleted` returns `Some(TuiCommand::ResolveMerge)` instead of false-queueing and returning `None`.

A manual TUI smoke test remains useful because the original symptom is interactive and timing-sensitive, but the primary regression is deterministic and should be covered by unit tests.
