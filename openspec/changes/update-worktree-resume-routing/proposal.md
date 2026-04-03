---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/parallel/dispatch.rs
  - src/execution/state.rs
  - src/server/api.rs
---

# Change: Normalize resumed worktree routing to apply or acceptance

**Change Type**: implementation

## Problem / Context

- Parallel resume currently derives an `Applied`/`Archiving` workspace state and can route directly toward archive handling.
- The requested behavior is stricter: resume decisions must be made from the worktree state, and non-terminal resumed changes must never jump directly to archive.
- When a reused worktree still has incomplete `tasks.md`, routing toward archive produces confusing `task incomplete` failures instead of returning to implementation.
- Existing parallel execution already treats `apply -> acceptance -> archive` as the canonical order; resume should preserve that order instead of bypassing it.

## Proposed Solution

- Introduce a deterministic worktree-based resume action decision that chooses only among terminal/no-op, `apply`, or `acceptance`.
- Prohibit resume-time direct entry into archive for non-terminal worktrees.
- Use worktree-local task progress as the primary gate:
  - if `tasks.md` progress is 100%, resume at acceptance
  - otherwise resume at apply
- Preserve terminal handling for already archived/merged/rejected worktrees.
- Update parallel runtime/display reconciliation so resumed changes no longer appear to be archiving before acceptance has re-run.

## Acceptance Criteria

- Resumed non-terminal worktrees never start archive directly.
- A resumed worktree with incomplete `tasks.md` resumes with apply.
- A resumed worktree with 100% complete `tasks.md` resumes with acceptance.
- Archive only starts after acceptance succeeds in the resumed flow.
- Parallel state/display logic remains consistent with the new resume routing semantics.

## Out of Scope

- Redesigning merge-wait or resolve-wait semantics after archive completion.
- Changing how archived or merged worktrees are treated as terminal states.
- Altering the acceptance verdict protocol itself.
