# Design: Manual Resolve Starts Scheduler

## Current Flow

`AppState::resolve_merge()` moves a selected `merge wait` row to `resolve pending`, records `ResolveMerge` in shared reducer state, and emits `TuiCommand::ResolveMerge`.

`TuiCommand::ResolveMerge` currently applies the same reducer command again, calls `DynamicQueue::notify_scheduler()`, and logs that scheduler execution will start.

That only works if a persistent scheduler task is already waiting on the same `DynamicQueue`. In the observed avacus session, the log has the scheduled-intent message but no scheduler wake/retry logs, which means no live scheduler consumed the notification.

## Design Goal

Manual resolve must be start-capable while preserving ownership:

- TUI records user intent.
- Reducer stores `ResolveWait`.
- Scheduler executes retry.
- Events update terminal/deferred/failure state.

The TUI command handler may start a scheduler task, but it must not execute merge/resolve itself.

## Runtime Model

Command handling should know whether the orchestrator task is live.

If live:

1. Apply `ReducerCommand::ResolveMerge`.
2. Notify dynamic queue.
3. Log that an existing scheduler was notified.

If not live:

1. Apply `ReducerCommand::ResolveMerge`.
2. Start a parallel scheduler run with the existing shared state and dynamic queue.
3. Ensure the scheduler starts in persistent/scheduler-owned mode and can consume `ResolveWait` intent without a fresh apply/archive queue target.
4. Return the spawned `JoinHandle` to the runner so later commands can detect liveness.
5. Log that a scheduler was started for manual resolve.

## Important Constraint

The started scheduler must not reset shared state in a way that erases the just-recorded `ResolveWait` intent.

`run_orchestrator_parallel()` currently initializes shared state with `OrchestratorState::with_mode(...)` for selected IDs and reapplies `AddToQueue` for those IDs. If manual resolve startup reuses that path naïvely with an empty or non-queued ID list, it may erase `ResolveWait` before the scheduler can consume it.

Acceptable implementation approaches:

- add a manual-resolve scheduler startup mode that preserves existing shared reducer state;
- add a scheduler entrypoint that does not reset shared state when the purpose is consuming existing `ResolveWait` intent;
- or seed the new scheduler state with the archived change and immediately reapply `ResolveMerge` after reset before the loop starts.

The first two are cleaner. The third is acceptable only if tests prove `ResolveWait` survives startup and refresh.

## Verification Strategy

The tests must prove both halves:

- no live scheduler: `ResolveMerge` starts a scheduler and retains `ResolveWait` until retry dispatch;
- live scheduler: `ResolveMerge` only notifies and does not spawn duplicate work.

The avacus manual run remains required because the original failure was a real TUI lifecycle gap, not just a pure reducer/scheduler unit bug.
