---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/dynamic_queue.rs
  - src/parallel/tests/auto_resolve.rs
  - src/parallel/tests/manual_resolve.rs
  - openspec/specs/parallel-execution/spec.md
---

# Fix Resolve Queue Re-analysis

**Change Type**: implementation

## Problem/Context

During parallel execution, queued changes can remain stuck while another change is resolving. The scheduler currently checks available execution slots before entering the re-analysis/dispatch path. Because active manual or automatic resolve work consumes scheduler capacity, `available_slots == 0` can prevent queued work from reaching dependency analysis and normal apply dispatch evaluation.

This conflicts with the intended non-blocking scheduler behavior: queue reconciliation and dependency analysis should remain observable while resolve work is active, while actual apply dispatch must still respect slot limits.

## Proposed Solution

Adjust the parallel scheduler so queued work continues through the re-analysis path while resolve work is active, but ordinary apply dispatch remains gated by recalculated available slots.

The implementation should:

- remove or relax the outer scheduler precondition that skips `perform_reanalysis_and_dispatch()` solely because `available_slots == 0`;
- allow queued work classification, reducer-visible queue reconciliation, dependency analysis, and diagnostics to run during active resolve;
- prevent ordinary apply dispatch when recalculated capacity is still zero;
- dispatch queued changes promptly after resolve completion or slot recovery without waiting for a stale queue debounce window;
- preserve blocked-only drain behavior and scheduler ownership of resolve/reject waiters.

## Acceptance Criteria

- Resolve activity no longer prevents queued ordinary changes from entering analysis/re-analysis evaluation.
- Queued ordinary changes are not dispatched while resolve consumes all scheduler capacity.
- When resolve completes and capacity is available, eligible queued changes proceed to apply without requiring another queue notification or user action.
- Operator-visible logs/events distinguish capacity-gated dispatch from absence of analysis candidates.
- Existing blocked-only, persistent-idle, resolve-wait, reject-wait, and terminal-error stop-gate behavior remains intact.

## Explicit Completion Conditions

- `src/parallel/orchestration.rs` no longer has an outer `available_slots > 0` guard that prevents queued work from entering the re-analysis path.
- `src/parallel/queue_state.rs` performs analysis/re-analysis for queued dispatchable candidates even when current capacity is zero, while still preventing dispatch when recalculated capacity is zero.
- Regression tests cover queued work during active manual resolve and/or auto resolve, zero-capacity dispatch suppression, and dispatch after resolve completion/slot recovery.
- `cflx openspec validate fix-resolve-queue-reanalysis --strict` passes.
- Relevant Rust tests for parallel scheduler behavior pass.

## Out of Scope

- Changing global merge/resolve locking semantics.
- Increasing configured parallelism or allowing resolve plus apply execution to exceed configured capacity.
- Replacing dependency analysis or changing dependency ordering semantics.
- Changing user-facing key bindings for manual resolve.
