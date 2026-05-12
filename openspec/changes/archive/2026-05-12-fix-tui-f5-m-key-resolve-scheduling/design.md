## Context

The TUI currently has two separate operator controls that became conflated:

- `F5` controls orchestration start/resume/retry and should be independent of the cursor row.
- `M` in Changes view registers retry intent for the cursor's `MergeWait` row.

The regression came from a historical change that tried to prevent `F5` on `MergeWait` from re-queuing apply/acceptance work by making `F5` resolve the cursor row instead. The correct fix is to exclude wait states from normal runnable queues while preserving F5 as global orchestration control.

## Design Principles

### F5 is orchestration control

`F5` should only choose among app-level orchestration commands:

- `start_processing()` in Select mode
- `resume_processing()` in Stopped mode
- `retry_error_changes()` in Error mode
- graceful-stop cancellation in Stopping mode

It must not dispatch cursor-local row actions. In particular, the cursor row being `merge wait` must not cause `F5` to emit `ResolveMerge`.

### M is cursor-local intent registration

Changes-view `M` is intentionally cursor-local. It operates only when the focused row is `MergeWait` and registers reducer-owned `ResolveMerge` intent. The actual merge/resolve retry is scheduler-owned.

Worktrees-view `M` is a separate view-scoped operation for merging a selected worktree branch into base. The proposal keeps this behavior separate and unchanged.

### Scheduler classification order

Merge retry classification must evaluate in this order:

1. Active resolve/base-mutating lane occupancy.
2. Dirty workspace/base state.
3. Clean retry eligibility.

This order matters because an active resolve may make the base/workspace appear dirty. That dirty state is auto-resumable because it is caused by an in-flight scheduler-owned base-mutating operation. If dirty checks run first, later changes can be incorrectly classified as manual `merge wait`.

## State Model

```text
merge wait
  M pressed
  retry intent registered
  |
  v
classification:
  active resolve/base-mutating lane?
    yes -> resolve pending
    no  -> dirty/manual blocker?
             yes -> merge wait
             no  -> resolving (scheduler starts one retry)
```

`resolve pending` must correspond to reducer-owned scheduler-consumable retry membership. If the reducer rejects the intent or a later scheduler classification finds manual blocker evidence with no active resolve/base-mutating lane, the row must not remain in stale pending state.

## Verification Strategy

- TUI unit tests protect key responsibility boundaries:
  - F5 cannot emit `ResolveMerge` from cursor state.
  - M emits/queues `ResolveMerge` only for `MergeWait` in Changes view.
  - key hints match responsibility boundaries.
- Parallel scheduler tests protect classification ordering:
  - dirty during active resolve/base-mutating work remains auto-resumable pending.
  - dirty without active resolve/base-mutating work becomes manual merge wait.
  - clean pending retry promotes exactly one item to resolving.
- Integration-style scheduler tests ensure reducer-owned `ResolveWait` can be consumed even when no normal queued changes are present.

## Constitutional Compliance

This design does not introduce durable out-of-worktree workflow-control state. Retry eligibility remains derived from workspace/git/base-tree evidence plus in-memory scheduler/reducer state for active orchestration observability and dispatch coordination.
