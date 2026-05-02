---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/command_handlers.rs
  - src/tui/orchestrator.rs
  - src/parallel_run_service.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: Preserve manual resolve scheduler state

**Change Type**: implementation

## Premise / Context

- In the avacuscc-dbot run, pressing `M` on `refactor-agent-tools-error-handling` logged `ResolveMerge(...)` and `started scheduler for manual resolve`.
- The same log immediately showed `No committed changes available for parallel execution`, followed by zero-change completion, so the scheduler never consumed the manual resolve work.
- Code inspection shows `TuiCommand::ResolveMerge` records reducer-owned `ResolveWait`, then starts `run_orchestrator_parallel(Vec::new(), ...)` when no scheduler is running.
- `run_orchestrator_parallel` currently recreates `OrchestratorState::with_mode(vec![], ...)` at startup, which discards the just-recorded reducer-owned `ResolveWait` before `ParallelRunService` can detect it.
- Existing specs already require reducer-owned `ResolveWait` to be schedulable work even with no active change list; this proposal makes the startup path actually preserve that state.
- The Conflux Constitution allows UI/shared reducer state as runtime coordination, but workflow-control decisions must remain derivable from workspace/git/base-tree evidence; this change must not introduce durable out-of-worktree workflow-control state.

## Requested Artifact

- implementation proposal to fix manual `M` resolve startup from `merge wait` rows when no parallel scheduler is running
- preserve reducer-owned `ResolveWait` across empty scheduler startup
- add regression tests that reproduce the observed zero-change completion failure and prove the scheduler reaches retry dispatch

## Problem

Manual resolve from a `merge wait` row is designed as scheduler-owned reducer intent. When the scheduler is already running, the TUI can notify it and the existing loop can synchronize `ResolveWait`. When the scheduler is not running, the TUI starts a scheduler-owned run with an empty active change list. That path currently resets shared reducer state to an empty `OrchestratorState`, deleting the `ResolveWait` intent that was just recorded.

The result is a user-visible trap: the row moves toward `resolve pending`, but the scheduler exits as a zero-change no-op before it can attempt the preserved-worktree merge retry.

## Proposed Solution

Preserve existing reducer-owned scheduler work when `run_orchestrator_parallel` is started solely to consume manual resolve intent.

1. Detect the empty-manual-resolve startup case before resetting shared state: active `change_ids` is empty and the existing shared reducer has one or more `ResolveWait` IDs.
2. In that case, do not replace shared state with `OrchestratorState::with_mode(vec![], ...)`.
3. Keep the normal state reset path for ordinary parallel starts with selected changes, and for empty starts without `ResolveWait`.
4. Ensure `ParallelRunService` sees `executor.has_resolve_wait() == true`, allowing the existing empty-queue scheduler path to continue into retry dispatch.
5. Ensure completion events do not report success before reducer-owned `ResolveWait` has either been attempted or explicitly cleared by merge/resolve failure semantics.
6. Add regression tests around the TUI command handler/orchestrator startup boundary so a future refactor cannot reintroduce zero-change completion before `ResolveWait` synchronization.

## Acceptance Criteria

- Pressing `M` on a `merge wait` row while no scheduler is running preserves the reducer-owned `ResolveWait` intent across scheduler startup.
- The empty scheduler run reaches `ParallelRunService` with `allow_empty_when_resolve_wait = true` and does not return solely because the active change list is empty.
- The scheduler synchronizes `ResolveWait` from shared state and attempts deferred merge retry for the target change when a preserved worktree exists.
- If there is no reducer-owned `ResolveWait`, an empty scheduler startup remains a safe no-op and does not dispatch apply, merge retry, or resolve work.
- Ordinary parallel starts with selected changes continue to reset shared state for that run and re-apply selected queue intent as before.
- The fix does not introduce or rely on durable out-of-worktree workflow-control state.

## Explicit Completion Conditions

- `src/tui/orchestrator.rs` preserves shared reducer state for empty manual resolve scheduler startup when `resolve_wait_change_ids()` is non-empty, while leaving ordinary selected-change startup behavior unchanged.
- `src/tui/command_handlers.rs` and/or related tests prove `TuiCommand::ResolveMerge` with `orchestrator_running = false` leaves shared reducer status as `resolve pending` after scheduler startup begins.
- `src/parallel_run_service.rs` tests prove an empty active change list with reducer-owned `ResolveWait` enters the scheduler path rather than emitting only `No committed changes available for parallel execution`.
- Scheduler/retry tests prove `sync_resolve_wait_from_shared_state_nonblocking()` and `maybe_dispatch_resolve_wait_retry()` are reached for the manual startup path.
- Regression verification includes the log-observable condition: manual resolve startup must produce the “continuing with empty queue / ResolveWait retry” path, not only zero-change completion.
- `cflx openspec validate fix-manual-resolve-preserve-state --strict --evidence warn` passes.

## Out of Scope

- Changing how merge conflicts are resolved once the scheduler starts retry work.
- Changing the meaning of `merge wait`, `resolve pending`, or terminal `merged` beyond preserving already-required reducer-owned intent.
- Introducing durable scheduler queues outside workspace/git/base-tree evidence.
- Cleaning up stale process instances or changing deployment/release packaging for `cflx`.
