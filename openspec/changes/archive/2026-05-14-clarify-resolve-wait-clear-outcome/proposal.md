---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
  - src/parallel/tests/executor.rs
  - openspec/specs/code-maintenance/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Clarify ResolveWait clearing outcome naming

**Change Type**: implementation

## Problem / Context

`ParallelExecutor::clear_resolve_wait_intent_for_success` is now used for more than successful merge outcomes. The same helper clears executor-local and reducer-owned `ResolveWait` membership when retry work reaches outcomes such as:

- already merged to base;
- merge retry success;
- missing archived workspace;
- stale workspace path.

The current function name says `for_success`, which can mislead future maintainers into thinking stale or missing workspace cleanup is being treated as a successful merge. This is especially risky because `ResolveWait` / `resolve pending` has regressed several times, and recent fixes rely on clear ownership between executor-local retry caches and reducer-owned retry intent.

## Proposed Solution

Prefer a small, behavior-preserving rename of the helper to a neutral outcome-oriented name, such as `clear_resolve_wait_intent_for_outcome`, and update all call sites in `src/parallel/queue_state.rs`.

If the implementation agent determines a rename is unexpectedly risky, add explicit code comments at the helper definition and stale/missing-workspace call sites documenting that the helper clears retry intent for terminal or no-longer-retryable outcomes, not only successful merges.

The change must not alter retry classification, reducer state transitions, event emission, or workspace cleanup behavior.

## Acceptance Criteria

- The helper name or adjacent code comments accurately communicate that the function clears `ResolveWait` for success, already-merged, stale workspace, and missing workspace outcomes.
- Missing or stale workspace handling no longer appears to rely on a success-only semantic from the code name alone.
- Existing `ResolveWait` behavior is preserved: success clears retry intent, stale/missing workspace clears retry intent, dirty base demotes to `merge wait`, and valid clean retry can proceed.
- Focused regression tests for missing/stale workspace and manual resolve dispatch continue to pass.
- No product behavior changes are introduced beyond naming/comment clarity.

## Explicit Completion Conditions

Implementation is complete when repository evidence shows:

- `src/parallel/queue_state.rs` no longer uses a misleading success-only helper name for stale/missing workspace cleanup, or comments clearly document the broader semantics where the helper is defined and used.
- All call sites compile after the rename/comment change.
- Focused tests pass for `test_missing_workspace_retry_clears_resolve_wait_in_reducer`, `test_stale_workspace_retry_clears_resolve_wait_in_reducer`, and `test_scheduler_dispatches_synced_manual_resolve_wait_without_queued_work`.
- Formatting and the repository's standard lint/typecheck commands pass if available.

## Out of Scope

- Changing `ResolveWait` retry scheduling behavior.
- Changing reducer-owned lifecycle semantics.
- Changing TUI display states or event log wording, except comments or internal helper names.
- Introducing new durable workflow state.
