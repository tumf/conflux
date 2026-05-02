---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - src/tui/command_handlers.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
  - src/parallel/tests/executor.rs
---

# Fix Scheduler Resolve Retry Dispatch

**Change Type**: implementation

## Problem / Context

The previous manual resolve wait change made reducer-owned `ResolveWait` intent visible to the scheduler's idle/drain checks, but it did not make the scheduler execute that work when woken by `TuiCommand::ResolveMerge`.

Observed behavior remains unchanged:

- A change starts or resumes as `merge wait`.
- The user presses `M`.
- TUI logs: `Scheduled merge-wait retry intent for '<change>'; execution will be started by scheduler`.
- The row becomes `resolve pending`.
- No scheduler-owned merge/resolve retry starts, because the scheduler wakes, sees no queued apply/archive work, and returns to waiting without calling the deferred retry path.

The canonical model is still correct: reducer owns intent, scheduler owns execution, reducer events own completion semantics. The missing piece is dispatch, not another local TUI execution lane.

## Proposed Solution

Teach the persistent parallel scheduler to consume reducer-owned `ResolveWait` intent as runnable scheduler work when it is woken or otherwise evaluates runnable work.

The implementation should:

- Sync shared reducer `ResolveWait` intent into scheduler state before scheduler work decisions.
- Trigger the existing scheduler-owned retry path for synced `ResolveWait` changes even when `queued` and `in_flight` are empty.
- Avoid busy retry loops when a retry remains auto-resumable or still blocked.
- Preserve existing retry triggers after merge/resolve/reject completion.
- Keep all terminal, deferred, failure, and refresh semantics routed through existing reducer events.

## Acceptance Criteria

- Pressing `M` on a `merge wait` row starts a scheduler-owned retry attempt without requiring new apply/archive queue work.
- The log line saying execution will be started by scheduler is followed by observable scheduler retry behavior, such as `Retrying deferred merge for '<change>'`, `MergeStarted`, `ResolveStarted`, `MergeDeferred`, `ResolveFailed`, or `MergeCompleted`.
- The scheduler does not spin retry attempts every tick when retry remains blocked for an auto-resumable reason.
- Completion-triggered retries for auto-resumable deferred merges continue to work.
- The TUI does not regain direct ownership of merge/resolve execution.

## Explicit Completion Conditions

- `src/parallel/orchestration.rs` or adjacent scheduler code includes a path that calls the deferred/manual retry executor after reducer-owned `ResolveWait` intent is synced and before waiting again.
- The retry path is gated so unchanged still-blocked `ResolveWait` work does not cause tight-loop retry attempts.
- `src/tui/command_handlers.rs` continues to only record reducer intent and wake the scheduler for `TuiCommand::ResolveMerge`.
- Regression tests fail on the previous implementation where scheduler only observed `ResolveWait` for drain checks, and pass once scheduler dispatches retry work.
- Targeted tests and formatting pass.

## Out of Scope

- Reintroducing direct TUI execution of merge resolution.
- Changing display labels for `merge wait` or `resolve pending`.
- Rewriting the full scheduler loop or dependency analyzer.
- Adding durable workflow-control state outside workspace/git/base-tree inputs.
