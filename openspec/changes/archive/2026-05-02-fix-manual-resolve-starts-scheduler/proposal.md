---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - src/tui/state.rs
  - src/tui/key_handlers.rs
  - src/tui/command_handlers.rs
  - src/tui/runner.rs
  - src/tui/orchestrator.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
---

# Fix Manual Resolve Starts Scheduler

**Change Type**: implementation

## Problem / Context

Manual merge-wait retry still fails in a real TUI session when the parallel scheduler is not currently running.

Observed in `/Users/tumf/wakumo/avacus/avacuscc-dbot` with `cflx v0.6.50`:

- `refactor-agent-tools-error-handling` is detected as archive-complete `merge wait`.
- Pressing `M` logs `Scheduled merge-wait retry intent for 'refactor-agent-tools-error-handling'; execution will be started by scheduler`.
- No subsequent `Queue notification received`, `Retrying deferred merge`, `MergeStarted`, `ResolveStarted`, `MergeDeferred`, `ResolveFailed`, or `MergeCompleted` appears.

The latest scheduler dispatch fix handles the case where a persistent scheduler is alive and receives a notification. This bug is the missing startup/resume case: `ResolveMerge` can be recorded while no scheduler task exists to consume the notification.

## Proposed Solution

Make manual resolve retry self-starting when no live scheduler exists.

The TUI command pipeline should distinguish two cases:

1. A live parallel scheduler exists: record reducer-owned `ResolveWait` intent and notify the scheduler.
2. No live scheduler exists, or the prior scheduler task has finished: record reducer-owned `ResolveWait` intent and start a parallel scheduler run capable of consuming existing `ResolveWait` intent without requiring new apply/archive queue work.

The solution must keep execution ownership in the scheduler. It must not reintroduce direct TUI execution of merge resolution.

The implementation should also make logs truthful. If `ResolveMerge` only records intent because no scheduler could be started, the log must not claim that execution will be started by scheduler.

## Acceptance Criteria

- Pressing `M` on a `merge wait` row while no orchestrator task is running starts or requests a parallel scheduler run.
- The new scheduler run consumes reducer-owned `ResolveWait` intent and reaches the existing scheduler-owned retry dispatch path.
- The TUI no longer logs `execution will be started by scheduler` unless there is an active scheduler or one was successfully started.
- Existing behavior when a scheduler is already running remains unchanged: `M` records reducer intent, wakes the scheduler, and does not spawn a duplicate scheduler.
- The avacus scenario produces a retry/defer/resolve/merge/error outcome after the scheduled intent rather than staying in pending with no scheduler log.

## Explicit Completion Conditions

- `src/tui/runner.rs` passes enough orchestrator-handle state into command handling for `TuiCommand::ResolveMerge` to know whether a live scheduler exists, or otherwise routes `ResolveMerge` through a start-capable command path.
- `src/tui/command_handlers.rs` starts `run_orchestrator_parallel` or an equivalent scheduler-owned run when handling `ResolveMerge` with no live scheduler.
- The started scheduler does not require the manual-resolve change to be `not queued`; it can consume reducer-owned `ResolveWait` intent for an archived `merge wait` worktree.
- Unit/integration tests cover both live-scheduler notification and no-scheduler startup paths.
- Manual verification in `/Users/tumf/wakumo/avacus/avacuscc-dbot` confirms `M` on `refactor-agent-tools-error-handling` produces scheduler retry/defer/resolve output after the scheduled-intent log.

## Out of Scope

- Directly resolving merge conflicts from the TUI command handler.
- Changing conflict-resolution semantics after scheduler retry begins.
- Rewriting the scheduler lifetime model beyond what is needed to start/route manual resolve work.
- Adding durable workflow-control state outside workspace/git/base-tree inputs.
