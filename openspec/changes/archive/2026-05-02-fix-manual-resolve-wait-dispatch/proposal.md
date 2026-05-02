---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/tui-resolve-queue/spec.md
  - src/tui/state.rs
  - src/tui/command_handlers.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
---

# Fix Manual Resolve Wait Dispatch

**Change Type**: implementation

## Problem / Context

A parallel-mode change can correctly appear as `merge wait` after startup or workspace refresh. When the user presses `M`, the TUI transitions the row to `resolve pending` and records `ResolveMerge` intent in the shared reducer, but the scheduler can fail to consume that intent as runnable work.

The user-visible result is a stuck row: `merge wait` becomes `resolve pending`, then no merge/resolve retry starts.

This violates the existing ownership rule in `orchestration-state`: intent belongs in the reducer, execution belongs in the scheduler, and completion semantics belong in reducer events.

## Proposed Solution

Make manual `ResolveMerge` intent scheduler-visible in the normal parallel scheduler loop, including the idle / wake-up path.

The implementation should keep the architecture split intact:

- TUI `M` key records reducer-owned retry intent and notifies the scheduler.
- The scheduler syncs reducer-owned `ResolveWait` intent before deciding that work is drained or before waiting again.
- The scheduler attempts deferred merge/retry work through the existing merge/retry path rather than direct TUI execution.
- Completion/failure continues to flow through `MergeCompleted`, `MergeDeferred`, `ResolveStarted`, `ResolveCompleted`, or `ResolveFailed` events as appropriate.

The fix must not introduce durable workflow-control state outside workspace/git/base-tree inputs and shared in-memory runtime coordination.

## Acceptance Criteria

- A change restored or observed as `merge wait` can be manually selected with `M` and must not remain indefinitely in `resolve pending` while the scheduler is alive.
- Manual `ResolveMerge` intent recorded in shared orchestration state is visible to `ParallelExecutor` without depending on TUI-local `resolve_queue` state.
- Scheduler idle/drained checks account for reducer-owned `ResolveWait` intent before exiting or sleeping as though no work exists.
- Existing auto-resumable `MergeDeferred(auto_resumable=true)` behavior remains intact.
- Refresh-driven reconciliation must not regress `resolve pending` back to `merge wait` before the scheduler consumes the intent.

## Explicit Completion Conditions

- `src/parallel/orchestration.rs` or adjacent scheduler code has a documented code path that syncs reducer-owned resolve wait intent before idle/drained decisions.
- `src/parallel/queue_state.rs` or adjacent queue/retry code uses the same reducer-owned intent source when retrying deferred/manual merge waits.
- Regression tests cover the startup/refresh case: reducer state contains `MergeWait`, `M` records `ResolveWait`, the scheduler sees the intent and does not treat the loop as drained.
- Existing tests for TUI `resolve_merge()` and deferred merge retry still pass.
- `cargo fmt --check`, targeted regression tests, and the relevant Rust test module pass.

## Out of Scope

- Changing the user-facing labels `merge wait` or `resolve pending`.
- Replacing the reducer-owned lifecycle model.
- Introducing persistent workflow-control files outside workspaces.
- Reworking conflict resolution prompts or AI resolve behavior.
