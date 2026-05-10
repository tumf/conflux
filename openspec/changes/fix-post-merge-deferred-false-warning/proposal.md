---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
  - src/parallel/dispatch.rs
  - openspec/specs/orchestration-state/spec.md
---

# Fix post-merge deferred false warning

**Change Type**: implementation

## Problem/Context

Recent Conflux logs show a change that was successfully merged and cleaned up, followed milliseconds later by a misleading manual `MergeDeferred` warning:

- `Merge completed for 'adopt-alt-residual-stateful-pair'`
- then `Archive incomplete for 'adopt-alt-residual-stateful-pair': worktree may be dirty...`
- then `Merge deferred for adopt-alt-residual-stateful-pair...`

The inspected timeline shows the resolve command created merge commit `43ecffe`, `git status` was clean, `MergeCompleted` was emitted, and worktree cleanup began. The later archive-incomplete warning came from a second background merge path that still observed the soon-to-be-removed archived worktree as dirty/incomplete after the change was already merged. This is not a valid user-action blocker and should not surface as an error/deferred state after terminal success.

The fix must obey `openspec/CONSTITUTION.md`: workflow routing must remain derivable from workspace/base git state, not durable external state or logs.

## Proposed Solution

Make post-archive merge handling idempotent around base integration:

- Before returning manual `MergeDeferred` for archive-incomplete verification, detect whether the target change is already integrated into the base branch.
- Suppress stale `MergeDeferred` events for already-merged changes and treat the duplicate merge task as completed/cleanable instead of user-action-required.
- Prevent queue reconciliation from re-dispatching archive-complete workspaces while an active merge task for that same change is still in progress.
- Add regression coverage for duplicate post-archive merge races so a terminal `MergeCompleted` cannot be followed by an archive-incomplete manual deferral for the same integrated change.

## Acceptance Criteria

- A change that has already reached base integration MUST NOT emit a later manual `MergeDeferred(auto_resumable=false)` solely because its archived worktree is dirty, incomplete, or being cleaned up.
- Duplicate background merge tasks for the same archived change MUST be idempotent: at most one path performs the merge, and later stale paths clear/suppress retry intent when base integration is already true.
- Queue reconciliation MUST NOT re-add an archived dirty repair candidate for a change that is already being handled by an active post-archive merge task.
- Existing legitimate manual deferrals, such as a genuinely dirty base before merge, MUST remain visible as `MergeWait` and must not be silently suppressed.

## Explicit Completion Conditions

- `src/parallel/merge.rs` checks base integration before surfacing archive-incomplete manual deferral from `attempt_merge`/post-archive merge handling.
- `src/parallel/queue_state.rs` or adjacent scheduler code tracks/consults in-flight post-archive merge work for the same change so reconciliation does not dispatch the same archived workspace as ordinary queued work while a merge task is running.
- Regression tests cover the stale duplicate-merge race and the still-valid dirty-base manual deferral path.
- The proposal validates with `cflx openspec validate fix-post-merge-deferred-false-warning --strict` and evidence warnings are reviewed.

## Out of Scope

- Changing the user-facing semantics of legitimate `MergeWait` after real dirty-base or unresolved-conflict blockers.
- Introducing durable workflow state outside the repository/workspace.
- Reworking dependency target semantics for rejected dependencies; those diagnostics are valid fail-closed behavior when a change depends on a rejected/missing proposal.
