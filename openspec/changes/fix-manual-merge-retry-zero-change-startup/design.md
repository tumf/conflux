# Design: Manual merge retry zero-change startup

## Background

Manual merge retry is intentionally scheduler-owned. The TUI command path records `ReducerCommand::ResolveMerge(change_id)` and either wakes an existing scheduler or starts a scheduler with no normal queued changes. That empty startup is valid only when shared reducer state contains lane-wait retry work that the scheduler can consume.

The observed failure mode is a restarted TUI where pressing `M` logs a manual resolve scheduler startup, then immediately completes as `0 changes processed`. That behavior contradicts the scheduler-owned retry model because the run accepted retry intent but did not consume it or visibly demote it.

## Design Principles

- Fix the ownership model, not only the observed wakeup symptom: manual retry intent has one authoritative lifecycle owner.
- `M` remains intent-only; the TUI must not directly run merge/resolve work.
- The scheduler/executor must use the same shared reducer state that accepted the retry intent.
- Executor-local retry sets are caches synchronized from reducer state, not competing truth sources.
- Empty normal queue does not mean no work when reducer-owned `ResolveWait` / `RejectWait` exists.
- Completion logs and `AllCompleted` must be truthful and based on observable reducer/scheduler state.
- Missing or stale workspace evidence must produce visible state, not silent pending.
- No durable out-of-worktree workflow state may become authoritative.
- Regression coverage must include adjacent paths so the fix cannot break active scheduler notification, non-empty queue dispatch, dirty-base demotion, auto-resumable deferral, or merged-state non-regression.

## Affected Flow

1. User presses `M` on `merge wait`.
2. TUI applies `ReducerCommand::ResolveMerge` to shared reducer state.
3. If the scheduler is stopped, TUI starts `run_orchestrator_parallel(Vec::new(), ...)`.
4. Parallel run service creates/configures the executor.
5. Executor syncs `ResolveWait` / `RejectWait` from shared state.
6. Scheduler evaluates base-lane waiters before idle completion.
7. Retry either starts, completes, demotes to manual wait, or surfaces an error/stalled state.
8. Completion is emitted only after queued, active, pending merge, manual resolve, and shared lane-wait state are drained.

## Key Implementation Constraints

### Shared state ownership

`ParallelRunService` must not replace an executor's caller-provided shared reducer state with an internal empty reducer during `run_parallel_order_based_with_executor`. If a service-level shared state is needed, it must be initialized from or explicitly set to the caller-provided state before executor execution.

### Empty startup handling

`prepare_parallel_execution(..., allow_empty_when_resolve_wait=true)` may allow an empty normal change list, but the subsequent scheduler loop must prove lane-wait work exists and dispatch/evaluate it. The empty path must not be treated as ordinary no-op execution.

### Completion semantics

TUI completion handling should consult shared reducer state and scheduler-local wait/active counters before emitting success logs. The user-visible success sequence must be suppressed when retry membership remains pending or an active base-mutating lifecycle still exists.

### Stale retry evidence

When retry dispatch cannot find a valid archived workspace or workspace path, the implementation should emit reducer-visible evidence that clears `ResolveWait` and surfaces a recoverable manual/actionable state. It must not only clear local executor sets while leaving the TUI display at `resolve pending`.

## Verification Strategy

- Unit tests for shared-state preservation through service/executor construction.
- Integration tests for empty startup with reducer-owned `ResolveWait` proving retry evaluation happens.
- TUI/event tests for truthful completion suppression while retry work remains.
- Regression tests for missing/stale workspace and dirty-base demotion.
- Existing active scheduler and non-empty queue tests retained to prevent behavior regressions.
