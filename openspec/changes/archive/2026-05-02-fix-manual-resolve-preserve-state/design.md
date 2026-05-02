# Design: Manual Resolve Scheduler State Preservation

## Context

Manual resolve from the TUI is intentionally reducer-owned scheduler work. The TUI records `ReducerCommand::ResolveMerge(change_id)`, which turns an existing `MergeWait` row into `ResolveWait`, then either wakes the running scheduler or starts a scheduler-owned run when no scheduler is active.

The broken path is the idle case:

1. user presses `M` on a `merge wait` row
2. command handler records `ResolveWait`
3. command handler starts `run_orchestrator_parallel(Vec::new(), ...)`
4. orchestrator startup replaces shared state with `OrchestratorState::with_mode(vec![], ...)`
5. `ResolveWait` is lost
6. `ParallelRunService` sees no active changes and no reducer-owned work, then returns as a zero-change no-op

The fix must preserve the reducer intent only for this specific empty manual resolve startup. It must not weaken normal run initialization for selected changes.

## Design Goals

- Preserve reducer-owned `ResolveWait` long enough for the scheduler to synchronize it.
- Keep normal selected-change startup deterministic and fresh.
- Keep empty startup without `ResolveWait` as a safe no-op.
- Avoid adding durable queues or state outside workspace/git/base-tree evidence.
- Make the failure mode testable without requiring a real long-running TUI session.

## Startup Classification

At the start of `run_orchestrator_parallel`, classify startup into three cases:

1. **Selected-change startup**: `change_ids` is non-empty.
   - Reset shared state to `OrchestratorState::with_mode(change_ids, ...)`.
   - Re-apply `AddToQueue` for selected IDs.
   - Existing behavior remains unchanged.

2. **Empty manual resolve startup**: `change_ids` is empty and existing shared reducer has at least one `ResolveWait` ID.
   - Do not reset shared state.
   - Ensure execution mode is compatible with parallel scheduling if needed without dropping runtime entries.
   - Continue startup so `ParallelRunService` can create an executor with the preserved shared state.

3. **Truly empty startup**: `change_ids` is empty and shared reducer has no `ResolveWait` IDs.
   - Existing no-op behavior remains valid.
   - No apply or merge retry work is dispatched.

## Scheduler Handoff

`ParallelRunService::run_parallel_order_based_with_executor` already computes:

```rust
let allow_empty_when_resolve_wait = changes.is_empty() && executor.has_resolve_wait();
```

This design makes that predicate reachable from the TUI idle manual resolve path by ensuring `executor.has_resolve_wait()` observes the reducer intent that was recorded before scheduler startup.

Once the executor begins, existing loop behavior should remain the primary owner:

- `sync_resolve_wait_from_shared_state_nonblocking()` copies reducer `ResolveWait` IDs into executor retry state.
- `maybe_dispatch_resolve_wait_retry()` calls `retry_deferred_merges()` when needed.
- merge/resolve success or failure events update reducer-owned lifecycle state.

## Constitution Compatibility

The preserved reducer state is runtime coordination for an already-observed workspace/git condition. The authoritative decision to retry still comes from repository-visible artifacts: archived worktree state, branch/ahead state, base branch merge state, and git merge results.

This change does not introduce durable workflow-control state under `~/.local/state/cflx/**` or elsewhere.

## Failure Handling

If scheduler retry cannot proceed because the preserved worktree is missing or merge is still blocked, that should be handled by existing retry/deferred/failure paths or by follow-up changes. This proposal only fixes the startup intent loss that prevents the scheduler from attempting the work at all.

## Verification Strategy

Use unit/integration tests around the startup boundary instead of relying on an interactive TUI session:

- command handler idle `ResolveMerge` handoff test
- orchestrator startup state-preservation test
- parallel run service empty-ResolveWait test
- ordinary empty no-op and selected-change reset tests

The regression should fail if a future implementation resets shared state before `ParallelRunService` can observe reducer-owned `ResolveWait`.
