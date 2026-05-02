---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
  - src/parallel/conflict.rs
  - src/hooks.rs
  - openspec/specs/hooks/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Change: Fix on_merged deferred retry idempotency

**Change Type**: implementation

## Premise / Context

- `on_merged` is configured to run `make bump-patch`, which mutates version files and creates a release commit/tag through `scripts/bump.sh`.
- Investigation of the 2026-05-02 run showed the same deferred merge reaching `Deferred merge succeeded` twice for `fix-conflictless-merge-resolve-retry`.
- The first success started `on_merged` at `03:45:41`, then a no-op resolve path ran for the same change, and a second deferred retry started `on_merged` again at `03:46:02`.
- The second hook did not complete before the orchestrator exited/restarted, leaving generated version-file changes uncommitted.
- The likely code-level issue is that `retry_deferred_merges()` removes the change from its local `resolve_wait_changes` set but does not clear reducer-owned/shared resolve-wait intent before later synchronization repopulates it.
- The Conflux Constitution requires workflow state to remain derivable from workspace/git/base-tree evidence and not depend on out-of-worktree durable state.

## Problem

A successful deferred merge can be retried again for the same change before or after `on_merged` has run. Because `on_merged` may perform non-idempotent release side effects, duplicate invocation can leave partially generated release artifacts in the working tree and obscure the real merge status.

## Proposed Solution

Make deferred merge retry completion idempotent across scheduler-local state, reducer-owned shared state, and hook execution:

1. When a deferred merge succeeds, remove the change from both local retry tracking and reducer/shared resolve-wait intent before future scheduler sync can reintroduce it.
2. Ensure a change that has reached merged state is not eligible for another deferred merge retry in the same scheduler run.
3. Ensure `on_merged` runs at most once for a successful merge integration path for a given change.
4. Preserve the existing timing guarantee: `on_merged` still runs after repository-visible merge success and before `MergeCompleted` / terminal merged status.
5. Add regression coverage for the observed double-retry/double-hook path.

## Acceptance Criteria

- A deferred merge that succeeds is removed from all authoritative in-memory retry intents before another retry dispatch can observe it.
- Re-syncing from reducer-owned shared state after a successful deferred merge does not re-add the same change to `resolve_wait_changes`.
- `on_merged` executes exactly once per successful change merge, including deferred merge retry success paths.
- A no-op or stale resolve retry for an already-merged change does not invoke `on_merged` again.
- The fix does not introduce out-of-worktree durable workflow-control state and remains consistent with `openspec/CONSTITUTION.md`.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` or related reducer/state code clears successful deferred merge retry intent from shared state and local state.
- Regression tests demonstrate that a deferred merge success followed by scheduler state synchronization does not retry the same change or emit a second `on_merged` hook event.
- Existing merge behavior tests still pass for immediate merge success, conflictless deferred merge success, and true-conflict resolve paths.
- `cflx openspec validate fix-on-merged-deferred-retry-idempotency --strict --evidence warn` passes without behavior-evidence warnings.
- Repository verification commands used by this project pass for the touched Rust modules.

## Out of Scope

- Changing the `make bump-patch` / `scripts/bump.sh` release workflow.
- Making release hooks themselves transactional or rollback-capable.
- Changing the user-visible meaning of `MergeWait`, `ResolveWait`, or `Merged` beyond preventing duplicate retry of already-merged changes.
- Introducing persistent lock files, databases, or other out-of-worktree workflow-control state.
