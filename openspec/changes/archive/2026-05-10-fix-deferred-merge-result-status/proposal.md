---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
---

# Fix deferred merge result status

**Change Type**: implementation

## Problem / Context

Runtime logs show parallel merge attempts that emit `MergeDeferred` because archive verification is incomplete or the verification path is no longer a valid git worktree, followed immediately by `Background merge task completed successfully for '<change>'`. A deferred merge is not a completed merge: it leaves the change in merge-wait/resolve-wait state and requires later retry or operator action.

This produces misleading operator diagnostics and can cause scheduler bookkeeping to run success follow-up work, such as retrying base-lane waiters, as if the merge had actually completed. The fix must preserve truthful completion and must derive state from repository/workspace evidence, not from logs.

## Proposed Solution

Distinguish background merge task outcomes for completed, deferred, and failed merge attempts.

- Make `handle_merge_and_cleanup` or the merge task result report whether the merge actually completed or was deferred.
- Log deferred merge outcomes as deferred/pending, not as successful completion.
- Avoid running success-only follow-up behavior for a deferred merge unless that behavior is explicitly safe for pending/deferred state.
- Preserve existing `MergeDeferred` events and workspace status transitions.
- Add regression coverage for both manual and auto-resumable deferred merge outcomes.

## Acceptance Criteria

- A merge attempt that returns `MergeAttempt::Deferred` does not produce `Background merge task completed successfully` for that change.
- Deferred merge outcomes remain visible to the TUI/event stream through `MergeDeferred` and the appropriate workspace status update.
- Success-only follow-up behavior is only triggered for actual `MergeAttempt::Merged` outcomes.
- Failed merge tasks still produce an error result and error event.
- Already-merged idempotent outcomes that return `MergeAttempt::Merged { revision: "already-merged-to-base" }` remain treated as successful completion.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `src/parallel/merge.rs` exposes an outcome that differentiates merged vs deferred instead of collapsing both into `Ok(())`.
- `src/parallel/queue_state.rs` logs and handles deferred outcomes separately from successful background merge completion.
- Tests cover `MergeAttempt::Deferred` so they fail if it is logged/handled as completed successfully.
- Tests cover a real `MergeAttempt::Merged` outcome so success behavior is preserved.
- `cflx openspec validate fix-deferred-merge-result-status --strict --evidence warn` passes.

## Completeness Checklist

- User-facing outcome: logs no longer claim a merge completed when it was deferred.
- Repository areas likely requiring change: `src/parallel/merge.rs`, `src/parallel/queue_state.rs`, and parallel merge/queue tests.
- Required verification: unit/integration tests for deferred, failed, and merged background merge result handling.
- Dependencies and rollout: no migration and no durable state changes.
- Non-goal: do not alter archive verification truth rules or suppress legitimate archive/merge blockers.

## Out of Scope

- Changing archive gate semantics.
- Treating deferred merge as failure.
- Changing manual `M` key retry semantics.
- Using logs as workflow-control input.
