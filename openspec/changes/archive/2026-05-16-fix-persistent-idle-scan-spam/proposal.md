---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/tui/queue.rs
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: Fix persistent idle scheduler scan spam

**Change Type**: implementation

## Problem / Context

In persistent TUI/server-style parallel execution, the scheduler can become idle with no queued work, no in-flight work, no resolve/reject waiters, and no pending merge tasks.

Current behavior still wakes every 500ms via the scheduler debounce timer. Each wake re-enters the top of the loop, runs dynamic queue checks, synchronizes reducer state, and performs queue reconciliation. Queue reconciliation may call worktree discovery and base-branch merge-state checks even though no work can be dispatched.

With debug logging enabled, this produces repeated scan noise such as:

- `Executing git command: worktree list --porcelain`
- `Executing git command: rev-parse --abbrev-ref HEAD`
- `Found consistent worktree ...`
- `is_merged_to_base: checking base branch HEAD tree file state`
- `Scheduler idle with no work; waiting for dynamic queue notifications (persistent lifetime)`

The scheduler should stay alive while persistent, but it should not poll-scan the repository forever when fully idle.

## Proposed Solution

Change persistent idle scheduling from timer-driven polling to event-driven waiting.

When the scheduler has no queued work, no in-flight work, no resolve/reject waiters, no active manual resolve, and no pending merge task, it should enter a persistent idle wait state that wakes only for meaningful events:

- dynamic queue notifications, including ordinary queued changes and explicit scheduler notifications
- background merge result notifications when a merge task is still possible to receive
- cancellation / stop signals if the persistent scheduler owns a cancellation token

On wake, the scheduler may re-run normal reconciliation and dispatch logic. While no wake event occurs, it must not repeatedly invoke worktree scans, base-branch checks, or repeated idle log emission.

The fix must preserve finite scheduler behavior: finite runs still exit once drained.

## Acceptance Criteria

- A persistent scheduler with no queued work, no in-flight work, no resolve/reject waiters, no manual resolve activity, and no pending merge tasks waits without running repository/worktree reconciliation every 500ms.
- The idle message is emitted at most once per idle entry, not continuously on each timer tick.
- Adding a change through `DynamicQueue::push` wakes the persistent scheduler and allows normal dispatch/reanalysis to proceed.
- Calling `DynamicQueue::notify_scheduler` for scheduler-owned retry work wakes the persistent scheduler without requiring another queued change.
- Finite scheduler lifetime still exits after all work drains.
- The implementation does not introduce durable workflow-control state and remains compatible with the workspace-local workflow state constitution.

## Explicit Completion Conditions

- `src/parallel/orchestration.rs` contains a distinct persistent-idle wait path or equivalent logic that does not include the 500ms debounce timer while fully idle.
- The normal active scheduler wait path still handles task completion, merge results, queue notifications, and debounce timing when work exists or may soon exist.
- Tests prove persistent idle does not repeatedly run reconciliation/scan work without a wake signal.
- Tests prove queue notification wakes persistent idle and permits queued work to be consumed.
- Tests prove finite idle exit behavior remains unchanged.
- Verification includes targeted Rust tests for the scheduler behavior and standard Rust format/lint/test commands or documented blockers.

## Out of Scope

- Changing VCS command debug logging levels globally.
- Removing debug logs for actual VCS commands when debug logging is intentionally enabled.
- Reworking queue reconciliation semantics for archived dirty repair candidates beyond suppressing idle polling.
- Introducing filesystem watchers or durable background state.
- Changing user-facing queue controls or TUI key bindings.
