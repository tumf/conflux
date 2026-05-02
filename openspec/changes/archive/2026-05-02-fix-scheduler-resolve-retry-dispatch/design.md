# Design: Scheduler Resolve Retry Dispatch

## Current Failure Mode

The scheduler now syncs reducer-owned `ResolveWait` intent into `resolve_wait_changes`, but that set is only used to avoid false drain/exit. The main loop only dispatches normal apply/archive work when `queued` is non-empty. If the scheduler is idle and the user presses `M`, the wake-up path logs that a queue notification was received, then returns to the top of the loop. It syncs intent, sees no queued work, does not call retry, and waits again.

So the state is visible but not executable.

## Desired Ownership

The desired split remains:

- TUI: user intent capture and scheduler wake-up.
- Reducer: authoritative lifecycle intent and display status.
- Scheduler: execution of merge/resolve retry.
- Events: terminal/deferred/failure lifecycle updates.

No direct TUI merge execution should be added.

## Dispatch Model

The scheduler needs a retry dispatch phase separate from apply/archive dispatch:

1. Check dynamic queue additions/removals.
2. Sync reducer-owned `ResolveWait` intent.
3. If retry intent is newly triggered or otherwise eligible, run `retry_deferred_merges().await`.
4. Then evaluate drain/idle and normal queued apply/archive dispatch.
5. Wait for task, merge result, queue notification, or timer.

This keeps retry execution in the scheduler while making manual `M` actionable.

## Busy Retry Guard

A naive loop that calls `retry_deferred_merges()` whenever `resolve_wait_changes` is non-empty can repeatedly retry every timer tick. The implementation needs a guard based on trigger, state transition, or attempt bookkeeping.

Acceptable minimal strategies include:

- only dispatch synced retry immediately after a queue/scheduler notification or explicit retry trigger;
- track the last attempted resolve-wait set and retry again only when the set changes or a merge/resolve/reject completion occurs;
- introduce a small in-memory retry trigger flag set by `notify_scheduler()` handling and completion handlers.

The guard must not be durable workflow-control state. It is scheduler-local runtime coordination only.

## Verification Notes

Tests should prove the old incomplete fix fails: reducer-owned `ResolveWait` is visible, but no retry attempt occurs when no queued work exists. The new tests should assert that retry dispatch is invoked through scheduler-owned code, not TUI direct execution.
