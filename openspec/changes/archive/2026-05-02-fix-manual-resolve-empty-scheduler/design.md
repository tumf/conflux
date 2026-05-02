# Design: Manual resolve startup with empty parallel inputs

## Background

Manual resolve in the TUI is not a normal apply run. The selected change is already archive-complete, so it may no longer exist under active `openspec/changes/<change_id>/`. The actionable intent lives in reducer-owned runtime state: `MergeWait` becomes `ResolveWait` when the user presses `M`.

The existing scheduler can already process `ResolveWait` once it is running:

- sync reducer-owned `ResolveWait` into `ParallelExecutor.resolve_wait_changes`
- dispatch `retry_deferred_merges()`
- locate the preserved worktree
- attempt merge / conflict resolution

The bug is before that point: `ParallelRunService` treats empty input changes as no work and returns before the scheduler loop can sync reducer state.

## Design Goals

- Keep normal parallel apply filtering unchanged.
- Allow empty input changes to mean "scheduler-only retry work" when reducer-owned `ResolveWait` exists.
- Avoid introducing durable workflow-control state outside the workspace/git/reducer runtime model.
- Preserve existing conflictless merge and true-conflict resolve semantics.

## Recommended Shape

Add a helper around `ParallelRunService` startup that can answer whether scheduler-only work exists:

- `has_shared_resolve_wait(shared_orchestrator_state) -> bool`, or equivalent executor-owned check.
- If `prepare_parallel_execution()` returns `None` because no committed changes remain, check for shared `ResolveWait` before returning.
- If shared `ResolveWait` exists, enter the executor scheduler path with an empty queued list and persistent lifetime when called from TUI/server queue mode.

The scheduler loop already checks `self.resolve_wait_changes` before drained/idle exit after calling `sync_resolve_wait_from_shared_state_nonblocking()`. The implementation should preserve that ordering.

## Alternative

Create a dedicated `run_resolve_wait_retry_with_channel_and_queue_state()` entrypoint. This is cleaner at the API level, but it may duplicate event forwarding and executor construction wiring. Use it only if the minimal branch would make `run_parallel_order_based_with_executor()` harder to reason about.

## Verification Strategy

Use tests that fail before the fix:

1. Empty `changes` with shared reducer `ResolveWait` must not return before retry dispatch.
2. Empty `changes` without shared reducer `ResolveWait` remains a no-op.
3. TUI `ResolveMerge` idle path starts a run that reaches scheduler retry rather than logging only `0 change(s)` completion.

Avoid tests that rely on external logs as authoritative state. Logs may be asserted as observability, but workflow behavior must be proven through events, reducer state, or mock workspace manager calls.
